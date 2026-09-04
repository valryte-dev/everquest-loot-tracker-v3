CREATE TABLE IF NOT EXISTS activity_level_history (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL COLLATE NOCASE,
    level INTEGER NOT NULL CHECK(level BETWEEN 1 AND 255),
    direction TEXT NOT NULL CHECK(direction IN ('gained', 'lost')),
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(character_name, happened_at, raw_line)
);

CREATE INDEX IF NOT EXISTS idx_activity_level_time
ON activity_level_history(happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_level_character
ON activity_level_history(character_name COLLATE NOCASE, happened_at);

UPDATE log_history_cursors SET byte_offset=0, file_size=0;

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (12, 'character level history');
