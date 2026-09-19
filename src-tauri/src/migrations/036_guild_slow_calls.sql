CREATE TABLE IF NOT EXISTS guild_slow_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL,
    speaker_name TEXT NOT NULL,
    mob_name TEXT NOT NULL,
    message TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_guild_slow_calls_character_time
    ON guild_slow_calls(character_name COLLATE NOCASE, happened_at DESC, id DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(36, 'guild slow call evidence');
