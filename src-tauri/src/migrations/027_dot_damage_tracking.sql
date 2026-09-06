CREATE TABLE IF NOT EXISTS dot_applications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    target_name TEXT NOT NULL COLLATE NOCASE,
    caster_name TEXT NOT NULL DEFAULT 'Unknown' COLLATE NOCASE,
    attribution_method TEXT NOT NULL DEFAULT 'unknown',
    landed_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    damage_per_tick INTEGER NOT NULL,
    tick_interval_seconds INTEGER NOT NULL DEFAULT 6,
    total_ticks INTEGER NOT NULL,
    ticks_applied INTEGER NOT NULL DEFAULT 0,
    inference_enabled INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'active',
    ended_at TEXT,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset, spell_name)
);
CREATE INDEX IF NOT EXISTS idx_dot_applications_active ON dot_applications(status, expires_at, encounter_id);
CREATE INDEX IF NOT EXISTS idx_dot_applications_encounter ON dot_applications(encounter_id, landed_at DESC);
INSERT OR IGNORE INTO schema_migrations(version, name) VALUES(27, 'damage over time inference');