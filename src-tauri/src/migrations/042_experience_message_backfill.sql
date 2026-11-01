CREATE TABLE IF NOT EXISTS experience_backfill_cursors (
    source_file TEXT PRIMARY KEY,
    character_name TEXT NOT NULL COLLATE NOCASE,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (42, 'experience message backfill cursors');
