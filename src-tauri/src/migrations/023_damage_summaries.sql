CREATE TABLE IF NOT EXISTS damage_participant_summaries (
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    attacker_name TEXT NOT NULL COLLATE NOCASE,
    total_damage INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    first_damage_at TEXT NOT NULL,
    last_damage_at TEXT NOT NULL,
    PRIMARY KEY(encounter_id, attacker_name)
);
CREATE TABLE IF NOT EXISTS damage_target_summaries (
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    target_name TEXT NOT NULL COLLATE NOCASE,
    total_damage INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    max_hit INTEGER NOT NULL DEFAULT 0,
    first_damage_at TEXT NOT NULL,
    last_damage_at TEXT NOT NULL,
    PRIMARY KEY(encounter_id, target_name)
);
CREATE INDEX IF NOT EXISTS idx_damage_encounters_active_recent
ON damage_encounters(outcome, last_damage_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_damage_received_encounter_target
ON damage_received_events(encounter_id, target_name COLLATE NOCASE, happened_at);
CREATE TRIGGER IF NOT EXISTS trg_damage_participant_summary_insert
AFTER INSERT ON damage_events
BEGIN
    INSERT INTO damage_participant_summaries(encounter_id,attacker_name,total_damage,hit_count,first_damage_at,last_damage_at)
    VALUES(NEW.encounter_id,COALESCE(NULLIF(NEW.attacker_name,''),'Unknown'),NEW.damage,1,NEW.happened_at,NEW.happened_at)
    ON CONFLICT(encounter_id,attacker_name) DO UPDATE SET
        total_damage=total_damage+NEW.damage,
        hit_count=hit_count+1,
        first_damage_at=MIN(first_damage_at,NEW.happened_at),
        last_damage_at=MAX(last_damage_at,NEW.happened_at);
END;
CREATE TRIGGER IF NOT EXISTS trg_damage_target_summary_insert
AFTER INSERT ON damage_received_events
BEGIN
    INSERT INTO damage_target_summaries(encounter_id,target_name,total_damage,hit_count,max_hit,first_damage_at,last_damage_at)
    VALUES(NEW.encounter_id,NEW.target_name,NEW.damage,1,NEW.damage,NEW.happened_at,NEW.happened_at)
    ON CONFLICT(encounter_id,target_name) DO UPDATE SET
        total_damage=total_damage+NEW.damage,
        hit_count=hit_count+1,
        max_hit=MAX(max_hit,NEW.damage),
        first_damage_at=MIN(first_damage_at,NEW.happened_at),
        last_damage_at=MAX(last_damage_at,NEW.happened_at);
END;
INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(23,'damage summary read model');
