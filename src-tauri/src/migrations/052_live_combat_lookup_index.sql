CREATE INDEX IF NOT EXISTS idx_damage_encounters_live_lookup
ON damage_encounters(
    source_file,
    character_name COLLATE NOCASE,
    mob_name COLLATE NOCASE,
    last_source_offset DESC
)
WHERE outcome='active';

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(52,'live combat lookup index');
