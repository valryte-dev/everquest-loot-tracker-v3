use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::{fs, path::Path};

use crate::infrastructure::database::Database;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStats {
    database_path: String,
    database_bytes: u64,
    wal_bytes: u64,
    page_size: i64,
    page_count: i64,
    freelist_pages: i64,
    reclaimable_bytes: u64,
    schema_version: i64,
    journal_mode: String,
    integrity: String,
    generated_at: String,
    categories: Vec<StorageCategory>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageCategory {
    key: &'static str,
    label: &'static str,
    description: &'static str,
    row_count: i64,
    estimated_payload_bytes: u64,
    oldest_at: Option<String>,
    newest_at: Option<String>,
    retention_supported: bool,
    summary_preserved: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    cutoff: String,
    outgoing_rows: i64,
    incoming_rows: i64,
    total_rows: i64,
    estimated_payload_bytes: u64,
    oldest_at: Option<String>,
    newest_at: Option<String>,
    encounter_summaries_preserved: bool,
}

struct TableMeasure {
    rows: i64,
    estimated_bytes: u64,
    oldest: Option<String>,
    newest: Option<String>,
}

pub fn stats(database: &Database) -> Result<DatabaseStats, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let path = database_path(&connection)?;
    let database_bytes = file_size(&path);
    let wal_bytes = file_size(format!("{path}-wal"));
    let page_size = pragma_i64(&connection, "page_size")?;
    let page_count = pragma_i64(&connection, "page_count")?;
    let freelist_pages = pragma_i64(&connection, "freelist_count")?;
    let schema_version = connection
        .query_row(
            "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let journal_mode = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(db_error)?;
    let integrity = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(db_error)?;

    let outgoing = measure(
        &connection,
        "damage_events",
        "happened_at",
        "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+LENGTH(COALESCE(happened_at,''))+LENGTH(COALESCE(attack_kind,''))+LENGTH(COALESCE(attacker_name,''))+48",
    )?;
    let incoming = measure(
        &connection,
        "damage_received_events",
        "happened_at",
        "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+LENGTH(COALESCE(happened_at,''))+LENGTH(COALESCE(attack_kind,''))+LENGTH(COALESCE(attacker_name,''))+LENGTH(COALESCE(target_name,''))+48",
    )?;
    let combat_summaries = combine(&[
        measure(&connection, "damage_encounters", "started_at", "160")?,
        measure(
            &connection,
            "damage_participant_summaries",
            "first_damage_at",
            "80",
        )?,
        measure(
            &connection,
            "damage_target_summaries",
            "first_damage_at",
            "80",
        )?,
    ]);
    let history = combine(&[
        measure(
            &connection,
            "activity_loot_history",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+96",
        )?,
        measure(
            &connection,
            "activity_mob_history",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+80",
        )?,
        measure(
            &connection,
            "activity_offer_history",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+96",
        )?,
        measure(
            &connection,
            "activity_level_history",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+64",
        )?,
        measure(
            &connection,
            "death_report_entries",
            "NULL",
            "LENGTH(COALESCE(raw_line,''))+24",
        )?,
    ]);
    let loot_trading = combine(&[
        measure(
            &connection,
            "loot_drops",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+96",
        )?,
        measure(
            &connection,
            "linked_loot_items",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+96",
        )?,
        measure(
            &connection,
            "merchant_messages",
            "happened_at",
            "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+96",
        )?,
        measure(&connection, "tracked_loot_items", "tracked_at", "128")?,
        measure(&connection, "completed_split_items", "completed_at", "128")?,
    ]);
    let roster_market = combine(&[
        measure(&connection, "inventory_items", "NULL", "96")?,
        measure(&connection, "spellbook_spells", "NULL", "64")?,
        measure(&connection, "master_items", "NULL", "96")?,
        measure(&connection, "item_market_values", "fetched_at", "160")?,
    ]);
    let diagnostics = combine(&[
        measure(
            &connection,
            "application_logs",
            "happened_at",
            "LENGTH(COALESCE(message,''))+64",
        )?,
        measure(
            &connection,
            "import_uploads",
            "happened_at",
            "LENGTH(COALESCE(detail,''))+96",
        )?,
        measure(
            &connection,
            "planner_upload_jobs",
            "created_at",
            "LENGTH(COALESCE(payload_text,''))+128",
        )?,
    ]);
    let shadows = combine(&[
        measure(
            &connection,
            "compound_workspace_snapshots",
            "captured_at",
            "LENGTH(COALESCE(source_json,''))+32",
        )?,
        measure(
            &connection,
            "split_lifecycle_snapshots",
            "captured_at",
            "LENGTH(COALESCE(source_json,''))+32",
        )?,
    ]);

    let combat_detail = combine(&[outgoing, incoming]);
    let categories = vec![
        category("combat-detail", "Combat hit detail", "Every outgoing and incoming hit. Encounter summaries and analytics can survive optional detail retention.", combat_detail, true, true),
        category("combat-summaries", "Combat summaries", "Encounter, participant, and target aggregates used by damage history and charts.", combat_summaries, false, false),
        category("history", "History and death context", "Loot, slain mobs, offers, levels, and the 30-line death-report context.", history, false, false),
        category("loot-trading", "Loot, splits, and trading", "Live/tracked/linked loot, merchant messages, and completed split records.", loot_trading, false, false),
        category("roster-market", "Roster and market catalog", "Current inventory, spellbooks, master items, and PigParse values.", roster_market, false, false),
        category("diagnostics", "Application diagnostics", "Rolling application messages, import records, and durable Planner jobs.", diagnostics, true, false),
        category("shadow-models", "Rollback shadow models", "Additive compound and split snapshots retained for migration parity and rollback.", shadows, false, false),
    ];

    Ok(DatabaseStats {
        database_path: path,
        database_bytes,
        wal_bytes,
        page_size,
        page_count,
        freelist_pages,
        reclaimable_bytes: (freelist_pages.max(0) as u64) * (page_size.max(0) as u64),
        schema_version,
        journal_mode,
        integrity,
        generated_at: Utc::now().to_rfc3339(),
        categories,
    })
}

pub fn preview_combat_retention(
    database: &Database,
    keep_days: u32,
) -> Result<CleanupPreview, String> {
    if !(7..=3650).contains(&keep_days) {
        return Err("Retention must be between 7 and 3650 days".into());
    }
    let connection = database.connect().map_err(|error| error.to_string())?;
    let cutoff_modifier = format!("-{keep_days} days");
    let outgoing = preview_table(&connection, "damage_events", &cutoff_modifier)?;
    let incoming = preview_table(&connection, "damage_received_events", &cutoff_modifier)?;
    Ok(CleanupPreview {
        cutoff: connection
            .query_row("SELECT datetime('now',?)", [&cutoff_modifier], |row| {
                row.get(0)
            })
            .map_err(db_error)?,
        outgoing_rows: outgoing.rows,
        incoming_rows: incoming.rows,
        total_rows: outgoing.rows + incoming.rows,
        estimated_payload_bytes: outgoing.estimated_bytes + incoming.estimated_bytes,
        oldest_at: min_option(outgoing.oldest, incoming.oldest),
        newest_at: max_option(outgoing.newest, incoming.newest),
        encounter_summaries_preserved: true,
    })
}

fn preview_table(
    connection: &Connection,
    table: &str,
    cutoff_modifier: &str,
) -> Result<TableMeasure, String> {
    let statement = format!(
        "SELECT COUNT(*),MIN(happened_at),MAX(happened_at) FROM {table} WHERE happened_at < datetime('now',?)"
    );
    let (rows, oldest, newest) = connection
        .query_row(&statement, [cutoff_modifier], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(db_error)?;
    let expression = if table == "damage_events" {
        "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+LENGTH(COALESCE(happened_at,''))+LENGTH(COALESCE(attack_kind,''))+LENGTH(COALESCE(attacker_name,''))+48"
    } else {
        "LENGTH(COALESCE(raw_line,''))+LENGTH(COALESCE(source_file,''))+LENGTH(COALESCE(happened_at,''))+LENGTH(COALESCE(attack_kind,''))+LENGTH(COALESCE(attacker_name,''))+LENGTH(COALESCE(target_name,''))+48"
    };
    let average = sample_average(connection, table, expression)?;
    Ok(TableMeasure {
        rows,
        estimated_bytes: estimate(rows, average),
        oldest,
        newest,
    })
}

fn measure(
    connection: &Connection,
    table: &str,
    date_column: &str,
    size_expression: &str,
) -> Result<TableMeasure, String> {
    if !table_exists(connection, table)? {
        return Ok(TableMeasure {
            rows: 0,
            estimated_bytes: 0,
            oldest: None,
            newest: None,
        });
    }
    let date_fields = if date_column == "NULL" {
        "NULL,NULL".to_owned()
    } else {
        format!("MIN({date_column}),MAX({date_column})")
    };
    let statement = format!("SELECT COUNT(*),{date_fields} FROM {table}");
    let (rows, oldest, newest) = connection
        .query_row(&statement, [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(db_error)?;
    let average = sample_average(connection, table, size_expression)?;
    Ok(TableMeasure {
        rows,
        estimated_bytes: estimate(rows, average),
        oldest,
        newest,
    })
}

fn sample_average(connection: &Connection, table: &str, expression: &str) -> Result<f64, String> {
    let statement = format!(
        "SELECT COALESCE(AVG(size),0) FROM (SELECT {expression} AS size FROM {table} ORDER BY rowid DESC LIMIT 10000)"
    );
    connection
        .query_row(&statement, [], |row| row.get(0))
        .map_err(db_error)
}

fn combine(values: &[TableMeasure]) -> TableMeasure {
    TableMeasure {
        rows: values.iter().map(|value| value.rows).sum(),
        estimated_bytes: values.iter().map(|value| value.estimated_bytes).sum(),
        oldest: values.iter().filter_map(|value| value.oldest.clone()).min(),
        newest: values.iter().filter_map(|value| value.newest.clone()).max(),
    }
}

fn category(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    value: TableMeasure,
    retention_supported: bool,
    summary_preserved: bool,
) -> StorageCategory {
    StorageCategory {
        key,
        label,
        description,
        row_count: value.rows,
        estimated_payload_bytes: value.estimated_bytes,
        oldest_at: value.oldest,
        newest_at: value.newest,
        retention_supported,
        summary_preserved,
    }
}

fn estimate(rows: i64, average: f64) -> u64 {
    ((rows.max(0) as f64) * average.max(0.0)).round() as u64
}
fn file_size(path: impl AsRef<Path>) -> u64 {
    fs::metadata(path).map(|value| value.len()).unwrap_or(0)
}
fn min_option(left: Option<String>, right: Option<String>) -> Option<String> {
    [left, right].into_iter().flatten().min()
}
fn max_option(left: Option<String>, right: Option<String>) -> Option<String> {
    [left, right].into_iter().flatten().max()
}
fn pragma_i64(connection: &Connection, name: &str) -> Result<i64, String> {
    connection
        .query_row(&format!("PRAGMA {name}"), [], |row| row.get(0))
        .map_err(db_error)
}
fn table_exists(connection: &Connection, table: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?",
            [table],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(db_error)
}
fn database_path(connection: &Connection) -> Result<String, String> {
    connection
        .query_row("PRAGMA database_list", [], |row| row.get(2))
        .map_err(db_error)
}
fn db_error(error: rusqlite::Error) -> String {
    error.to_string()
}
#[cfg(test)]
mod tests {
    use super::{preview_combat_retention, stats};
    use crate::infrastructure::database::Database;

    #[test]
    fn statistics_and_retention_preview_are_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,total_damage,melee_damage,spell_damage,hit_count,max_hit,outcome,source_file,first_source_offset,last_source_offset)
             VALUES('Tester','old mob',datetime('now','-100 days'),datetime('now','-100 days'),10,10,0,1,10,'slain','old-log',1,1),
                   ('Tester','recent mob',datetime('now','-10 days'),datetime('now','-10 days'),20,20,0,1,20,'slain','recent-log',1,1)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name)
             VALUES(1,datetime('now','-100 days'),'melee','crush',10,'old outgoing','old-log',2,'Tester'),
                   (2,datetime('now','-10 days'),'melee','crush',20,'recent outgoing','recent-log',2,'Tester')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO damage_received_events(encounter_id,happened_at,attacker_name,target_name,attack_kind,damage,raw_line,source_file,source_offset)
             VALUES(1,datetime('now','-100 days'),'old mob','Tester','hits',5,'old incoming','old-log',3),
                   (2,datetime('now','-10 days'),'recent mob','Tester','hits',6,'recent incoming','recent-log',3)",
            [],
        ).unwrap();
        drop(connection);

        let report = stats(&database).unwrap();
        let combat = report
            .categories
            .iter()
            .find(|category| category.key == "combat-detail")
            .unwrap();
        assert_eq!(combat.row_count, 4);
        assert!(combat.summary_preserved);
        assert_eq!(report.integrity, "ok");

        let preview = preview_combat_retention(&database, 90).unwrap();
        assert_eq!((preview.outgoing_rows, preview.incoming_rows), (1, 1));
        assert_eq!(preview.total_rows, 2);
        assert!(preview.encounter_summaries_preserved);

        let connection = database.connect().unwrap();
        let remaining: i64 = connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM damage_events)+(SELECT COUNT(*) FROM damage_received_events)",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 4);
    }
}
