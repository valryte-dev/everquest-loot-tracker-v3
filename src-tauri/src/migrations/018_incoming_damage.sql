CREATE TABLE IF NOT EXISTS damage_received_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    happened_at TEXT NOT NULL,
    attacker_name TEXT NOT NULL,
    target_name TEXT NOT NULL,
    attack_kind TEXT NOT NULL,
    damage INTEGER NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_damage_received_encounter_time
    ON damage_received_events(encounter_id, happened_at, id);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(18, 'incoming mob damage');
