use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration as StdDuration,
};

const WIKI_API: &str = "https://wiki.project1999.com/api.php";
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS spell_info (
 spell_name TEXT PRIMARY KEY COLLATE NOCASE, wiki_url TEXT NOT NULL,
 description TEXT NOT NULL DEFAULT '', classes_json TEXT NOT NULL DEFAULT '[]',
 effects_json TEXT NOT NULL DEFAULT '[]', mana TEXT NOT NULL DEFAULT '',
 skill TEXT NOT NULL DEFAULT '', casting_time TEXT NOT NULL DEFAULT '',
 recast_time TEXT NOT NULL DEFAULT '', fizzle_time TEXT NOT NULL DEFAULT '',
 resist TEXT NOT NULL DEFAULT '', range_value TEXT NOT NULL DEFAULT '',
 target_type TEXT NOT NULL DEFAULT '', spell_type TEXT NOT NULL DEFAULT '',
 duration TEXT NOT NULL DEFAULT '', reagent TEXT NOT NULL DEFAULT '',
 focus TEXT NOT NULL DEFAULT '', where_to_obtain TEXT NOT NULL DEFAULT '',
 fetched_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_spell_info_fetched_at ON spell_info(fetched_at);
CREATE TABLE IF NOT EXISTS spell_catalog_meta (
 key TEXT PRIMARY KEY,
 value TEXT NOT NULL
);
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpellClass {
    pub name: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpellEffect {
    pub slot: u32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellInfo {
    pub spell_name: String,
    pub wiki_url: String,
    pub description: String,
    pub classes: Vec<SpellClass>,
    pub effects: Vec<SpellEffect>,
    pub mana: String,
    pub skill: String,
    pub casting_time: String,
    pub recast_time: String,
    pub fizzle_time: String,
    pub resist: String,
    pub range: String,
    pub target_type: String,
    pub spell_type: String,
    pub duration: String,
    pub reagent: String,
    pub focus: String,
    pub where_to_obtain: String,
    pub fetched_at: String,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellCatalogStatus {
    pub cached_count: i64,
    pub processed: i64,
    pub saved: i64,
    pub failed: i64,
    pub refreshing: bool,
    pub started_at: Option<String>,
    pub last_refresh_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone)]
pub struct SpellCatalog {
    path: PathBuf,
    client: Client,
    certificate_fallback_client: Client,
    refreshing: Arc<AtomicBool>,
}

impl SpellCatalog {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_owned();
        connect(&path)?
            .execute_batch(SCHEMA)
            .map_err(|e| e.to_string())?;
        let client = Client::builder()
            .timeout(StdDuration::from_secs(15))
            .user_agent("EverQuestLootTracker/3.4 (spell metadata cache)")
            .build()
            .map_err(|e| e.to_string())?;
        // Project1999 occasionally serves an incomplete certificate chain. This client is
        // used only after an UnknownIssuer failure and only against the hard-coded wiki API.
        let certificate_fallback_client = Client::builder()
            .timeout(StdDuration::from_secs(15))
            .user_agent("EverQuestLootTracker/3.4 (P99 certificate-chain fallback)")
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path,
            client,
            certificate_fallback_client,
            refreshing: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn start_if_needed(&self) {
        let status = self.status().ok();
        let stale = status
            .as_ref()
            .and_then(|value| value.last_refresh_at.as_deref())
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .is_none_or(|date| date.with_timezone(&Utc) < Utc::now() - Duration::days(30));
        if status.is_none_or(|value| value.cached_count == 0) || stale {
            self.start_refresh();
        }
    }

    pub fn start_refresh(&self) -> SpellCatalogStatus {
        if self
            .refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let catalog = self.clone();
            thread::spawn(move || {
                if let Err(error) = catalog.refresh_all() {
                    let _ = catalog.set_meta("last_error", &error);
                }
                catalog.refreshing.store(false, Ordering::SeqCst);
            });
        }
        self.status().unwrap_or_else(|error| SpellCatalogStatus {
            cached_count: 0,
            processed: 0,
            saved: 0,
            failed: 0,
            refreshing: self.refreshing.load(Ordering::SeqCst),
            started_at: None,
            last_refresh_at: None,
            last_error: Some(error),
        })
    }

    pub fn status(&self) -> Result<SpellCatalogStatus, String> {
        let connection = connect(&self.path)?;
        let cached_count = connection
            .query_row("SELECT COUNT(*) FROM spell_info", [], |row| row.get(0))
            .map_err(|error| error.to_string())?;
        Ok(SpellCatalogStatus {
            cached_count,
            processed: meta_i64(&connection, "refresh_processed")?,
            saved: meta_i64(&connection, "refresh_saved")?,
            failed: meta_i64(&connection, "refresh_failed")?,
            refreshing: self.refreshing.load(Ordering::SeqCst),
            started_at: meta(&connection, "refresh_started_at")?,
            last_refresh_at: meta(&connection, "last_refresh_at")?,
            last_error: meta(&connection, "last_error")?.filter(|value| !value.is_empty()),
        })
    }

    pub fn get(&self, value: &str) -> Result<SpellInfo, String> {
        let name = normalize_spell_name(value).ok_or("A spell name is required")?;
        let cached = self.read_cached(&name)?;
        if cached.as_ref().is_some_and(fresh) {
            return Ok(cached.unwrap());
        }
        match self.fetch(&name) {
            Ok(info) => {
                self.save(&info)?;
                Ok(info)
            }
            Err(error) => cached
                .map(|mut info| {
                    info.stale = true;
                    info
                })
                .ok_or(error),
        }
    }

    fn fetch(&self, name: &str) -> Result<SpellInfo, String> {
        let body = self.get_wiki_json(&[
            ("action", "parse"),
            ("page", name),
            ("prop", "wikitext"),
            ("format", "json"),
        ])?;
        if let Some(message) = body.pointer("/error/info").and_then(Value::as_str) {
            return Err(format!("Project 1999 wiki: {message}"));
        }
        let title = body
            .pointer("/parse/title")
            .and_then(Value::as_str)
            .unwrap_or(name);
        let text = body
            .pointer("/parse/wikitext/*")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("No spell template was found for {name}"))?;
        parse_spell_template(title, text)
    }

    fn read_cached(&self, name: &str) -> Result<Option<SpellInfo>, String> {
        connect(&self.path)?.query_row(
   "SELECT spell_name,wiki_url,description,classes_json,effects_json,mana,skill,casting_time,recast_time,fizzle_time,resist,range_value,target_type,spell_type,duration,reagent,focus,where_to_obtain,fetched_at FROM spell_info WHERE spell_name=?1 COLLATE NOCASE",
   [name], row_to_info).optional().map_err(|e|e.to_string())
    }

    fn save(&self, i: &SpellInfo) -> Result<(), String> {
        save_on(&connect(&self.path)?, i)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        set_meta_on(&connect(&self.path)?, key, value)
    }

    fn refresh_all(&self) -> Result<(), String> {
        let started_at = Utc::now().to_rfc3339();
        self.set_meta("refresh_started_at", &started_at)?;
        self.set_meta("refresh_processed", "0")?;
        self.set_meta("refresh_saved", "0")?;
        self.set_meta("refresh_failed", "0")?;
        self.set_meta("last_error", "")?;
        let mut continuation: Option<String> = None;
        let mut processed = 0_i64;
        let mut saved = 0_i64;
        let mut failed = 0_i64;

        loop {
            let mut query = vec![
                ("action", "query"),
                ("generator", "categorymembers"),
                ("gcmtitle", "Category:Spells"),
                ("gcmnamespace", "0"),
                ("gcmlimit", "50"),
                ("prop", "revisions"),
                ("rvprop", "content"),
                ("format", "json"),
            ];
            if let Some(value) = continuation.as_deref() {
                query.push(("gcmcontinue", value));
            }
            let body = self.get_wiki_json(&query)?;
            if let Some(message) = body.pointer("/error/info").and_then(Value::as_str) {
                return Err(format!("Project 1999 wiki: {message}"));
            }
            let connection = connect(&self.path)?;
            if let Some(pages) = body.pointer("/query/pages").and_then(Value::as_object) {
                for page in pages.values() {
                    processed += 1;
                    let title = page.get("title").and_then(Value::as_str).unwrap_or("");
                    let text = page.pointer("/revisions/0/*").and_then(Value::as_str);
                    match text.and_then(|value| parse_spell_template(title, value).ok()) {
                        Some(info) => match save_on(&connection, &info) {
                            Ok(()) => saved += 1,
                            Err(_) => failed += 1,
                        },
                        None => failed += 1,
                    }
                }
            }
            set_meta_on(&connection, "refresh_processed", &processed.to_string())?;
            set_meta_on(&connection, "refresh_saved", &saved.to_string())?;
            set_meta_on(&connection, "refresh_failed", &failed.to_string())?;
            continuation = body
                .pointer("/query-continue/categorymembers/gcmcontinue")
                .and_then(Value::as_str)
                .map(str::to_string);
            if continuation.is_none() {
                break;
            }
            thread::sleep(StdDuration::from_millis(100));
        }
        self.set_meta("last_refresh_at", &Utc::now().to_rfc3339())?;
        Ok(())
    }

    fn get_wiki_json(&self, query: &[(&str, &str)]) -> Result<Value, String> {
        match request_json(&self.client, query) {
            Ok(value) => Ok(value),
            Err(error) if certificate_chain_error(&error) => {
                request_json(&self.certificate_fallback_client, query).map_err(|fallback| {
                    format!("Project 1999 wiki connection failed after its certificate-chain fallback: {fallback}")
                })
            }
            Err(error) => Err(format!("Project 1999 wiki request failed: {error}")),
        }
    }
}

fn request_json(client: &Client, query: &[(&str, &str)]) -> Result<Value, reqwest::Error> {
    client
        .get(WIKI_API)
        .query(query)
        .send()?
        .error_for_status()?
        .json()
}

fn certificate_chain_error(error: &reqwest::Error) -> bool {
    let detail = format!("{error:?}");
    certificate_chain_detail(&detail)
}

fn certificate_chain_detail(detail: &str) -> bool {
    detail.contains("UnknownIssuer") || detail.contains("InvalidCertificate")
}

fn save_on(connection: &Connection, i: &SpellInfo) -> Result<(), String> {
    connection.execute(
   "INSERT INTO spell_info VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)
    ON CONFLICT(spell_name) DO UPDATE SET wiki_url=excluded.wiki_url,description=excluded.description,classes_json=excluded.classes_json,effects_json=excluded.effects_json,mana=excluded.mana,skill=excluded.skill,casting_time=excluded.casting_time,recast_time=excluded.recast_time,fizzle_time=excluded.fizzle_time,resist=excluded.resist,range_value=excluded.range_value,target_type=excluded.target_type,spell_type=excluded.spell_type,duration=excluded.duration,reagent=excluded.reagent,focus=excluded.focus,where_to_obtain=excluded.where_to_obtain,fetched_at=excluded.fetched_at",
   params![i.spell_name,i.wiki_url,i.description,serde_json::to_string(&i.classes).map_err(|e|e.to_string())?,serde_json::to_string(&i.effects).map_err(|e|e.to_string())?,i.mana,i.skill,i.casting_time,i.recast_time,i.fizzle_time,i.resist,i.range,i.target_type,i.spell_type,i.duration,i.reagent,i.focus,i.where_to_obtain,i.fetched_at]
  ).map_err(|e|e.to_string())?;
    Ok(())
}

fn set_meta_on(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection.execute(
        "INSERT INTO spell_catalog_meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

fn meta(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT value FROM spell_catalog_meta WHERE key=?1",
            [key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

fn meta_i64(connection: &Connection, key: &str) -> Result<i64, String> {
    Ok(meta(connection, key)?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0))
}

fn connect(path: &Path) -> Result<Connection, String> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    c.busy_timeout(StdDuration::from_secs(10))
        .map_err(|e| e.to_string())?;
    Ok(c)
}

fn row_to_info(row: &rusqlite::Row<'_>) -> rusqlite::Result<SpellInfo> {
    let classes: String = row.get(3)?;
    let effects: String = row.get(4)?;
    Ok(SpellInfo {
        spell_name: row.get(0)?,
        wiki_url: row.get(1)?,
        description: row.get(2)?,
        classes: serde_json::from_str(&classes).unwrap_or_default(),
        effects: serde_json::from_str(&effects).unwrap_or_default(),
        mana: row.get(5)?,
        skill: row.get(6)?,
        casting_time: row.get(7)?,
        recast_time: row.get(8)?,
        fizzle_time: row.get(9)?,
        resist: row.get(10)?,
        range: row.get(11)?,
        target_type: row.get(12)?,
        spell_type: row.get(13)?,
        duration: row.get(14)?,
        reagent: row.get(15)?,
        focus: row.get(16)?,
        where_to_obtain: row.get(17)?,
        fetched_at: row.get(18)?,
        stale: false,
    })
}

fn fresh(info: &SpellInfo) -> bool {
    DateTime::parse_from_rfc3339(&info.fetched_at)
        .map(|d| d.with_timezone(&Utc) >= Utc::now() - Duration::days(30))
        .unwrap_or(false)
}

pub fn normalize_spell_name(value: &str) -> Option<String> {
    let prefix = Regex::new(r"(?i)^spell:\s*").unwrap();
    let name = prefix
        .replace(value.trim(), "")
        .replace(['`', '‘', '’', '´'], "'")
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string();
    (!name.is_empty()).then_some(name)
}

fn field<'a>(fields: &'a HashMap<String, String>, name: &str) -> &'a str {
    fields.get(name).map(String::as_str).unwrap_or("")
}

fn clean(value: &str) -> String {
    let links = Regex::new(r"\[\[([^\]|]+)(?:\|\s*([^\]]+))?\]\]").unwrap();
    let markup = Regex::new(r"'{2,5}|<[^>]+>").unwrap();
    let whitespace = Regex::new(r"\s+").unwrap();
    let value = links.replace_all(value, |c: &regex::Captures<'_>| {
        c.get(2)
            .or_else(|| c.get(1))
            .map(|m| m.as_str().trim())
            .unwrap_or("")
            .to_string()
    });
    whitespace
        .replace_all(
            markup
                .replace_all(&value, "")
                .trim()
                .trim_start_matches('*')
                .trim(),
            " ",
        )
        .to_string()
}

