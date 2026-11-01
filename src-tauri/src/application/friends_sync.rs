use chrono::Local;
use encoding_rs::WINDOWS_1252;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::infrastructure::database::Database;

use super::services::output_directory;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsSyncCharacter {
    pub character: String,
    pub server: String,
    pub file_name: String,
    pub friends: Vec<String>,
    pub has_friends_section: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsSyncSnapshot {
    pub directory: String,
    pub characters: Vec<FriendsSyncCharacter>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsSyncRequest {
    pub source_file_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsEditRequest {
    pub source_file_name: String,
    pub friends: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsEditResult {
    pub source_character: String,
    pub friend_count: usize,
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendsSyncResult {
    pub source_character: String,
    pub friend_count: usize,
    pub applied_count: usize,
    pub target_characters: Vec<String>,
    pub backup_directory: String,
}

#[derive(Debug)]
struct IniCharacter {
    character: String,
    server: String,
    path: PathBuf,
    bytes: Vec<u8>,
    section: Option<(usize, usize)>,
}

pub fn snapshot(database: &Database) -> Result<FriendsSyncSnapshot, String> {
    let directory = everquest_directory(database)?;
    let characters = discover(&directory)?
        .into_iter()
        .map(|ini| FriendsSyncCharacter {
            character: ini.character,
            server: ini.server,
            file_name: ini
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            friends: ini
                .section
                .map(|bounds| friend_names(&ini.bytes[bounds.0..bounds.1]))
                .unwrap_or_default(),
            has_friends_section: ini.section.is_some(),
        })
        .collect();
    Ok(FriendsSyncSnapshot {
        directory: directory.display().to_string(),
        characters,
    })
}

pub fn apply(
    database: &Database,
    request: FriendsSyncRequest,
) -> Result<FriendsSyncResult, String> {
    let directory = everquest_directory(database)?;
    let characters = discover(&directory)?;
    let source = characters
        .iter()
        .find(|entry| {
            entry.path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .eq_ignore_ascii_case(request.source_file_name.trim())
            })
        })
        .ok_or_else(|| {
            format!(
                "Could not find the source character INI {}",
                request.source_file_name.trim()
            )
        })?;
    let source_bounds = source
        .section
        .ok_or_else(|| format!("{} does not have a [Friends] section", source.character))?;
    let source_section = source.bytes[source_bounds.0..source_bounds.1].to_vec();
    let friends = friend_names(&source_section);
    let targets = characters
        .iter()
        .filter(|entry| entry.path != source.path)
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Err("No other character INI files were found".to_owned());
    }

    let backup_directory = directory.join("loot-tracker-backups").join(format!(
        "friends-sync-{}",
        Local::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    fs::create_dir_all(&backup_directory).map_err(error)?;
    for target in &targets {
        fs::copy(
            &target.path,
            backup_directory.join(target.path.file_name().unwrap_or_default()),
        )
        .map_err(error)?;
    }

    let mut written: Vec<PathBuf> = Vec::new();
    for target in &targets {
        let updated = replace_friends_section(&target.bytes, target.section, &source_section);
        if let Err(write_error) = fs::write(&target.path, updated) {
            for path in &written {
                if let Some(file_name) = path.file_name() {
                    let _ = fs::copy(backup_directory.join(file_name), path);
                }
            }
            return Err(format!(
                "Friends sync failed while writing {}. Previously changed files were restored: {write_error}",
                target.path.display()
            ));
        }
        written.push(target.path.clone());
    }

    Ok(FriendsSyncResult {
        source_character: source.character.clone(),
        friend_count: friends.len(),
        applied_count: targets.len(),
        target_characters: targets
            .iter()
            .map(|entry| entry.character.clone())
            .collect(),
        backup_directory: backup_directory.display().to_string(),
    })
}

pub fn update(
    database: &Database,
    request: FriendsEditRequest,
) -> Result<FriendsEditResult, String> {
    let directory = everquest_directory(database)?;
    let characters = discover(&directory)?;
    let source = characters
        .iter()
        .find(|entry| {
            entry.path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .eq_ignore_ascii_case(request.source_file_name.trim())
            })
        })
        .ok_or_else(|| {
            format!(
                "Could not find the source character INI {}",
                request.source_file_name.trim()
            )
        })?;
    let bounds = source
        .section
        .ok_or_else(|| format!("{} does not have a [Friends] section", source.character))?;
    let friends = validate_friend_names(&request.friends)?;
    let section = render_friends_section(&source.bytes[bounds.0..bounds.1], &friends)?;
    let updated = replace_friends_section(&source.bytes, source.section, &section);

    let backup_directory = directory.join("loot-tracker-backups").join(format!(
        "friends-edit-{}",
        Local::now().format("%Y%m%d-%H%M%S-%3f")
    ));
    fs::create_dir_all(&backup_directory).map_err(error)?;
    let backup_path = backup_directory.join(source.path.file_name().unwrap_or_default());
    fs::copy(&source.path, &backup_path).map_err(error)?;
    if let Err(write_error) = fs::write(&source.path, updated) {
        let _ = fs::copy(&backup_path, &source.path);
        return Err(format!(
            "Friends edit failed while writing {}. The original file was restored: {write_error}",
            source.path.display()
        ));
    }

    Ok(FriendsEditResult {
        source_character: source.character.clone(),
        friend_count: friends.len(),
        backup_path: backup_path.display().to_string(),
    })
}

fn everquest_directory(database: &Database) -> Result<PathBuf, String> {
    let connection = database.connect().map_err(error)?;
    let logs_directory: String = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "Choose the Logs folder on the System page first".to_owned())?;
    let directory = output_directory(Path::new(&logs_directory));
    if !directory.is_dir() {
        return Err(format!(
            "EverQuest directory does not exist: {}",
            directory.display()
        ));
    }
    Ok(directory)
}

fn discover(directory: &Path) -> Result<Vec<IniCharacter>, String> {
    let mut values = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(error)?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("ini"))
        {
            continue;
        }
        let Some((character, server)) = character_and_server(&path) else {
            continue;
        };
        let bytes = fs::read(&path).map_err(error)?;
        values.push(IniCharacter {
            character,
            server,
            section: section_bounds(&bytes, b"friends"),
            path,
            bytes,
        });
    }
    values.sort_by(|left, right| {
        left.character
            .to_ascii_lowercase()
            .cmp(&right.character.to_ascii_lowercase())
            .then_with(|| left.server.cmp(&right.server))
    });
    Ok(values)
}

