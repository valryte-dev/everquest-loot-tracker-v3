use crate::{
    application::{data, services},
    domain::log_events::{parse_log_event, ChatChannel, GroupChangeKind, LogEvent},
    domain::merchant::{parse_listing_items, CatalogItem},
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
use tauri::Emitter;

pub fn start(database_path: PathBuf, app_handle: tauri::AppHandle) {
    thread::Builder::new()
        .name("eq-runtime-watcher".into())
        .spawn(move || watch(database_path, app_handle))
        .expect("runtime watcher thread must start");
}

fn watch(database_path: PathBuf, app_handle: tauri::AppHandle) {
    let signature_path = database_path.with_extension("db-wal");
    let database = match Database::open(database_path) {
        Ok(database) => database,
        Err(_) => return,
    };
    let mut active_log: Option<PathBuf> = None;
    let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
    let mut last_mob: HashMap<PathBuf, String> = HashMap::new();
    let mut export_signatures: HashMap<PathBuf, (u64, SystemTime)> = HashMap::new();
    let mut export_directory: Option<PathBuf> = None;
    let mut database_signature = file_signature(&signature_path);
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
        let next_signature = file_signature(&signature_path);
        if next_signature != database_signature {
            database_signature = next_signature;
            let _ = app_handle.emit("data-changed", "watcher");
        }
        thread::sleep(Duration::from_millis(750));
    }
}