fn parse_spell_template(title: &str, text: &str) -> Result<SpellInfo, String> {
    let fields = parse_fields(text);
    if fields.is_empty() {
        return Err(format!(
            "{title} is not a recognized Project 1999 spell page"
        ));
    }
    let class_re = Regex::new(r"(?m)\[\[([^\]|]+)(?:\|[^\]]+)?\]\]\s*-\s*Level\s*(\d+)").unwrap();
    let classes = class_re
        .captures_iter(field(&fields, "classes"))
        .filter_map(|c| {
            Some(SpellClass {
                name: c[1].trim().into(),
                level: c[2].parse().ok()?,
            })
        })
        .collect();
    let effect_re =
        Regex::new(r"(?i)\{\{\s*SpellSlotRow\s*\|\s*(\d+)\s*\|\s*(.*?)\s*\}\}").unwrap();
    let effects = effect_re
        .captures_iter(field(&fields, "slots"))
        .filter_map(|c| {
            Some(SpellEffect {
                slot: c[1].parse().ok()?,
                description: clean(&c[2]),
            })
        })
        .collect();
    let other = field(&fields, "other");
    let extra = |label: &str| {
        Regex::new(&format!(r"(?im)^\s*\*?\s*'{{0,5}}{label}:'{{0,5}}\s*(.+)$"))
            .unwrap()
            .captures(other)
            .map(|c| clean(&c[1]))
            .unwrap_or_default()
    };
    let canonical = clean(field(&fields, "spellname"));
    let name = if canonical.is_empty() {
        title.into()
    } else {
        canonical
    };
    Ok(SpellInfo {
        wiki_url: format!("https://wiki.project1999.com/{}", name.replace(' ', "_")),
        spell_name: name,
        description: clean(field(&fields, "description")),
        classes,
        effects,
        mana: clean(field(&fields, "mana")),
        skill: clean(field(&fields, "skill")),
        casting_time: clean(field(&fields, "casting_time")),
        recast_time: clean(field(&fields, "recast_time")),
        fizzle_time: clean(field(&fields, "fizzle_time")),
        resist: clean(field(&fields, "resist")),
        range: clean(field(&fields, "range")),
        target_type: clean(field(&fields, "target_type")),
        spell_type: clean(field(&fields, "spell_type")),
        duration: clean(field(&fields, "duration")),
        reagent: extra("Reagent"),
        focus: extra("Focus"),
        where_to_obtain: clean(field(&fields, "where_to_obtain")),
        fetched_at: Utc::now().to_rfc3339(),
        stale: false,
    })
}

