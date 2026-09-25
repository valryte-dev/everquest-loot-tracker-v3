use rusqlite::{Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use thiserror::Error;

const COMPATIBILITY_SCHEMA: &str = include_str!("../migrations/000_v2_compatibility.sql");
const V3_MIGRATION: &str = include_str!("../migrations/001_v3_foundation.sql");
const FEATURE_PARITY_MIGRATION: &str = include_str!("../migrations/002_feature_parity.sql");
const MASTER_ITEMS_MIGRATION: &str = include_str!("../migrations/003_master_items.sql");
const MERCHANT_MODE_MIGRATION: &str = include_str!("../migrations/004_merchant_mode.sql");
const TRACKED_LOOT_MIGRATION: &str = include_str!("../migrations/005_tracked_loot.sql");
const LINKED_LOOT_MIGRATION: &str = include_str!("../migrations/006_linked_loot.sql");
const LINKED_LOOT_CATALOG_MIGRATION: &str =
    include_str!("../migrations/007_linked_loot_catalog.sql");
const SPLIT_PAYOUT_PHASES_MIGRATION: &str =
    include_str!("../migrations/008_split_payout_phases.sql");
const INDIVIDUAL_SPLIT_PAYOUTS_MIGRATION: &str =
    include_str!("../migrations/009_individual_split_payouts.sql");
const UNIFIED_ITEM_VALUES_MIGRATION: &str =
    include_str!("../migrations/010_unified_item_values.sql");
const ACTIVITY_HISTORY_MIGRATION: &str = include_str!("../migrations/011_activity_history.sql");
const LEVEL_HISTORY_MIGRATION: &str = include_str!("../migrations/012_level_history.sql");
const LIVE_LOG_CURSORS_MIGRATION: &str = include_str!("../migrations/013_live_log_cursors.sql");
const DEATH_REPORTS_MIGRATION: &str = include_str!("../migrations/014_death_reports.sql");
const DAMAGE_TRACKER_MIGRATION: &str = include_str!("../migrations/015_damage_tracker.sql");
const WEAPON_LOADOUTS_MIGRATION: &str = include_str!("../migrations/016_weapon_loadouts.sql");
const DAMAGE_ATTACKERS_MIGRATION: &str = include_str!("../migrations/017_damage_attackers.sql");
const INCOMING_DAMAGE_MIGRATION: &str = include_str!("../migrations/018_incoming_damage.sql");
const CLERIC_HEAL_CALLS_MIGRATION: &str = include_str!("../migrations/019_cleric_heal_calls.sql");
const GUILD_ONLY_CLERIC_HEAL_CALLS_MIGRATION: &str =
    include_str!("../migrations/020_guild_only_cleric_heal_calls.sql");
const EXPORT_IMPORT_STATE_MIGRATION: &str =
    include_str!("../migrations/021_export_import_state.sql");
const PLANNER_UPLOAD_JOBS_MIGRATION: &str =
    include_str!("../migrations/022_planner_upload_jobs.sql");
const DAMAGE_SUMMARIES_MIGRATION: &str = include_str!("../migrations/023_damage_summaries.sql");
const CANONICAL_ITEM_IDS_MIGRATION: &str = include_str!("../migrations/024_canonical_item_ids.sql");
const NORMALIZED_COMPOUNDS_MIGRATION: &str =
    include_str!("../migrations/025_normalized_compounds.sql");
const SPLIT_LIFECYCLE_MIGRATION: &str = include_str!("../migrations/026_split_lifecycle.sql");
const DOT_DAMAGE_TRACKING_MIGRATION: &str =
    include_str!("../migrations/027_dot_damage_tracking.sql");
const REPAIR_DOT_EVENT_TIMESTAMPS_MIGRATION: &str =
    include_str!("../migrations/028_repair_dot_event_timestamps.sql");
const PROC_DAMAGE_METRICS_MIGRATION: &str =
    include_str!("../migrations/029_proc_damage_metrics.sql");
const COMBAT_SPELL_ACTIVITY_MIGRATION: &str =
    include_str!("../migrations/030_combat_spell_activity.sql");
const COMBAT_PET_EVIDENCE_MIGRATION: &str =
    include_str!("../migrations/031_combat_pet_evidence.sql");
const ITEM_PROC_CATALOG_MIGRATION: &str = include_str!("../migrations/032_item_proc_catalog.sql");
const QUEST_ITEM_CATALOG_MIGRATION: &str = include_str!("../migrations/033_quest_item_catalog.sql");
const PROTECTED_COMBAT_FIGHTS_MIGRATION: &str =
    include_str!("../migrations/034_protected_combat_fights.sql");
const DAMAGE_ATTACK_TYPE_SUMMARIES_MIGRATION: &str =
    include_str!("../migrations/035_damage_attack_type_summaries.sql");
const GUILD_SLOW_CALLS_MIGRATION: &str = include_str!("../migrations/036_guild_slow_calls.sql");
const CHARACTER_PROFILES_MIGRATION: &str = include_str!("../migrations/037_character_profiles.sql");
const SPELL_RESEARCH_CATALOG_MIGRATION: &str =
    include_str!("../migrations/038_spell_research_catalog.sql");

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
    writer_gate: Arc<Mutex<()>>,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_owned();
        let connection = Self::connection_at(&path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;
        drop(connection);
        Ok(Self {
            path,
            writer_gate: Arc::new(Mutex::new(())),
        })
    }

    fn connection_at(path: &Path) -> Result<Connection, rusqlite::Error> {
        Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )
    }

    pub fn connect(&self) -> Result<Connection, DatabaseError> {
        let connection = Self::connection_at(&self.path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;
        Ok(connection)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn writer_guard(&self) -> MutexGuard<'_, ()> {
        self.writer_gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn refresh_item_values(connection: &Connection) -> Result<(), DatabaseError> {
        connection.execute_batch(UNIFIED_ITEM_VALUES_MIGRATION)?;
        for table in [
            "loot_drops",
            "tracked_loot_items",
            "manual_split_list_items",
            "split_loot_items",
            "completed_split_items",
            "activity_loot_history",
            "activity_offer_history",
            "linked_loot_items",
            "item_proc_spells",
            "quest_catalog_components",
            "spell_research_components",
        ] {
            connection.execute(
                &format!("UPDATE {table} SET item_id=(SELECT item_id FROM item_name_resolutions WHERE item_name={table}.item_name COLLATE NOCASE) WHERE item_id IS NULL"),
                [],
            )?;
        }
        connection.execute(
            "UPDATE quest_catalog_entries SET reward_item_id=(
                SELECT item_id FROM item_name_resolutions
                WHERE item_name=quest_catalog_entries.reward_name COLLATE NOCASE
             ) WHERE reward_item_id IS NULL",
            [],
        )?;
        connection.execute(
            "UPDATE spell_research_recipes SET spell_item_id=(
                SELECT item_id FROM item_name_resolutions
                WHERE item_name=('Spell: '||spell_research_recipes.spell_name) COLLATE NOCASE
             ) WHERE spell_item_id IS NULL",
            [],
        )?;
        Ok(())
    }
    pub fn migrate(&self) -> Result<i64, DatabaseError> {
        let mut connection = Self::connection_at(&self.path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let transaction = connection.transaction()?;
        transaction.execute_batch(COMPATIBILITY_SCHEMA)?;
        transaction.execute_batch(V3_MIGRATION)?;
        transaction.execute_batch(FEATURE_PARITY_MIGRATION)?;
        transaction.execute_batch(MASTER_ITEMS_MIGRATION)?;
        transaction.execute_batch(MERCHANT_MODE_MIGRATION)?;
        transaction.execute_batch(TRACKED_LOOT_MIGRATION)?;
        transaction.execute_batch(LINKED_LOOT_MIGRATION)?;
        transaction.execute_batch(LINKED_LOOT_CATALOG_MIGRATION)?;
        let schema_version: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        if schema_version < 8 {
            transaction.execute_batch(SPLIT_PAYOUT_PHASES_MIGRATION)?;
        }
        if schema_version < 9 {
            transaction.execute_batch(INDIVIDUAL_SPLIT_PAYOUTS_MIGRATION)?;
        }
        transaction.execute_batch(UNIFIED_ITEM_VALUES_MIGRATION)?;
        transaction.execute_batch(ACTIVITY_HISTORY_MIGRATION)?;
        if schema_version < 12 {
            transaction.execute_batch(LEVEL_HISTORY_MIGRATION)?;
        }
        if schema_version < 13 {
            transaction.execute_batch(LIVE_LOG_CURSORS_MIGRATION)?;
        }
        if schema_version < 14 {
            transaction.execute_batch(DEATH_REPORTS_MIGRATION)?;
        }
        if schema_version < 15 {
            transaction.execute_batch(DAMAGE_TRACKER_MIGRATION)?;
        }
        if schema_version < 16 {
            transaction.execute_batch(WEAPON_LOADOUTS_MIGRATION)?;
        }
        if schema_version < 17 {
            transaction.execute_batch(DAMAGE_ATTACKERS_MIGRATION)?;
        }
        if schema_version < 18 {
            transaction.execute_batch(INCOMING_DAMAGE_MIGRATION)?;
        }
        if schema_version < 19 {
            transaction.execute_batch(CLERIC_HEAL_CALLS_MIGRATION)?;
        }
        if schema_version < 20 {
            transaction.execute_batch(GUILD_ONLY_CLERIC_HEAL_CALLS_MIGRATION)?;
        }
        if schema_version < 21 {
            transaction.execute_batch(EXPORT_IMPORT_STATE_MIGRATION)?;
        }
        if schema_version < 22 {
            transaction.execute_batch(PLANNER_UPLOAD_JOBS_MIGRATION)?;
        }
        if schema_version < 23 {
            transaction.execute_batch(DAMAGE_SUMMARIES_MIGRATION)?;
        }
        if schema_version < 24 {
            for table in [
                "loot_drops",
                "tracked_loot_items",
                "manual_split_list_items",
                "split_loot_items",
                "completed_split_items",
                "activity_loot_history",
                "activity_offer_history",
                "linked_loot_items",
            ] {
                let exists: i64 = transaction.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info(?) WHERE name='item_id'",
                    [table],
                    |row| row.get(0),
                )?;
                if exists == 0 {
                    transaction.execute(
                        &format!("ALTER TABLE {table} ADD COLUMN item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL"),
                        [],
                    )?;
                }
            }
            transaction.execute_batch(CANONICAL_ITEM_IDS_MIGRATION)?;
        }
        if schema_version < 25 {
            transaction.execute_batch(NORMALIZED_COMPOUNDS_MIGRATION)?;
        }
        if schema_version < 26 {
            transaction.execute_batch(SPLIT_LIFECYCLE_MIGRATION)?;
        }
        if schema_version < 27 {
            transaction.execute_batch(DOT_DAMAGE_TRACKING_MIGRATION)?;
        }
        if schema_version < 28 {
            transaction.execute_batch(REPAIR_DOT_EVENT_TIMESTAMPS_MIGRATION)?;
        }
        if schema_version < 29 {
            transaction.execute_batch(PROC_DAMAGE_METRICS_MIGRATION)?;
        }
        if schema_version < 30 {
            transaction.execute_batch(COMBAT_SPELL_ACTIVITY_MIGRATION)?;
        }
        if schema_version < 31 {
            transaction.execute_batch(COMBAT_PET_EVIDENCE_MIGRATION)?;
        }
        if schema_version < 32 {
            transaction.execute_batch(ITEM_PROC_CATALOG_MIGRATION)?;
        }
        if schema_version < 33 {
            transaction.execute_batch(QUEST_ITEM_CATALOG_MIGRATION)?;
        }
        if schema_version < 34 {
            let protected_exists: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('damage_encounters') WHERE name='is_protected'",
                [], |row| row.get(0),
            )?;
            if protected_exists == 0 {
                transaction.execute_batch("ALTER TABLE damage_encounters ADD COLUMN is_protected INTEGER NOT NULL DEFAULT 0 CHECK(is_protected IN (0,1));")?;
            }
            let protected_at_exists: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('damage_encounters') WHERE name='protected_at'",
                [], |row| row.get(0),
            )?;
            if protected_at_exists == 0 {
                transaction
                    .execute_batch("ALTER TABLE damage_encounters ADD COLUMN protected_at TEXT;")?;
            }
            transaction.execute_batch(PROTECTED_COMBAT_FIGHTS_MIGRATION)?;
        }
        if schema_version < 35 {
            transaction.execute_batch(DAMAGE_ATTACK_TYPE_SUMMARIES_MIGRATION)?;
        }
        if schema_version < 36 {
            transaction.execute_batch(GUILD_SLOW_CALLS_MIGRATION)?;
        }
        if schema_version < 37 {
            transaction.execute_batch(CHARACTER_PROFILES_MIGRATION)?;
        }
        if schema_version < 38 {
            transaction.execute_batch(SPELL_RESEARCH_CATALOG_MIGRATION)?;
        }
        super::proc_catalog::reconcile(&transaction)?;
        super::quest_catalog::reconcile(&transaction)?;
        super::spell_research_catalog::reconcile(&transaction)?;
        transaction.commit()?;
        Ok(connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_clones_serialize_coordinated_writers() {
        use std::{sync::mpsc, thread, time::Duration};

        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        let first_writer = database.writer_guard();
        let second_database = database.clone();
        let (acquired_tx, acquired_rx) = mpsc::sync_channel(1);
        let waiting_writer = thread::spawn(move || {
            let _second_writer = second_database.writer_guard();
            acquired_tx.send(()).unwrap();
        });

        assert!(acquired_rx.recv_timeout(Duration::from_millis(50)).is_err());
        drop(first_writer);
        acquired_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        waiting_writer.join().unwrap();
    }

    #[test]
    fn migration_is_additive_and_repeatable() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        for table in [
            "proc_occurrences",
            "damage_spell_summaries",
            "combat_spell_activity",
            "combat_pet_evidence",
            "quest_catalog_entries",
            "quest_catalog_components",
            "spell_research_components",
            "damage_attack_type_summaries",
        ] {
            let found: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "missing {table}");
        }
    }

    #[test]
    fn migrates_an_existing_schema_28_database_to_proc_metrics() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        {
            let connection = database.connect().unwrap();
            connection.execute(
                "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset,total_damage,spell_damage,hit_count) VALUES('Tester','Target','2026-09-06 10:00:00','2026-09-06 10:00:06','test.log',10,20,125,125,1)",
                [],
            ).unwrap();
            let encounter_id = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO dot_applications(encounter_id,spell_name,target_name,caster_name,attribution_method,landed_at,expires_at,damage_per_tick,tick_interval_seconds,total_ticks,ticks_applied,source_file,source_offset) VALUES(?,'Dawncall','Target','Tester','proc','2026-09-06 10:00:00','2026-09-06 10:00:36',125,6,6,1,'test.log',10)",
                [encounter_id],
            ).unwrap();
            let application_id = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'2026-09-06 10:00:06','spell','Dawncall',125,'old inferred tick',?,1,'Tester')",
                rusqlite::params![encounter_id,format!("dot://{application_id}")],
            ).unwrap();
            connection
                .execute_batch(
                    "DROP TABLE combat_spell_activity;
                     DROP TABLE damage_spell_summaries;
                     DROP TABLE proc_occurrences;
                     DELETE FROM schema_migrations WHERE version>=29;",
                )
                .unwrap();
            let previous: i64 = connection
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(previous, 28);
        }
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let tables: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('proc_occurrences','damage_spell_summaries','combat_spell_activity')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3);
        let summary: (i64, i64, i64) = connection
            .query_row(
                "SELECT proc_count,dot_damage,proc_dot_damage FROM damage_spell_summaries",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(summary, (1, 125, 125));
        let proc_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(proc_count, 1);
    }
    #[test]
    fn repairs_misordered_inferred_dot_event_timestamps() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        {
            let connection = database.connect().unwrap();
            connection.execute(
                "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset) VALUES('Utrido','Ran Walker','2026-09-06 10:53:04','2026-09-06 10:53:10','test.log',10,20)",
                [],
            ).unwrap();
            let encounter_id = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO dot_applications(encounter_id,spell_name,target_name,caster_name,attribution_method,landed_at,expires_at,damage_per_tick,tick_interval_seconds,total_ticks,ticks_applied,source_file,source_offset) VALUES(?,'Dawncall','Ran Walker','Utrido','next_attack','2026-09-06 10:53:04','2026-09-06 10:53:40',125,6,6,1,'test.log',10)",
                [encounter_id],
            ).unwrap();
            let application_id = connection.last_insert_rowid();
            connection.execute(
                "INSERT INTO damage_events(encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name) VALUES(?,'spell','2026-09-06 10:53:10','Dawncall',125,'bad',?,1,'Utrido')",
                rusqlite::params![encounter_id,format!("dot://{application_id}")],
            ).unwrap();
            connection
                .execute("DELETE FROM schema_migrations WHERE version>=28", [])
                .unwrap();
        }
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let repaired: (String, String) = connection.query_row(
            "SELECT happened_at,damage_type FROM damage_events WHERE source_file LIKE 'dot://%'",
            [], |row| Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        let summary_time: String = connection.query_row(
            "SELECT last_damage_at FROM damage_participant_summaries WHERE attacker_name='Utrido'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(repaired, ("2026-09-06 10:53:10".into(), "spell".into()));
        assert_eq!(summary_time, "2026-09-06 10:53:10");
    }

    #[test]
    fn guild_only_cleric_calls_migration_removes_non_guild_rows() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        {
            let connection = database.connect().unwrap();
            connection
                .execute_batch(
                    "DELETE FROM schema_migrations WHERE version>=20;
                     INSERT INTO cleric_heal_calls(
                         happened_at,character_name,cleric_name,call_number,target_name,
                         channel,message,raw_line,source_file,source_offset
                     ) VALUES
                         ('2026-09-05 16:40:29','Youngman','Bakamore',1,'Forsure',
                          'group','LoF 001 CH - Forsure','group raw','test.log',1),
                         ('2026-09-05 16:40:39','Youngman','Clerica',2,'Forsure',
                          'guild','LoF 002 CH - Forsure','guild raw','test.log',2);",
                )
                .unwrap();
        }

        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let channels: Vec<String> = connection
            .prepare("SELECT channel FROM cleric_heal_calls ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(channels, vec!["guild"]);
    }

    #[test]
    fn payout_phase_migration_preserves_legacy_sales_as_pending() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        {
            let connection = database.connect().unwrap();
            connection.execute_batch(COMPATIBILITY_SCHEMA).unwrap();
            connection.execute_batch(V3_MIGRATION).unwrap();
            connection.execute_batch(FEATURE_PARITY_MIGRATION).unwrap();
            connection.execute_batch(MASTER_ITEMS_MIGRATION).unwrap();
            connection.execute_batch(MERCHANT_MODE_MIGRATION).unwrap();
            connection.execute_batch(TRACKED_LOOT_MIGRATION).unwrap();
            connection.execute_batch(LINKED_LOOT_MIGRATION).unwrap();
            connection
                .execute_batch(LINKED_LOOT_CATALOG_MIGRATION)
                .unwrap();
            connection.execute("INSERT INTO completed_split_items(item_name,value_pp,disposition) VALUES('Legacy sale',100,'sold')", []).unwrap();
            connection.execute("INSERT INTO completed_split_items(item_name,value_pp,disposition) VALUES('Legacy consumed',50,'consumed')", []).unwrap();
        }
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let sold: (String, Option<String>) = connection.query_row("SELECT payout_status,paid_at FROM completed_split_items WHERE item_name='Legacy sale'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        let consumed: (String, Option<String>) = connection.query_row("SELECT payout_status,paid_at FROM completed_split_items WHERE item_name='Legacy consumed'", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(sold, ("pending".to_owned(), None));
        assert_eq!(consumed.0, "completed");
        assert!(consumed.1.is_some());
    }

    #[test]
    fn individual_payout_migration_seeds_previously_completed_sales() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        {
            let connection = database.connect().unwrap();
            connection.execute_batch(COMPATIBILITY_SCHEMA).unwrap();
            connection.execute_batch(V3_MIGRATION).unwrap();
            connection.execute_batch(FEATURE_PARITY_MIGRATION).unwrap();
            connection.execute_batch(MASTER_ITEMS_MIGRATION).unwrap();
            connection.execute_batch(MERCHANT_MODE_MIGRATION).unwrap();
            connection.execute_batch(TRACKED_LOOT_MIGRATION).unwrap();
            connection.execute_batch(LINKED_LOOT_MIGRATION).unwrap();
            connection
                .execute_batch(LINKED_LOOT_CATALOG_MIGRATION)
                .unwrap();
            connection
                .execute_batch(SPLIT_PAYOUT_PHASES_MIGRATION)
                .unwrap();
            connection.execute("INSERT INTO completed_split_items(item_name,value_pp,disposition,payout_status,paid_at) VALUES('Already paid',200,'sold','completed','2026-08-30')", []).unwrap();
            let item_id = connection.last_insert_rowid();
            connection.execute("INSERT INTO completed_split_members(completed_split_item_id,member_name) VALUES(?,'One'),(?,'Two')", [item_id,item_id]).unwrap();
        }
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let seeded: i64 = connection
            .query_row("SELECT COUNT(*) FROM completed_split_payouts", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(seeded, 2);
    }

    #[test]
    fn unified_item_resolver_uses_master_ids_and_market_fallbacks() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO master_items(item_id,item_name,source) VALUES(20819,'Elders Earring','inventory')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_60d_pp,count_60d,fetched_at)
             VALUES('Green',20819,0,'Elder''s Earring','Today',7400,11,CURRENT_TIMESTAMP)",
            [],
        ).unwrap();
        Database::refresh_item_values(&connection).unwrap();

        let resolved: (i64, String, i64) = connection.query_row(
            "SELECT value_pp,value_basis,sample_count FROM resolved_item_values WHERE item_id=20819",
            [],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        assert_eq!(resolved, (7400, "60-day WTS".to_owned(), 11));

        let alias_id: i64 = connection.query_row(
            "SELECT item_id FROM item_name_resolutions WHERE item_name='Elder''s Earring' COLLATE NOCASE",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(alias_id, 20819);
    }
}
