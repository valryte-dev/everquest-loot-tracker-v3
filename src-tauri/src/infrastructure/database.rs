use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
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

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub struct Database {
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_owned();
        let connection = Self::connection_at(&path)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(10))?;
        drop(connection);
        Ok(Self { path })
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
    pub fn refresh_item_values(connection: &Connection) -> Result<(), DatabaseError> {
        connection.execute_batch(UNIFIED_ITEM_VALUES_MIGRATION)?;
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
    fn migration_is_additive_and_repeatable() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot-tracker.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 17);
        assert_eq!(database.migrate().unwrap(), 17);
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
        assert_eq!(database.migrate().unwrap(), 17);
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
        assert_eq!(database.migrate().unwrap(), 17);
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
