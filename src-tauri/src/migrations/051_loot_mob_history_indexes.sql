CREATE INDEX IF NOT EXISTS idx_activity_mob_source_offset
ON activity_mob_history(source_file, source_offset DESC);

CREATE INDEX IF NOT EXISTS idx_activity_mob_character_time
ON activity_mob_history(character_name COLLATE NOCASE, happened_at DESC, id DESC);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(51,'loot mob history indexes');
