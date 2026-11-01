CREATE TABLE IF NOT EXISTS spawn_timer_trigger_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL,
    mob_name TEXT NOT NULL,
    trigger_kind TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset, trigger_kind)
);

CREATE INDEX IF NOT EXISTS idx_spawn_timer_trigger_history_mob_time
ON spawn_timer_trigger_history(mob_name COLLATE NOCASE, happened_at DESC);

CREATE INDEX IF NOT EXISTS idx_spawn_timer_trigger_history_character_time
ON spawn_timer_trigger_history(character_name COLLATE NOCASE, happened_at DESC);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(49,'spawn timer trigger history');
