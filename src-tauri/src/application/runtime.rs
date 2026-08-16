use crate::{
    application::{data, services},
    domain::log_events::{parse_log_event, GroupChangeKind, LogEvent},
    infrastructure::database::Database,
};
use rusqlite::params;
use serde_json::json;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

pub fn start(database_path: PathBuf) {
    thread::Builder::new()
        .name("eq-runtime-watcher".into())
        .spawn(move || watch(database_path))
        .expect("runtime watcher thread must start");
}

fn watch(database_path: PathBuf) {
    let database = match Database::open(database_path) {
        Ok(database) => database,
        Err(_) => return,
    };
    let mut active_log: Option<PathBuf> = None;
    let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
    let mut last_mob: HashMap<PathBuf, String> = HashMap::new();
    let mut export_signatures: HashMap<PathBuf, (u64, SystemTime)> = HashMap::new();
    let mut export_directory: Option<PathBuf> = None;
    loop {
        if let Err(error) = poll(
            &database,
            &mut active_log,
            &mut offsets,
            &mut last_mob,
            &mut export_signatures,
            &mut export_directory,
        ) {
            log(&database, "error", "watcher", &error);
        }
        thread::sleep(Duration::from_millis(750));
    }
}

fn poll(
    database: &Database,
    active_log: &mut Option<PathBuf>,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    exports: &mut HashMap<PathBuf, (u64, SystemTime)>,
    watched_export_directory: &mut Option<PathBuf>,
) -> Result<(), String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let directory: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |row| row.get(0),
        )
        .ok();
    drop(connection);
    let Some(directory) = directory else {
        return Ok(());
    };
    let directory = PathBuf::from(directory);
    if !directory.is_dir() {
        return Ok(());
    }

    let mut logs = fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_log(path))
        .collect::<Vec<_>>();
    logs.sort_by_key(|path| {
        fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH)
    });
    if let Some(newest) = logs.pop() {
        if active_log.as_ref() != Some(&newest) {
            let character = character_from_log(&newest).unwrap_or_else(|| "Unknown".into());
            let connection = database.connect().map_err(|error| error.to_string())?;
            let previous: Option<String> = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key='active_character'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            if previous
                .as_deref()
                .is_some_and(|value| !value.eq_ignore_ascii_case(&character))
            {
                connection
                    .execute("DELETE FROM current_group", [])
                    .map_err(|e| e.to_string())?;
            }
            connection
                .execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [&character],
                )
                .map_err(|e| e.to_string())?;
            connection.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",[&character]).map_err(|e|e.to_string())?;
            connection.execute("INSERT INTO app_settings(key,value) VALUES('active_character',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[&character]).map_err(|e|e.to_string())?;
            log_with(
                &connection,
                "info",
                "watcher",
                &format!("Active log: {} ({character})", newest.display()),
            );
            let size = fs::metadata(&newest).map(|m| m.len()).unwrap_or(0);
            offsets.entry(newest.clone()).or_insert(size);
            *active_log = Some(newest.clone());
        }
        process_log(database, &newest, offsets, last_mob)?;
    }
    let output = services::output_directory(&directory);
    if watched_export_directory.as_ref() != Some(&output) {
        exports.clear();
        baseline_exports(&output, exports)?;
        *watched_export_directory = Some(output.clone());
        let connection = database.connect().map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO app_settings(key,value) VALUES('export_directory',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[output.display().to_string()]).map_err(|error|error.to_string())?;
        log_with(
            &connection,
            "info",
            "watcher",
            &format!("Watching exports in {}", output.display()),
        );
    } else {
        process_exports(database, &output, exports)?;
    }
    Ok(())
}

