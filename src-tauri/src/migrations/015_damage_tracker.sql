CREATE TABLE IF NOT EXISTS damage_encounters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_name TEXT NOT NULL,
    mob_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    last_damage_at TEXT NOT NULL,
    total_damage INTEGER NOT NULL DEFAULT 0,
    melee_damage INTEGER NOT NULL DEFAULT 0,
    spell_damage INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    max_hit INTEGER NOT NULL DEFAULT 0,
    outcome TEXT NOT NULL DEFAULT 'active',
    source_file TEXT NOT NULL,
    first_source_offset INTEGER NOT NULL,
    last_source_offset INTEGER NOT NULL,
    UNIQUE(source_file, first_source_offset)
);

CREATE TABLE IF NOT EXISTS damage_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    happened_at TEXT NOT NULL,
    damage_type TEXT NOT NULL,
    attack_kind TEXT NOT NULL,
    damage INTEGER NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE TABLE IF NOT EXISTS damage_scan_cursors (
    source_file TEXT PRIMARY KEY,
    character_name TEXT NOT NULL,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_damage_encounters_started
    ON damage_encounters(started_at DESC);
CREATE INDEX IF NOT EXISTS idx_damage_encounters_character
    ON damage_encounters(character_name COLLATE NOCASE, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_damage_encounters_mob
    ON damage_encounters(mob_name COLLATE NOCASE, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_damage_events_encounter_time
    ON damage_events(encounter_id, happened_at, id);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(15, 'damage tracker');