fn character_and_server(path: &Path) -> Option<(String, String)> {
    let stem = path.file_stem()?.to_str()?;
    if stem.to_ascii_lowercase().starts_with("ui_") {
        return None;
    }
    let (character, server) = stem.split_once('_')?;
    if character.is_empty()
        || server.is_empty()
        || !character.chars().all(|value| value.is_ascii_alphabetic())
    {
        return None;
    }
    Some((character.to_owned(), server.to_owned()))
}

fn section_bounds(bytes: &[u8], section: &[u8]) -> Option<(usize, usize)> {
    let mut cursor = 0;
    let mut start = None;
    while cursor < bytes.len() {
        let line_start = cursor;
        cursor = bytes[cursor..]
            .iter()
            .position(|value| *value == b'\n')
            .map(|offset| cursor + offset + 1)
            .unwrap_or(bytes.len());
        let line = trim_ascii(&bytes[line_start..cursor]);
        let is_header =
            line.len() >= 2 && line.first() == Some(&b'[') && line.last() == Some(&b']');
        if is_header {
            if let Some(section_start) = start {
                return Some((section_start, line_start));
            }
            if line[1..line.len() - 1].eq_ignore_ascii_case(section) {
                start = Some(line_start);
            }
        }
    }
    start.map(|value| (value, bytes.len()))
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn friend_names(section: &[u8]) -> Vec<String> {
    section
        .split(|value| *value == b'\n')
        .filter_map(|line| {
            let line = trim_ascii(line);
            if line.first() == Some(&b'[') {
                return None;
            }
            let equals = line.iter().position(|value| *value == b'=')?;
            let key = trim_ascii(&line[..equals]);
            if !is_friend_key(key) {
                return None;
            }
            let raw = trim_ascii(&line[equals + 1..]);
            if raw.is_empty() || raw.eq_ignore_ascii_case(b"*NULL*") {
                return None;
            }
            let (name, _, _) = WINDOWS_1252.decode(raw);
            Some(name.into_owned())
        })
        .collect()
}

fn validate_friend_names(values: &[String]) -> Result<Vec<String>, String> {
    let mut friends = Vec::with_capacity(values.len());
    for value in values {
        let name = value.trim();
        if name.is_empty() {
            return Err("Friend names cannot be empty".to_owned());
        }
        if name.eq_ignore_ascii_case("*NULL*")
            || name.len() > 64
            || !name
                .chars()
                .all(|character| character.is_ascii_alphabetic())
        {
            return Err(format!(
                "{name} is not a valid EverQuest character name; use letters only"
            ));
        }
        if friends
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(name))
        {
            return Err(format!("{name} is already in the Friends list"));
        }
        friends.push(name.to_owned());
    }
    Ok(friends)
}

