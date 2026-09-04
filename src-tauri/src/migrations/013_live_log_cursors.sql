CREATE TABLE IF NOT EXISTS live_log_cursors (
    source_file TEXT PRIMARY KEY,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(13, 'persistent live log cursors');
