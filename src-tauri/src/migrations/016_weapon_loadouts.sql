CREATE TABLE IF NOT EXISTS character_weapon_loadouts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_name TEXT NOT NULL COLLATE NOCASE,
    captured_at TEXT NOT NULL,
    primary_weapon_name TEXT,
    primary_item_id INTEGER,
    secondary_weapon_name TEXT,
    secondary_item_id INTEGER,
    source_file TEXT NOT NULL
);

ALTER TABLE damage_events ADD COLUMN weapon_loadout_id INTEGER
    REFERENCES character_weapon_loadouts(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_weapon_loadouts_character_time
    ON character_weapon_loadouts(character_name COLLATE NOCASE, captured_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_damage_events_loadout
    ON damage_events(weapon_loadout_id);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(16, 'character weapon loadout history');
