CREATE TABLE IF NOT EXISTS character_zone_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    happened_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    character_name TEXT NOT NULL COLLATE NOCASE,
    zone_name TEXT NOT NULL COLLATE NOCASE,
    evidence TEXT NOT NULL,
    source_file TEXT,
    source_offset INTEGER,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_character_zone_history_character_time
ON character_zone_history(character_name COLLATE NOCASE, happened_at DESC);

CREATE TABLE IF NOT EXISTS experience_gain_contexts (
    experience_gain_event_id INTEGER PRIMARY KEY,
    mob_name TEXT COLLATE NOCASE,
    mob_evidence TEXT,
    zone_name TEXT COLLATE NOCASE,
    zone_evidence TEXT,
    FOREIGN KEY(experience_gain_event_id) REFERENCES experience_gain_events(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS experience_gain_group_members (
    experience_gain_event_id INTEGER NOT NULL,
    member_name TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY(experience_gain_event_id, member_name),
    FOREIGN KEY(experience_gain_event_id) REFERENCES experience_gain_events(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_experience_context_mob
ON experience_gain_contexts(mob_name COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_experience_context_zone
ON experience_gain_contexts(zone_name COLLATE NOCASE);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (40, 'experience gain context');
