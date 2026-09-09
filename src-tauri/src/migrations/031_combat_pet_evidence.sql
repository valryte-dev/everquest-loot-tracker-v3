CREATE TABLE IF NOT EXISTS combat_pet_evidence (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    character_name TEXT NOT NULL COLLATE NOCASE,
    pet_name TEXT NOT NULL COLLATE NOCASE,
    target_name TEXT NOT NULL COLLATE NOCASE,
    last_seen_at TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_combat_pet_evidence_lookup
ON combat_pet_evidence(source_file, character_name COLLATE NOCASE, pet_name COLLATE NOCASE, last_seen_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(31, 'combat pet evidence');