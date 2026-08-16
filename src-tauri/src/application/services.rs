use chrono::Local;
use encoding_rs::WINDOWS_1252;
use reqwest::blocking::Client;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use tiny_http::{Header, Response, Server};

use super::data;
use crate::infrastructure::database::Database;

pub fn refresh_market(database: &Database) -> Result<Value, String> {
    let payload: Value = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(err)?
        .get("https://www.pigparse.org/api/item/getall/Green")
        .header("User-Agent", "EverQuestLootTracker/3")
        .send()
        .map_err(err)?
        .error_for_status()
        .map_err(err)?
        .json()
        .map_err(err)?;
    let rows = payload
        .as_array()
        .ok_or("PigParse returned an unexpected response")?;
    if rows.is_empty() {
        return Err("PigParse returned no market values".into());
    }
    let now = Local::now().to_rfc3339();
    let mut c = database.connect().map_err(|e| e.to_string())?;
    let tx = c.transaction().map_err(sql)?;
    tx.execute(
        "DELETE FROM item_market_values WHERE server='Green' COLLATE NOCASE AND is_manual=0",
        [],
    )
    .map_err(sql)?;
    let mut count = 0;
    for row in rows {
        let name = row.get("n").and_then(Value::as_str).unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        tx.execute("INSERT OR IGNORE INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,current_count,current_average_pp,count_30d,average_30d_pp,count_60d,average_60d_pp,count_90d,average_90d_pp,count_6m,average_6m_pp,count_all,average_all_pp,fetched_at,is_manual) VALUES('Green',?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,0)",params![num(row,"i"),num(row,"t"),name,text(row,"l"),num(row,"tc"),num(row,"ta"),num(row,"t30"),num(row,"a30"),num(row,"t60"),num(row,"a60"),num(row,"t90"),num(row,"a90"),num(row,"t6m"),num(row,"a6m"),num(row,"ty"),num(row,"ay"),now]).map_err(sql)?;
        if num(row, "t") == 0 && num(row, "i") > 0 {
            tx.execute("INSERT OR IGNORE INTO master_items(item_id,item_name,source,updated_at) VALUES(?,?,'market',?)",params![num(row,"i"),name,now]).map_err(sql)?;
        }
        count += 1;
    }
    tx.commit().map_err(sql)?;
    Ok(json!({"count":count}))
}

pub fn upload_exports(database: &Database) -> Result<Value, String> {
    let c = database.connect().map_err(|e| e.to_string())?;
    let directory: String = c
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Choose the EverQuest Logs folder first".to_string())?;
    let directory = output_directory(Path::new(&directory));
    let mut files = Vec::new();
    for entry in fs::read_dir(&directory)
        .map_err(err)?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let name = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
        let lower = name.to_ascii_lowercase();
        if lower.ends_with("-inventory.txt") || lower.ends_with("-spellbook.txt") {
            let bytes = fs::read(&path).map_err(err)?;
            if bytes.len() > 128 * 1024 {
                continue;
            }
            files.push(json!({"name":name,"text":String::from_utf8_lossy(&bytes)}));
        }
    }
    if files.is_empty() {
        return Err("No inventory or spellbook exports found".into());
    }
    upload_files(database, files)
}

pub fn upload_file_payloads(database: &Database, payload: &Value) -> Result<Value, String> {
    let input = payload
        .get("files")
        .and_then(Value::as_array)
        .ok_or("files are required")?;
    if input.is_empty() {
        return Err("Import at least one file before pushing to Planner".into());
    }
    let mut files = Vec::new();
    let mut total = 0usize;
    for file in input {
        let name = file
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let text = file
            .get("text")
            .and_then(Value::as_str)
            .ok_or("file text is required")?;
        let lower = name.to_ascii_lowercase();
        if !(lower.ends_with("-inventory.txt") || lower.ends_with("-spellbook.txt")) {
            return Err(format!("Unsupported export filename: {name}"));
        }
        if text.len() > 128 * 1024 {
            return Err(format!("{name} exceeds the 128 KB Planner limit"));
        }
        total += text.len();
        if total > 256 * 1024 {
            return Err("The selected files exceed the 256 KB Planner limit".into());
        }
        files.push(json!({"name":name,"text":text}));
    }
    upload_files(database, files)
}

