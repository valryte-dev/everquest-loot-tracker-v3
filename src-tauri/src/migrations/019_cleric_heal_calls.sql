CREATE TABLE IF NOT EXISTS cleric_heal_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL,
    cleric_name TEXT NOT NULL,
    call_number INTEGER NOT NULL,
    target_name TEXT,
    channel TEXT NOT NULL,
    message TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_cleric_heal_calls_time
    ON cleric_heal_calls(happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_cleric_heal_calls_cleric
    ON cleric_heal_calls(cleric_name COLLATE NOCASE, happened_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(19, 'cleric heal calls');
