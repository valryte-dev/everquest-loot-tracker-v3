ALTER TABLE damage_events ADD COLUMN attacker_name TEXT;

UPDATE damage_events
SET attacker_name = (
    SELECT character_name FROM damage_encounters WHERE id = damage_events.encounter_id
)
WHERE attacker_name IS NULL OR TRIM(attacker_name) = '';

CREATE INDEX IF NOT EXISTS idx_damage_events_encounter_attacker
    ON damage_events(encounter_id, attacker_name COLLATE NOCASE, happened_at);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(17, 'damage event attackers');
