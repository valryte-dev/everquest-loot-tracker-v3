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
        assert_eq!(database.migrate().unwrap(), 6);
        assert_eq!(database.migrate().unwrap(), 6);
    }
}