fn process_log(
    database: &Database,
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let offset = offsets.entry(path.to_owned()).or_insert(size);
    if size < *offset {
        *offset = 0;
    }
    if size == *offset {
        return Ok(());
    }
    let mut line_offset = *offset;
    file.seek(SeekFrom::Start(line_offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    for line_bytes in bytes.split_inclusive(|byte| *byte == b'\n') {
        if !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(line_bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        if let Some(event) = parse_log_event(line, &character) {
            apply_event(database, path, line_offset as i64, line, &event, last_mob)?;
        } else if line.to_ascii_lowercase().contains(" looted ") {
            log(
                database,
                "warning",
                "parser",
                &format!("Unrecognized loot line in {}: {line}", path.display()),
            );
        }
        line_offset += line_bytes.len() as u64;
    }
    *offset = line_offset;
    if let Ok(connection) = database.connect() {
        let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('active_log_path',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[path.display().to_string()]);
        let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('active_log_offset',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[line_offset.to_string()]);
        let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('last_log_read_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]);
    }
    Ok(())
}

fn apply_event(
    database: &Database,
    path: &Path,
    source_offset: i64,
    raw: &str,
    event: &LogEvent,
    last_mob: &mut HashMap<PathBuf, String>,
) -> Result<(), String> {
    let c = database.connect().map_err(|e| e.to_string())?;
    match event {
        LogEvent::MobSlain { mob_name, .. } => {
            c.execute(
                "INSERT INTO mobs(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                [mob_name],
            )
            .map_err(|e| e.to_string())?;
            last_mob.insert(path.to_owned(), mob_name.clone());
        }
        LogEvent::GroupChange {
            character, change, ..
        } => {
            c.execute(
                "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                [character],
            )
            .map_err(|e| e.to_string())?;
            match change {
                GroupChangeKind::Left => {
                    c.execute("DELETE FROM current_group WHERE member_id=(SELECT id FROM known_members WHERE name=? COLLATE NOCASE)",[character]).map_err(|e|e.to_string())?;
                }
                _ => {
                    c.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",[character]).map_err(|e|e.to_string())?;
                }
            }
        }
        LogEvent::Loot {
            happened_at,
            looter,
            item_name,
        } => {
            let source = path.display().to_string();
            let inserted = c.execute("INSERT OR IGNORE INTO loot_drops(happened_at,item_name,mob_name,looter_name,raw_line,source_file,source_offset) VALUES(?,?,?,?,?,?,?)",params![happened_at.to_string(),item_name,last_mob.get(path),looter,raw,source,source_offset]).map_err(|e|e.to_string())?;
            if inserted > 0 {
                let id = c.last_insert_rowid();
                c.execute("INSERT OR IGNORE INTO loot_drop_members(loot_drop_id,member_name) SELECT ?,m.name FROM current_group g JOIN known_members m ON m.id=g.member_id",[id]).map_err(|e|e.to_string())?;
                log_with(&c, "info", "loot", &format!("{looter} looted {item_name}"));
            }
        }
    }
    Ok(())
}

fn process_exports(
    database: &Database,
    directory: &Path,
    seen: &mut HashMap<PathBuf, (u64, SystemTime)>,
) -> Result<(), String> {
    for entry in fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let lower = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !(lower.ends_with("-inventory.txt") || lower.ends_with("-spellbook.txt")) {
            continue;
        }
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let signature = (
            metadata.len(),
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        );
        match seen.get(&path) {
            None | Some(_) if seen.get(&path) != Some(&signature) => {
                seen.insert(path.clone(), signature);
                match data::mutate(
                    database,
                    "inventory.import",
                    &json!({"path":path.display().to_string()}),
                ) {
                    Ok(_) => {
                        if let Ok(connection) = database.connect() {
                            let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('last_export_file',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[path.display().to_string()]);
                            let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('last_export_import_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]);
                        }
                        log(
                            database,
                            "info",
                            "inventory",
                            &format!("Imported {}", path.display()),
                        );
                        let upload = fs::read_to_string(&path).map(|text| json!({"files":[{"name":path.file_name().and_then(|value|value.to_str()).unwrap_or(""),"text":text}]})).map_err(|error|error.to_string()).and_then(|payload|services::upload_file_payloads(database,&payload));
                        match upload {
                            Ok(_) => log(
                                database,
                                "info",
                                "planner",
                                "Uploaded exports; the private review link is available on System",
                            ),
                            Err(error) => log(
                                database,
                                "error",
                                "planner",
                                &format!("Inventory upload failed: {error}"),
                            ),
                        }
                    }
                    Err(e) => log(database, "error", "inventory", &e),
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn baseline_exports(
    directory: &Path,
    seen: &mut HashMap<PathBuf, (u64, SystemTime)>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !(name.ends_with("-inventory.txt") || name.ends_with("-spellbook.txt")) {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        seen.insert(
            path,
            (
                metadata.len(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            ),
        );
    }
    Ok(())
}

fn is_log(path: &Path) -> bool {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
    name.to_ascii_lowercase().starts_with("eqlog_")
        && name.to_ascii_lowercase().ends_with("_p1999green.txt")
}
fn character_from_log(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if !name.get(..6)?.eq_ignore_ascii_case("eqlog_") {
        return None;
    }
    let rest = &name[6..];
    let marker = rest.to_ascii_lowercase().rfind("_p1999green.txt")?;
    Some(rest[..marker].to_owned())
}
fn log(database: &Database, level: &str, area: &str, message: &str) {
    if let Ok(c) = database.connect() {
        log_with(&c, level, area, message)
    }
}
fn log_with(c: &rusqlite::Connection, level: &str, area: &str, message: &str) {
    let _ = c.execute(
        "INSERT INTO application_logs(level,area,message) VALUES(?,?,?)",
        params![level, area, message],
    );
}

#[cfg(test)]
mod tests {
    use super::process_log;
    use crate::infrastructure::database::Database;
    use std::{collections::HashMap, fs};

    #[test]
    fn processes_multiple_new_loot_lines_with_distinct_offsets() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(&log, b"[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--\r\n[Mon Aug 03 07:09:19 2026] --Vinkledoo has looted Blue Throne.--\r\n").unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let connection = database.connect().unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM loot_drops", [], |row| row.get(0))
            .unwrap();
        let distinct: i64 = connection
            .query_row(
                "SELECT COUNT(DISTINCT source_offset) FROM loot_drops",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((count, distinct), (2, 2));
    }
}