fn render_friends_section(section: &[u8], friends: &[String]) -> Result<Vec<u8>, String> {
    let encoded = friends
        .iter()
        .map(|name| {
            let (value, _, had_errors) = WINDOWS_1252.encode(name);
            if had_errors {
                Err(format!("{name} cannot be written to an EverQuest INI file"))
            } else {
                Ok(value.into_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let slot_count = section
        .split(|value| *value == b'\n')
        .filter(|line| {
            line.iter()
                .position(|value| *value == b'=')
                .is_some_and(|equals| is_friend_key(trim_ascii(&line[..equals])))
        })
        .count();
    if encoded.len() > slot_count {
        return Err(format!(
            "The Friends section has {slot_count} slots but {} names were provided",
            encoded.len()
        ));
    }

    let mut slot = 0;
    let mut output = Vec::with_capacity(section.len());
    for line in section.split_inclusive(|value| *value == b'\n') {
        let content_end = line
            .iter()
            .rposition(|value| !matches!(*value, b'\r' | b'\n'))
            .map(|index| index + 1)
            .unwrap_or(0);
        let content = &line[..content_end];
        let newline = &line[content_end..];
        let Some(equals) = content.iter().position(|value| *value == b'=') else {
            output.extend_from_slice(line);
            continue;
        };
        if !is_friend_key(trim_ascii(&content[..equals])) {
            output.extend_from_slice(line);
            continue;
        }
        output.extend_from_slice(&content[..=equals]);
        output.extend_from_slice(encoded.get(slot).map(Vec::as_slice).unwrap_or(b"*NULL*"));
        output.extend_from_slice(newline);
        slot += 1;
    }
    Ok(output)
}

fn replace_friends_section(
    target: &[u8],
    bounds: Option<(usize, usize)>,
    source_section: &[u8],
) -> Vec<u8> {
    match bounds {
        Some((start, end)) => [
            target[..start].as_ref(),
            source_section,
            target[end..].as_ref(),
        ]
        .concat(),
        None => {
            let separator: &[u8] = if target.is_empty() || target.ends_with(b"\n") {
                b""
            } else if target.windows(2).any(|value| value == b"\r\n") {
                b"\r\n"
            } else {
                b"\n"
            };
            [target, separator, source_section].concat()
        }
    }
}

fn is_friend_key(value: &[u8]) -> bool {
    starts_with_ignore_ascii_case(value, b"friend")
        && value
            .get(b"friend".len()..)
            .is_some_and(|suffix| !suffix.is_empty() && suffix.iter().all(u8::is_ascii_digit))
}

fn starts_with_ignore_ascii_case(value: &[u8], prefix: &[u8]) -> bool {
    value
        .get(..prefix.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
}

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

#[cfg(test)]
mod tests {
    use super::{character_and_server, friend_names, replace_friends_section, section_bounds};
    use std::path::Path;

    #[test]
    fn copies_only_the_friends_section_and_preserves_binary_content() {
        let source = b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\n[Socials]\r\nSource=1\r\n";
        let target = b"[Friends]\r\nFriend0=Old\r\n[Socials]\r\nBinary=\xff\r\n";
        let source_bounds = section_bounds(source, b"friends").unwrap();
        let actual = replace_friends_section(
            target,
            section_bounds(target, b"friends"),
            &source[source_bounds.0..source_bounds.1],
        );
        assert_eq!(
            actual,
            b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\n[Socials]\r\nBinary=\xff\r\n"
        );
        let bounds = section_bounds(&actual, b"friends").unwrap();
        assert_eq!(
            friend_names(&actual[bounds.0..bounds.1]),
            vec!["Alice", "Bob"]
        );
    }

    #[test]
    fn appends_friends_when_target_has_no_section() {
        let source = b"[Friends]\nFriend0=Alice\n";
        assert_eq!(
            replace_friends_section(b"[Socials]\nValue=1", None, source),
            b"[Socials]\nValue=1\n[Friends]\nFriend0=Alice\n"
        );
    }

    #[test]
    fn discovers_character_files_but_not_ui_layouts() {
        assert_eq!(
            character_and_server(Path::new("Derpsmonk_P1999Green.ini")),
            Some(("Derpsmonk".into(), "P1999Green".into()))
        );
        assert_eq!(
            character_and_server(Path::new("UI_Derpsmonk_P1999Green.ini")),
            None
        );
        assert_eq!(character_and_server(Path::new("eqclient.ini")), None);
    }

    #[test]
    fn syncs_all_targets_with_exact_backups_and_leaves_source_untouched() {
        use super::{apply, snapshot, FriendsSyncRequest};
        use crate::infrastructure::database::Database;
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let logs = directory.path().join("Logs");
        fs::create_dir(&logs).unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [logs.display().to_string()],
            )
            .unwrap();

        let source_path = directory.path().join("Source_P1999Green.ini");
        let target_path = directory.path().join("Target_P1999Green.ini");
        let source = b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\n[Socials]\r\nSource=1\r\n";
        let target = b"[Friends]\r\nFriend0=Old\r\n[Socials]\r\nBinary=\xff\r\n";
        fs::write(&source_path, source).unwrap();
        fs::write(&target_path, target).unwrap();
        fs::write(
            directory.path().join("UI_Source_P1999Green.ini"),
            b"[Friends]\nFriend0=Ignored\n",
        )
        .unwrap();

        let before = snapshot(&database).unwrap();
        assert_eq!(before.characters.len(), 2);
        let result = apply(
            &database,
            FriendsSyncRequest {
                source_file_name: "Source_P1999Green.ini".into(),
            },
        )
        .unwrap();

        assert_eq!((result.friend_count, result.applied_count), (2, 1));
        assert_eq!(fs::read(source_path).unwrap(), source);
        assert_eq!(
            fs::read(target_path).unwrap(),
            b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\n[Socials]\r\nBinary=\xff\r\n"
        );
        assert_eq!(
            fs::read(Path::new(&result.backup_directory).join("Target_P1999Green.ini")).unwrap(),
            target
        );
    }

    #[test]
    fn renders_friend_slots_without_touching_other_lines() {
        use super::{friend_names, render_friends_section, validate_friend_names};

        let section = b"[Friends]\r\nFriend0=Old\r\nFriend1=*NULL*\r\nFriend2=*NULL*\r\nFriendlyFire=keep\r\n";
        let friends = validate_friend_names(&["Alice".into(), "Bob".into()]).unwrap();
        let rendered = render_friends_section(section, &friends).unwrap();
        assert_eq!(
            rendered,
            b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\nFriendlyFire=keep\r\n"
        );
        assert_eq!(friend_names(&rendered), vec!["Alice", "Bob"]);
        assert!(validate_friend_names(&["Alice".into(), "alice".into()]).is_err());
        assert!(validate_friend_names(&["Bad Name".into()]).is_err());
    }

    #[test]
    fn edits_one_source_with_an_exact_backup() {
        use super::{update, FriendsEditRequest};
        use crate::infrastructure::database::Database;
        use std::fs;

        let directory = tempfile::tempdir().unwrap();
        let logs = directory.path().join("Logs");
        fs::create_dir(&logs).unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [logs.display().to_string()],
            )
            .unwrap();

        let source_path = directory.path().join("Source_P1999Green.ini");
        let original = b"[Friends]\r\nFriend0=Old\r\nFriend1=*NULL*\r\nFriend2=*NULL*\r\n[Ignored]\r\nIgnored0=Keep\r\n[Socials]\r\nBinary=\xff\r\n";
        fs::write(&source_path, original).unwrap();

        let result = update(
            &database,
            FriendsEditRequest {
                source_file_name: "Source_P1999Green.ini".into(),
                friends: vec!["Alice".into(), "Bob".into()],
            },
        )
        .unwrap();

        assert_eq!(result.friend_count, 2);
        assert_eq!(fs::read(result.backup_path).unwrap(), original);
        assert_eq!(
            fs::read(source_path).unwrap(),
            b"[Friends]\r\nFriend0=Alice\r\nFriend1=Bob\r\nFriend2=*NULL*\r\n[Ignored]\r\nIgnored0=Keep\r\n[Socials]\r\nBinary=\xff\r\n"
        );
    }
}