fn file_signature(path: &Path) -> Option<(u64, SystemTime)> {
    fs::metadata(path).ok().map(|metadata| {
        (
            metadata.len(),
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        )
    })
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
                    if let Some(local_character) = character_from_log(path) {
                        c.execute(
                            "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                            [&local_character],
                        )
                        .map_err(|e| e.to_string())?;
                        c.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",[&local_character]).map_err(|e|e.to_string())?;
                    }
                }
            }
        }
        LogEvent::GroupCleared { .. } => {
            c.execute("DELETE FROM current_group", [])
                .map_err(|e| e.to_string())?;
            log_with(
                &c,
                "info",
                "group",
                "Local player was removed; current group cleared",
            );
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
        LogEvent::MerchantListing {
            happened_at,
            speaker,
            action,
            message,
        } => {
            if !merchant_mode_enabled(&c) {
                return Ok(());
            }
            let source = path.display().to_string();
            let inserted = c
                .execute(
                    "INSERT OR IGNORE INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(?,?,?,?,?,?,?)",
                    params![happened_at.to_string(), action.as_str(), speaker, message, raw, source, source_offset],
                )
                .map_err(|error| error.to_string())?;
            if inserted > 0 {
                let message_id = c.last_insert_rowid();
                let catalog = merchant_catalog(&c)?;
                for (order, item) in parse_listing_items(message, &catalog).iter().enumerate() {
                    c.execute(
                        "INSERT INTO merchant_message_items(merchant_message_id,item_name,item_id,asking_price_pp,sort_order) VALUES(?,?,?,?,?)",
                        params![message_id, item.item_name, item.item_id, item.asking_price_pp, order as i64],
                    )
                    .map_err(|error| error.to_string())?;
                }
                finish_merchant_capture(&c)?;
            }
        }
        LogEvent::DirectTell {
            happened_at,
            speaker,
            message,
        } => {
            if !merchant_mode_enabled(&c) {
                return Ok(());
            }
            let source = path.display().to_string();
            let inserted = c
                .execute(
                    "INSERT OR IGNORE INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(?,'tell',?,?,?,?,?)",
                    params![happened_at.to_string(), speaker, message, raw, source, source_offset],
                )
                .map_err(|error| error.to_string())?;
            if inserted > 0 {
                finish_merchant_capture(&c)?;
            }
        }
        LogEvent::LinkedItems {
            happened_at,
            speaker,
            channel,
            message,
            item_names,
        } => {
            if *channel == ChatChannel::Group {
                c.execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [speaker],
                )
                .map_err(|error| error.to_string())?;
                c.execute(
                    "INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",
                    [speaker],
                )
                .map_err(|error| error.to_string())?;
                if let Some(local_character) = character_from_log(path) {
                    c.execute(
                        "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                        [&local_character],
                    )
                    .map_err(|error| error.to_string())?;
                    c.execute(
                        "INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",
                        [&local_character],
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
            let source = path.display().to_string();
            let resolved_items = resolve_linked_items(&c, message, item_names)?;
            let mut inserted = 0;
            for (link_index, item_name) in resolved_items.iter().enumerate() {
                inserted += c
                    .execute(
                        "INSERT OR IGNORE INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                         VALUES(?,?,?,?,?,?,?,?)",
                        params![
                            happened_at.to_string(),
                            channel.as_str(),
                            speaker,
                            item_name,
                            raw,
                            source,
                            source_offset,
                            link_index as i64
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            if inserted > 0 {
                log_with(
                    &c,
                    "info",
                    "linked-loot",
                    &format!(
                        "{speaker} linked {} item{} in {} chat",
                        inserted,
                        if inserted == 1 { "" } else { "s" },
                        channel.as_str()
                    ),
                );
            }
        }
    }
    Ok(())
}

pub(crate) fn unquote_chat_message(message: &str) -> &str {
    let message = message.trim();
    let bytes = message.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == 39 || bytes[0] == 34) && bytes[bytes.len() - 1] == bytes[0]
    {
        message[1..message.len() - 1].trim()
    } else {
        message
    }
}

fn strip_linked_item_apostrophes(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(*character as u32, 0x27 | 0x60 | 0x2018 | 0x2019 | 0x00b4))
        .collect()
}

fn normalize_linked_item_text(value: &str) -> String {
    strip_linked_item_apostrophes(value)
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

pub(crate) fn resolve_linked_items(
    connection: &rusqlite::Connection,
    message: &str,
    encoded_items: &[String],
) -> Result<Vec<String>, String> {
    if !encoded_items.is_empty() {
        return Ok(encoded_items.to_vec());
    }
    let message = unquote_chat_message(message);
    let display_message = strip_linked_item_apostrophes(message);
    let normalized_message = normalize_linked_item_text(message);
    let mut statement = connection
        .prepare(
            "SELECT item_name FROM master_items
             WHERE item_name<>'' AND instr(
               replace(replace(replace(replace(replace(lower(?),char(39),''),'`',''),'’',''),'‘',''),'´',''),
               replace(replace(replace(replace(replace(lower(item_name),char(39),''),'`',''),'’',''),'‘',''),'´','')
             )>0
             UNION
             SELECT item_name FROM item_market_values
             WHERE item_name<>'' AND instr(
               replace(replace(replace(replace(replace(lower(?),char(39),''),'`',''),'’',''),'‘',''),'´',''),
               replace(replace(replace(replace(replace(lower(item_name),char(39),''),'`',''),'’',''),'‘',''),'´','')
             )>0",
        )
        .map_err(|error| error.to_string())?;
    let names = statement
        .query_map(params![message, message], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .map(|row| row.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut raw_matches = Vec::new();
    for name in names {
        let normalized_name = normalize_linked_item_text(&name);
        if normalized_name.is_empty() {
            continue;
        }
        for (start, _) in normalized_message.match_indices(&normalized_name) {
            let end = start + normalized_name.len();
            raw_matches.push((start, end, name.clone()));
        }
    }
    let mut matches = raw_matches
        .iter()
        .filter(|candidate| {
            let before = normalized_message[..candidate.0].chars().next_back();
            let after = normalized_message[candidate.1..].chars().next();
            let joins_previous = raw_matches.iter().any(|other| other.1 == candidate.0);
            let joins_next = raw_matches.iter().any(|other| other.0 == candidate.1);
            let follows_known_item = raw_matches.iter().any(|other| {
                other.1 <= candidate.0
                    && normalized_message[other.1..candidate.0]
                        .chars()
                        .all(|value| value.is_whitespace() || ",;/|:-".contains(value))
            });
            let suspicious_title_prefix = display_message[..candidate.0]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
                && display_message[..candidate.0]
                    .split_whitespace()
                    .next_back()
                    .map(|word| word.trim_matches(|value: char| !value.is_ascii_alphanumeric()))
                    .is_some_and(|word| {
                        let lower = word.to_ascii_lowercase();
                        word.chars()
                            .next()
                            .is_some_and(|value| value.is_ascii_uppercase())
                            && word.chars().skip(1).any(|value| value.is_ascii_lowercase())
                            && !matches!(
                                lower.as_str(),
                                "anyone"
                                    | "buying"
                                    | "check"
                                    | "found"
                                    | "getting"
                                    | "got"
                                    | "have"
                                    | "here"
                                    | "link"
                                    | "look"
                                    | "need"
                                    | "price"
                                    | "selling"
                                    | "someone"
                                    | "that"
                                    | "this"
                                    | "want"
                            )
                    });
            (before.is_none_or(|value| !value.is_ascii_alphanumeric()) || joins_previous)
                && (after.is_none_or(|value| !value.is_ascii_alphanumeric()) || joins_next)
                && (!suspicious_title_prefix || follows_known_item)
        })
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| (right.1 - right.0).cmp(&(left.1 - left.0)))
    });
    let mut selected: Vec<(usize, usize, String)> = Vec::new();
    for candidate in matches {
        if selected
            .iter()
            .all(|existing| candidate.1 <= existing.0 || candidate.0 >= existing.1)
        {
            selected.push(candidate);
        }
    }
    selected.sort_by_key(|value| value.0);
    Ok(selected.into_iter().map(|value| value.2).collect())
}

fn merchant_mode_enabled(connection: &rusqlite::Connection) -> bool {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='merchant_mode_enabled'",
            [],
            |row| row.get::<_, String>(0),
        )
        .is_ok_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
}

fn merchant_catalog(connection: &rusqlite::Connection) -> Result<Vec<CatalogItem>, String> {
    let mut statement = connection
        .prepare("SELECT item_id,item_name FROM master_items ORDER BY LENGTH(item_name) DESC")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(CatalogItem {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|error| error.to_string())?;
    let catalog = rows
        .map(|row| row.map_err(|error| error.to_string()))
        .collect();
    catalog
}

fn finish_merchant_capture(connection: &rusqlite::Connection) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('merchant_last_capture_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM merchant_messages WHERE id NOT IN (SELECT id FROM merchant_messages ORDER BY id DESC LIMIT 2000)",
            [],
        )
        .map_err(|error| error.to_string())?;
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
    use super::{process_log, resolve_linked_items};
    use crate::infrastructure::database::Database;
    use std::{collections::HashMap, fs};

    #[test]
    fn unknown_longer_item_phrase_does_not_collapse_to_known_suffix() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute("DELETE FROM master_items WHERE item_id=4294", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source) VALUES(17789,'Shackles','test')",
                [],
            )
            .unwrap();

        assert!(resolve_linked_items(
            &connection,
            "'Dusty Rusted Shackles where did that other lizard go'",
            &[],
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn resolves_case_and_apostrophe_variants_to_the_master_item() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source)
                 VALUES(20819,'Elders Earring','test')
                 ON CONFLICT(item_id) DO UPDATE SET item_name=excluded.item_name",
                [],
            )
            .unwrap();

        assert_eq!(
            resolve_linked_items(&connection, "'Elder's Earring'", &[]).unwrap(),
            vec!["Elders Earring"]
        );
        assert_eq!(
            resolve_linked_items(&connection, "'still 7500 for elders earring'", &[]).unwrap(),
            vec!["Elders Earring"]
        );
    }

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

    #[test]
    fn clears_the_entire_group_when_the_local_player_is_removed() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO known_members(name) VALUES('Youngman'),('Posed'),('Nukeman')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO current_group(member_id) SELECT id FROM known_members",
                [],
            )
            .unwrap();
        drop(connection);

        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Mon Aug 03 07:35:16 2026] You have been removed from the group.\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();

        let connection = database.connect().unwrap();
        let active_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let remembered_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM known_members", [], |row| row.get(0))
            .unwrap();
        assert_eq!(active_count, 0);
        assert_eq!(remembered_count, 3);
    }

    #[test]
    fn captures_merchant_activity_only_while_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO master_items(item_id,item_name,source) VALUES(1,'This Item','test'),(2,'That Item','test')",[]).unwrap();
        drop(connection);

        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Mon Aug 03 07:09:18 2026] Trader auctions, 'WTS This Item 1300, That Item'\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let connection = database.connect().unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM merchant_messages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        connection
            .execute(
                "UPDATE app_settings SET value='true' WHERE key='merchant_mode_enabled'",
                [],
            )
            .unwrap();
        drop(connection);

        fs::write(&log, b"[Mon Aug 03 07:09:19 2026] Buyer auctions, 'WTB This Item 1.5k / That Item'\r\n[Mon Aug 03 07:09:20 2026] Buyer tells you, 'Still available?'\r\n").unwrap();
        offsets.insert(log.clone(), 0);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let connection = database.connect().unwrap();
        let messages = connection
            .query_row("SELECT COUNT(*) FROM merchant_messages", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let items = connection
            .query_row("SELECT COUNT(*) FROM merchant_message_items", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let price = connection
            .query_row(
                "SELECT asking_price_pp FROM merchant_message_items ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!((messages, items, price), (2, 2, 1500));
    }

    #[test]
    fn captures_group_and_guild_item_links_with_group_presence() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source)
                 VALUES(1,'Water Sprinkler of Nem Ankh','test')",
                [],
            )
            .unwrap();
        drop(connection);
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let group_link = format!("\u{12}{}A Blue Crown\u{12}", "0".repeat(45));
        let guild_link = format!("\u{12}{}      White Dragon Scale \u{12}", "A".repeat(45));
        fs::write(
            &log,
            format!(
                "[Mon Aug 03 07:16:30 2026] Posed tells the group, '{group_link}'\r\n[Mon Aug 03 07:16:31 2026] Skriz tells the guild, '{guild_link}'\r\n[Thu Aug 27 12:41:43 2026] Dubbyl tells the group, 'Water Sprinkler of Nem Ankh'\r\n[Thu Aug 27 12:41:44 2026] Dubbyl tells the group, 'ordinary conversation'\r\n"
            ),
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();

        let connection = database.connect().unwrap();
        let linked: i64 = connection
            .query_row("SELECT COUNT(*) FROM linked_loot_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        let grouped: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let guild_member_active: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM current_group g JOIN known_members m ON m.id=g.member_id WHERE m.name='Skriz'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let plain_item: String = connection
            .query_row(
                "SELECT item_name FROM linked_loot_items WHERE speaker_name='Dubbyl'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((linked, grouped, guild_member_active), (3, 3, 0));
        assert_eq!(plain_item, "Water Sprinkler of Nem Ankh");
    }
}
