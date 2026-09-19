CREATE INDEX IF NOT EXISTS idx_damage_encounters_protected_started
ON damage_encounters(is_protected, started_at);

CREATE TABLE IF NOT EXISTS purged_damage_encounter_ranges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_file TEXT NOT NULL,
    first_source_offset INTEGER NOT NULL,
    last_source_offset INTEGER NOT NULL,
    character_name TEXT NOT NULL,
    mob_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    purged_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reason TEXT NOT NULL DEFAULT 'retention',
    UNIQUE(source_file, first_source_offset)
);

CREATE INDEX IF NOT EXISTS idx_purged_damage_ranges_source_offsets
ON purged_damage_encounter_ranges(source_file, first_source_offset, last_source_offset);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(34, 'protected combat fights and durable purge ranges');