fn upload_files(database: &Database, files: Vec<Value>) -> Result<Value, String> {
    let c = database.connect().map_err(|e| e.to_string())?;
    let claim: Option<String> = c
        .query_row(
            "SELECT value FROM app_settings WHERE key='planner_import_claim'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    let token = claim
        .as_deref()
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .and_then(|v| v.get("token").and_then(Value::as_str).map(str::to_owned));
    let client = Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(err)?;
    let request = |token: Option<&str>| {
        let url = token
            .map(|t| format!("https://p99planner.com/api/import/{t}"))
            .unwrap_or_else(|| "https://p99planner.com/api/import".into());
        let builder = if token.is_some() {
            client.put(url)
        } else {
            client.post(url)
        };
        builder
            .header("User-Agent", "EverQuestLootTracker/3")
            .json(&json!({"files":files}))
            .send()
    };
    let mut response = request(token.as_deref()).map_err(err)?;
    if response.status().as_u16() == 410 {
        response = request(None).map_err(err)?;
    }
    let response = response.error_for_status().map_err(err)?;
    let value: Value = response.json().map_err(err)?;
    let url = text(&value, "url");
    let stored = json!({"token":text(&value,"token"),"url":url,"expires":text(&value,"expires")})
        .to_string();
    c.execute("INSERT INTO app_settings(key,value) VALUES('planner_import_claim',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[stored]).map_err(sql)?;
    c.execute("INSERT INTO app_settings(key,value) VALUES('planner_import_url',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[&url]).map_err(sql)?;
    for file in &files {
        c.execute(
            "INSERT INTO import_uploads(file_name,status,review_url) VALUES(?,'uploaded',?)",
            params![text(file, "name"), url],
        )
        .map_err(sql)?;
    }
    Ok(json!({"url":url,"files":files.len()}))
}

pub fn output_directory(logs_directory: &Path) -> PathBuf {
    if logs_directory
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("logs"))
    {
        logs_directory
            .parent()
            .unwrap_or(logs_directory)
            .to_path_buf()
    } else {
        logs_directory.to_path_buf()
    }
}

pub fn export_wts(database: &Database, group_id: i64) -> Result<Value, String> {
    let c = database.connect().map_err(|e| e.to_string())?;
    let character: String = c
        .query_row(
            "SELECT character_name FROM wts_groups WHERE id=?",
            [group_id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let directory: String = c
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "Choose the Logs folder first".to_string())?;
    let ini = find_ini(Path::new(&directory), &character)?;
    let groups = {
        let mut st=c.prepare("SELECT id,name FROM wts_groups WHERE character_name=? COLLATE NOCASE ORDER BY created_at,id").map_err(sql)?;
        let values = st
            .query_map([&character], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(sql)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        values
    };
    if groups.len() > 12 {
        return Err("Page 10 supports at most 12 WTS groups per character".into());
    }
    for (index, (id, name)) in groups.iter().enumerate() {
        let items = {
            let mut st=c.prepare("SELECT w.item_name,COALESCE((SELECT m.item_id FROM master_items m WHERE m.item_name=w.item_name COLLATE NOCASE LIMIT 1),w.item_id) FROM wts_group_items w WHERE w.wts_group_id=? ORDER BY w.sort_order").map_err(sql)?;
            let values = st
                .query_map([id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                })
                .map_err(sql)?
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            values
        };
        write_social(&ini, index + 1, name, &items)?;
    }
    let button = groups
        .iter()
        .position(|(id, _)| *id == group_id)
        .unwrap_or(0)
        + 1;
    Ok(
        json!({"iniPath":ini.display().to_string(),"buttonNumber":button,"buttonsWritten":groups.len()}),
    )
}

pub fn backup(database: &Database) -> Result<Value, String> {
    let source = database_path(database)?;
    let backup = source.with_file_name(format!(
        "loot-tracker-backup-{}.db",
        Local::now().format("%Y%m%d-%H%M%S")
    ));
    let c = database.connect().map_err(|e| e.to_string())?;
    let _ = c.execute_batch("PRAGMA wal_checkpoint(FULL);");
    fs::copy(&source, &backup).map_err(err)?;
    Ok(json!({"path":backup.display().to_string()}))
}

pub fn restore(database: &Database, backup_path: &str) -> Result<Value, String> {
    let backup = PathBuf::from(backup_path);
    if !backup.is_file() {
        return Err("The selected backup file does not exist".into());
    }
    let header = fs::read(&backup).map_err(err)?;
    if !header.starts_with(b"SQLite format 3\0") {
        return Err("The selected file is not a SQLite database".into());
    }
    let target = database_path(database)?;
    if backup == target {
        return Err("Choose a backup file, not the active database".into());
    }
    {
        let c = database.connect().map_err(|e| e.to_string())?;
        c.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(sql)?;
    }
    fs::copy(&backup, &target).map_err(err)?;
    database.migrate().map_err(|e| e.to_string())?;
    Ok(json!({"path":target.display().to_string(),"restoredFrom":backup.display().to_string()}))
}
fn database_path(database: &Database) -> Result<PathBuf, String> {
    let c = database.connect().map_err(|e| e.to_string())?;
    let path: String = c
        .query_row("PRAGMA database_list", [], |r| r.get(2))
        .map_err(sql)?;
    Ok(PathBuf::from(path))
}

pub fn start_web(database_path: PathBuf) {
    thread::spawn(move || {
        let Ok(server) = Server::http("127.0.0.1:8765") else {
            return;
        };
        if let Ok(db) = Database::open(&database_path) {
            if let Ok(c) = db.connect() {
                let _=c.execute("INSERT INTO app_settings(key,value) VALUES('web_url','http://127.0.0.1:8765/') ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]);
            }
            for request in server.incoming_requests() {
                let body = data::snapshot(&db)
                    .map(render_dashboard)
                    .unwrap_or_else(|e| format!("<h1>Loot Tracker</h1><pre>{}</pre>", escape(&e)));
                let mut response = Response::from_string(body);
                if let Ok(header) = Header::from_bytes("Content-Type", "text/html; charset=utf-8") {
                    response = response.with_header(header)
                }
                let _ = request.respond(response);
            }
        }
    });
}
fn render_dashboard(v: Value) -> String {
    let rows = |key: &str, cols: &[(&str, &str)]| {
        v.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|r| {
                format!(
                    "<tr>{}</tr>",
                    cols.iter()
                        .map(|(k, _)| format!(
                            "<td>{}</td>",
                            escape(&r.get(*k).map(display).unwrap_or_default())
                        ))
                        .collect::<String>()
                )
            })
            .collect::<String>()
    };
    let table = |title: &str, key: &str, cols: &[(&str, &str)]| {
        format!("<section><h2>{title}</h2><input placeholder='Filter…' oninput=\"let q=this.value.toLowerCase();this.nextElementSibling.querySelectorAll('tbody tr').forEach(r=>r.hidden=!r.innerText.toLowerCase().includes(q))\"><table><thead><tr>{}</tr></thead><tbody>{}</tbody></table></section>",cols.iter().map(|(_,l)|format!("<th>{l}</th>")).collect::<String>(),rows(key,cols))
    };
    format!("<!doctype html><html><head><meta charset=utf-8><title>EverQuest Loot Tracker</title><style>body{{font:14px system-ui;background:#0b1017;color:#e7edf4;margin:0;padding:30px}}h1{{margin-top:0}}nav a{{color:#62b6ff;margin-right:16px}}section{{background:#111923;border:1px solid #263443;border-radius:14px;padding:18px;margin:18px 0}}input{{background:#0b1017;color:white;border:1px solid #263443;border-radius:8px;padding:9px;width:320px}}table{{width:100%;border-collapse:collapse;margin-top:10px}}th,td{{text-align:left;border-top:1px solid #263443;padding:9px}}</style></head><body><h1>EverQuest Loot Tracker</h1><p>Read-only local dashboard · refresh for current data</p>{}{}{}<section><h2>Compound workspace</h2><pre>{}</pre></section></body></html>",table("Recent loot","loot",&[("itemName","Item"),("mobName","Dropped by"),("looterName","Looted by"),("valuePp","Value"),("happenedAt","When")]),table("Active splits","splits",&[("itemName","Item"),("looterName","Held by"),("payoutValuePp","Value"),("attendees","Shared by")]),table("Sold & consumed","history",&[("itemName","Item"),("disposition","Result"),("valuePp","Value"),("note","Note")]),escape(&serde_json::to_string_pretty(v.get("compound").unwrap_or(&Value::Null)).unwrap_or_default()))
}

fn find_ini(logs: &Path, character: &str) -> Result<PathBuf, String> {
    let filename = format!("{character}_P1999Green.ini");
    for dir in [logs.parent().unwrap_or(logs), logs] {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.filter_map(Result::ok) {
                if e.file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&filename)
                {
                    return Ok(e.path());
                }
            }
        }
    }
    Err(format!("Could not find {filename} beside the Logs folder"))
}
fn write_social(
    path: &Path,
    button: usize,
    name: &str,
    items: &[(String, Option<i64>)],
) -> Result<(), String> {
    let original = fs::read(path).map_err(err)?;
    let newline = if original.windows(2).any(|v| v == b"\r\n") {
        b"\r\n".as_slice()
    } else {
        b"\n".as_slice()
    };
    let mut lines = original
        .split_inclusive(|b| *b == b'\n')
        .map(|v| v.to_vec())
        .collect::<Vec<_>>();
    let mut start = None;
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let text = String::from_utf8_lossy(line).trim().to_ascii_lowercase();
        if text == "[socials]" {
            start = Some(i)
        } else if start.is_some() && text.starts_with('[') {
            end = i;
            break;
        }
    }
    if start.is_none() {
        if !original.ends_with(b"\n") {
            lines.push(newline.to_vec())
        }
        lines.push([b"[Socials]".as_slice(), newline].concat());
        start = Some(lines.len() - 1);
        end = lines.len();
    }
    let prefix = format!("Page10Button{button}");
    let auction = auction_bytes(items);
    let entries = [
        (format!("{prefix}Name"), format!("WTS{button}").into_bytes()),
        (format!("{prefix}Color"), b"0".to_vec()),
        (format!("{prefix}Line1"), auction),
    ];
    for (key, value) in entries {
        let mut found = false;
        for line in lines.iter_mut().take(end).skip(start.unwrap() + 1) {
            let raw = String::from_utf8_lossy(line);
            if raw
                .to_ascii_lowercase()
                .starts_with(&format!("{}=", key.to_ascii_lowercase()))
            {
                *line = [key.as_bytes(), b"=", &value, newline].concat();
                found = true;
                break;
            }
        }
        if !found {
            lines.insert(end, [key.as_bytes(), b"=", &value, newline].concat());
            end += 1;
        }
    }
    fs::write(path, lines.concat()).map_err(err)?;
    let _ = name;
    Ok(())
}
fn auction_bytes(items: &[(String, Option<i64>)]) -> Vec<u8> {
    let mut out = b"/auction WTS ".to_vec();
    for (index, (name, id)) in items.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(b" / ")
        }
        let (encoded, _, _) = WINDOWS_1252.encode(name);
        if let Some(id) = id.filter(|v| *v > 0) {
            let mut metadata = format!("00{:X}", id).into_bytes();
            metadata.resize(45, b'0');
            metadata.truncate(45);
            out.push(0x12);
            out.extend(metadata);
            out.extend_from_slice(b"      ");
            out.extend_from_slice(&encoded);
            out.extend_from_slice(b" \x12");
        } else {
            out.extend_from_slice(&encoded)
        }
    }
    out
}
fn num(v: &Value, k: &str) -> i64 {
    v.get(k)
        .and_then(|x| x.as_i64().or_else(|| x.as_str()?.parse().ok()))
        .unwrap_or(0)
}
fn text(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_owned()
}
fn display(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(display).collect::<Vec<_>>().join(", "),
        _ => v.to_string(),
    }
}
fn escape(v: &str) -> String {
    v.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}
fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::{auction_bytes, output_directory, write_social};
    use std::path::Path;

    #[test]
    fn exports_are_watched_beside_the_logs_folder() {
        let everquest = Path::new("EverQuest");
        assert_eq!(output_directory(&everquest.join("Logs")), everquest);

        let exports = Path::new("Exports");
        assert_eq!(output_directory(exports), exports);
    }

    #[test]
    fn auction_uses_clickable_titanium_item_link_without_angle_brackets() {
        let value = auction_bytes(&[("Tears of Prexus".into(), Some(13047))]);
        assert_eq!(
            value,
            b"/auction WTS \x120032F7000000000000000000000000000000000000000      Tears of Prexus \x12"
        );
        assert!(!value.contains(&b'<') && !value.contains(&b'>'));
    }

    #[test]
    fn social_writer_only_changes_allowed_page_ten_keys() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Khards_P1999Green.ini");
        let original = b"[Friends]\r\nBinary=\xff\r\n[Socials]\r\nPage2Button1Name=Map\r\nPage10Button1Name=Old\r\nPage10Button1Color=17\r\nPage10Button1Line1=/say old\r\n[Next]\r\nValue=1\r\n";
        std::fs::write(&path, original).unwrap();
        write_social(
            &path,
            1,
            "Tunnel sale",
            &[("Tears of Prexus".into(), Some(13047))],
        )
        .unwrap();
        let actual = std::fs::read(path).unwrap();
        let mut expected = original.to_vec();
        replace(
            &mut expected,
            b"Page10Button1Name=Old",
            b"Page10Button1Name=WTS1",
        );
        replace(
            &mut expected,
            b"Page10Button1Color=17",
            b"Page10Button1Color=0",
        );
        replace(&mut expected,b"Page10Button1Line1=/say old",b"Page10Button1Line1=/auction WTS \x120032F7000000000000000000000000000000000000000      Tears of Prexus \x12");
        assert_eq!(actual, expected);
    }

    fn replace(value: &mut Vec<u8>, old: &[u8], new: &[u8]) {
        let start = value
            .windows(old.len())
            .position(|window| window == old)
            .unwrap();
        value.splice(start..start + old.len(), new.iter().copied());
    }
}
