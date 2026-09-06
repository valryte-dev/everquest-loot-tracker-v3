CREATE TABLE IF NOT EXISTS export_import_state (
    source_file TEXT PRIMARY KEY,
    file_size INTEGER NOT NULL,
    modified_unix_ns INTEGER NOT NULL,
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(21, 'persistent export import state');