fn parse_fields(text: &str) -> HashMap<String, String> {
    let start = Regex::new(r"^\|\s*([a-z_]+)\s*=\s*(.*)$").unwrap();
    let mut fields = HashMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.lines() {
        if let Some(capture) = start.captures(line) {
            if let Some((name, value)) = current.take() {
                fields.insert(name, value.trim().to_string());
            }
            current = Some((capture[1].to_string(), capture[2].to_string()));
        } else if let Some((_, value)) = current.as_mut() {
            if line.trim() != "}}" {
                value.push('\n');
                value.push_str(line);
            }
        }
    }
    if let Some((name, value)) = current {
        fields.insert(name, value.trim().to_string());
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    const SERVANT: &str = r#"{{Spellpage|
| spellname = Servant of Bones
| description = Animates an undead servant. Consumes bone chips when cast.
| classes = * [[Necromancer]] - Level 56
| slots = {{SpellSlotRow | 1 | Summon Skeleton Pet: skel_pet_44_ }}
| skill = [[Skill Conjuration | Conjuration]]
| mana = 525
| range =
| casting_time = 15.00
| fizzle_time = 2.25
| recast_time = 2.25
| duration = Instant
| target_type = Self
| spell_type = Beneficial
| resist = Unresistable
| other =
* '''Reagent:''' [[Bone Chips]] x2
* '''Focus:''' [[Encyclopedia Necrotheurgia]]
| where_to_obtain = * Kunark Level 50+ Mob Drop
}}"#;
    #[test]
    fn parses_spell_template() {
        let s = parse_spell_template("Servant of Bones", SERVANT).unwrap();
        assert_eq!(
            s.classes,
            vec![SpellClass {
                name: "Necromancer".into(),
                level: 56
            }]
        );
        assert_eq!(
            s.effects[0].description,
            "Summon Skeleton Pet: skel_pet_44_"
        );
        assert_eq!(s.skill, "Conjuration");
        assert_eq!(s.reagent, "Bone Chips x2");
    }
    #[test]
    fn normalizes_scroll_name() {
        assert_eq!(
            normalize_spell_name(" Spell: Servant of Bones. ").as_deref(),
            Some("Servant of Bones")
        );
        assert_eq!(
            normalize_spell_name("Spell: Atol`s Spectral Shackles").as_deref(),
            Some("Atol's Spectral Shackles")
        );
        assert_eq!(
            normalize_spell_name("Spell: Atol’s Spectral Shackles").as_deref(),
            Some("Atol's Spectral Shackles")
        );
    }
    #[test]
    fn recognizes_the_p99_unknown_issuer_failure_for_scoped_retry() {
        let error = Client::builder()
            .timeout(StdDuration::from_millis(1))
            .build()
            .unwrap()
            .get("https://127.0.0.1:1")
            .send()
            .unwrap_err();
        assert!(!certificate_chain_error(&error));
        assert!(certificate_chain_detail(
            "InvalidCertificate(UnknownIssuer)"
        ));
    }
    #[test]
    fn separate_catalog_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let c = SpellCatalog::open(d.path().join("spell-info.db")).unwrap();
        let s = parse_spell_template("Servant of Bones", SERVANT).unwrap();
        c.save(&s).unwrap();
        assert_eq!(
            c.read_cached("servant of bones").unwrap().unwrap().classes[0].level,
            56
        );
    }

    #[test]
    #[ignore = "requires public Project 1999 wiki access"]
    fn downloads_the_complete_spell_category_in_batches() {
        let directory = tempfile::tempdir().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spell-info.db")).unwrap();
        catalog.refresh_all().unwrap();
        let status = catalog.status().unwrap();
        assert!(status.processed > 1_400, "processed {}", status.processed);
        assert!(
            status.cached_count > 1_300,
            "cached {}",
            status.cached_count
        );
        assert!(status.last_refresh_at.is_some());
    }
}
