use rusqlite::{params, OptionalExtension};
use serde::Serialize;

use crate::{domain::log_events::LogEvent, infrastructure::database::Database};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapPosition {
    north_south: f64,
    east_west: f64,
    elevation: f64,
    heading_degrees: Option<f64>,
    happened_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapSnapshot {
    active_character: String,
    zone_name: String,
    position: Option<MapPosition>,
}

pub fn snapshot(database: &Database) -> Result<MapSnapshot, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let active_character = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='active_character'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let stored = if active_character.is_empty() {
        None
    } else {
        connection
            .query_row(
                "SELECT zone_name,north_south,east_west,elevation,heading_degrees,happened_at
                 FROM character_map_state WHERE character_name=? COLLATE NOCASE",
                [&active_character],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<f64>>(1)?,
                        row.get::<_, Option<f64>>(2)?,
                        row.get::<_, Option<f64>>(3)?,
                        row.get::<_, Option<f64>>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
    };
    let history_zone = if active_character.is_empty() {
        None
    } else {
        connection
            .query_row(
                "SELECT zone_name FROM character_zone_history
                 WHERE character_name=? COLLATE NOCASE
                 ORDER BY datetime(happened_at) DESC,id DESC LIMIT 1",
                [&active_character],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
    };
    let (stored_zone, position) = stored.map_or_else(
        || (String::new(), None),
        |(zone, north_south, east_west, elevation, heading_degrees, happened_at)| {
            let position = north_south.zip(east_west).zip(elevation).map(
                |((north_south, east_west), elevation)| MapPosition {
                    north_south,
                    east_west,
                    elevation,
                    heading_degrees,
                    happened_at,
                },
            );
            (zone, position)
        },
    );
    Ok(MapSnapshot {
        active_character,
        zone_name: history_zone.unwrap_or(stored_zone),
        position,
    })
}

pub fn record_event(
    connection: &rusqlite::Connection,
    character: &str,
    source_file: &str,
    source_offset: i64,
    event: &LogEvent,
) -> Result<(), String> {
    match event {
        LogEvent::ZoneChanged {
            happened_at,
            zone_name,
            ..
        } => connection
            .execute(
                "INSERT INTO character_map_state(character_name,zone_name,happened_at,source_file,source_offset)
                 VALUES(?,?,?,?,?)
                 ON CONFLICT(character_name) DO UPDATE SET
                    zone_name=excluded.zone_name,
                    north_south=NULL,east_west=NULL,elevation=NULL,heading_degrees=NULL,
                    happened_at=excluded.happened_at,source_file=excluded.source_file,source_offset=excluded.source_offset
                 WHERE datetime(excluded.happened_at)>=datetime(character_map_state.happened_at)",
                params![character, zone_name, happened_at.to_string(), source_file, source_offset],
            )
            .map(|_| ())
            .map_err(|error| error.to_string()),
        LogEvent::PlayerLocation {
            happened_at,
            north_south_milli,
            east_west_milli,
            elevation_milli,
        } => {
            let previous = connection
                .query_row(
                    "SELECT north_south,east_west FROM character_map_state
                     WHERE character_name=? COLLATE NOCASE",
                    [character],
                    |row| Ok((row.get::<_, Option<f64>>(0)?, row.get::<_, Option<f64>>(1)?)),
                )
                .optional()
                .map_err(|error| error.to_string())?;
            let north_south = *north_south_milli as f64 / 1000.0;
            let east_west = *east_west_milli as f64 / 1000.0;
            let elevation = *elevation_milli as f64 / 1000.0;
            let heading = previous.and_then(|(old_ns, old_ew)| {
                old_ns.zip(old_ew).and_then(|(old_ns, old_ew)| {
                    let delta_ns = north_south - old_ns;
                    let delta_ew = east_west - old_ew;
                    (delta_ns.hypot(delta_ew) >= 0.25)
                        .then(|| delta_ew.atan2(delta_ns).to_degrees())
                })
            });
            let zone_name = connection
                .query_row(
                    "SELECT zone_name FROM character_zone_history
                     WHERE character_name=? COLLATE NOCASE
                     ORDER BY datetime(happened_at) DESC,id DESC LIMIT 1",
                    [character],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?
                .unwrap_or_default();
            connection
                .execute(
                    "INSERT INTO character_map_state(
                        character_name,zone_name,north_south,east_west,elevation,heading_degrees,
                        happened_at,source_file,source_offset
                     ) VALUES(?,?,?,?,?,?,?,?,?)
                     ON CONFLICT(character_name) DO UPDATE SET
                        zone_name=CASE WHEN excluded.zone_name<>'' THEN excluded.zone_name ELSE character_map_state.zone_name END,
                        north_south=excluded.north_south,east_west=excluded.east_west,elevation=excluded.elevation,
                        heading_degrees=COALESCE(excluded.heading_degrees,character_map_state.heading_degrees),
                        happened_at=excluded.happened_at,source_file=excluded.source_file,source_offset=excluded.source_offset
                     WHERE datetime(excluded.happened_at)>=datetime(character_map_state.happened_at)",
                    params![character,zone_name,north_south,east_west,elevation,heading,happened_at.to_string(),source_file,source_offset],
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    #[test]
    fn position_is_persisted_and_heading_is_inferred() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("map.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_character','Mapper')",
                [],
            )
            .unwrap();
        let at = NaiveDateTime::parse_from_str("2026-11-05 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        record_event(
            &connection,
            "Mapper",
            "eqlog.txt",
            1,
            &LogEvent::ZoneChanged {
                happened_at: at,
                zone_name: "Kael Drakkel".into(),
                evidence: "entered".into(),
            },
        )
        .unwrap();
        record_event(
            &connection,
            "Mapper",
            "eqlog.txt",
            2,
            &LogEvent::PlayerLocation {
                happened_at: at,
                north_south_milli: 1000,
                east_west_milli: 2000,
                elevation_milli: 3000,
            },
        )
        .unwrap();
        record_event(
            &connection,
            "Mapper",
            "eqlog.txt",
            3,
            &LogEvent::PlayerLocation {
                happened_at: at,
                north_south_milli: 2000,
                east_west_milli: 2000,
                elevation_milli: 3000,
            },
        )
        .unwrap();
        let result = snapshot(&database).unwrap();
        assert_eq!(result.zone_name, "Kael Drakkel");
        assert_eq!(result.position.unwrap().heading_degrees, Some(0.0));
    }
}
