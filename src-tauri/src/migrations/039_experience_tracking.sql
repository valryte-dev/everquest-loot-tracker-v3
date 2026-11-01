CREATE TABLE IF NOT EXISTS experience_gain_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_experience_gain_character_time
ON experience_gain_events(character_name COLLATE NOCASE, happened_at DESC);

CREATE TABLE IF NOT EXISTS experience_observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    character_name TEXT NOT NULL,
    level INTEGER,
    percent REAL NOT NULL CHECK(percent >= 0 AND percent <= 100),
    confidence REAL NOT NULL CHECK(confidence >= 0 AND confidence <= 1),
    gain_events_since_previous INTEGER NOT NULL DEFAULT 0,
    source TEXT NOT NULL DEFAULT 'visual'
);

CREATE INDEX IF NOT EXISTS idx_experience_observation_character_time
ON experience_observations(character_name COLLATE NOCASE, happened_at DESC);

CREATE TABLE IF NOT EXISTS experience_calibrations (
    character_name TEXT PRIMARY KEY COLLATE NOCASE,
    region_json TEXT NOT NULL,
    hue INTEGER NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (39, 'visual experience tracking');
