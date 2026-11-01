use chrono::NaiveDateTime;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const SETTING_KEY: &str = "eq_time_sync";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqTimeSync {
    pub game_minutes: u16,
    pub synchronized_at: String,
    pub game_date: String,
    pub character: String,
}

pub fn record(
    connection: &Connection,
    happened_at: NaiveDateTime,
    game_date: &str,
    game_minutes: u16,
    character: &str,
) -> Result<(), String> {
    if let Some(current) = snapshot(connection)? {
        let current_time =
            NaiveDateTime::parse_from_str(&current.synchronized_at, "%Y-%m-%d %H:%M:%S").ok();
        if current_time.is_some_and(|current_time| current_time > happened_at) {
            return Ok(());
        }
    }
    let sync = EqTimeSync {
        game_minutes,
        synchronized_at: happened_at.to_string(),
        game_date: game_date.to_owned(),
        character: character.to_owned(),
    };
    let value = serde_json::to_string(&sync).map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES(?,?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![SETTING_KEY, value],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn snapshot(connection: &Connection) -> Result<Option<EqTimeSync>, String> {
    let value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key=?",
            [SETTING_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(value.and_then(|value| serde_json::from_str(&value).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_loads_a_typed_time_sync() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch("CREATE TABLE app_settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);")
            .unwrap();
        let happened_at =
            NaiveDateTime::parse_from_str("2026-10-28 08:53:48", "%Y-%m-%d %H:%M:%S").unwrap();
        record(
            &connection,
            happened_at,
            "Tuesday, September 17, 3253",
            360,
            "Utrido",
        )
        .unwrap();
        assert_eq!(
            snapshot(&connection).unwrap(),
            Some(EqTimeSync {
                game_minutes: 360,
                synchronized_at: "2026-10-28 08:53:48".into(),
                game_date: "Tuesday, September 17, 3253".into(),
                character: "Utrido".into(),
            })
        );

        let older =
            NaiveDateTime::parse_from_str("2026-10-28 08:52:00", "%Y-%m-%d %H:%M:%S").unwrap();
        record(
            &connection,
            older,
            "Tuesday, September 17, 3253",
            300,
            "Olderlog",
        )
        .unwrap();
        assert_eq!(snapshot(&connection).unwrap().unwrap().game_minutes, 360);
    }
}
