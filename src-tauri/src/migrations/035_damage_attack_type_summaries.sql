CREATE TABLE IF NOT EXISTS damage_attack_type_summaries (
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    attack_kind TEXT NOT NULL COLLATE NOCASE,
    damage_type TEXT NOT NULL,
    total_damage INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    max_hit INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(encounter_id, attack_kind, damage_type)
);

CREATE TRIGGER IF NOT EXISTS trg_damage_attack_type_summary_insert
AFTER INSERT ON damage_events
WHEN NEW.damage>0 AND COALESCE(TRIM(NEW.attack_kind),'')<>''
BEGIN
    INSERT INTO damage_attack_type_summaries(encounter_id,attack_kind,damage_type,total_damage,hit_count,max_hit)
    VALUES(NEW.encounter_id,NEW.attack_kind,NEW.damage_type,NEW.damage,1,NEW.damage)
    ON CONFLICT(encounter_id,attack_kind,damage_type) DO UPDATE SET
        total_damage=total_damage+NEW.damage, hit_count=hit_count+1, max_hit=MAX(max_hit,NEW.damage);
END;

CREATE TRIGGER IF NOT EXISTS trg_damage_attack_type_summary_delete
AFTER DELETE ON damage_events
WHEN OLD.damage>0 AND COALESCE(TRIM(OLD.attack_kind),'')<>''
BEGIN
    UPDATE damage_attack_type_summaries
    SET total_damage=MAX(0,total_damage-OLD.damage),hit_count=MAX(0,hit_count-1)
    WHERE encounter_id=OLD.encounter_id AND attack_kind=OLD.attack_kind COLLATE NOCASE AND damage_type=OLD.damage_type;
    DELETE FROM damage_attack_type_summaries
    WHERE encounter_id=OLD.encounter_id AND attack_kind=OLD.attack_kind COLLATE NOCASE AND damage_type=OLD.damage_type AND hit_count=0;
END;

CREATE TRIGGER IF NOT EXISTS trg_damage_attack_type_summary_update
AFTER UPDATE OF encounter_id,attack_kind,damage_type,damage ON damage_events
BEGIN
    UPDATE damage_attack_type_summaries
    SET total_damage=MAX(0,total_damage-OLD.damage),hit_count=MAX(0,hit_count-1)
    WHERE OLD.damage>0 AND encounter_id=OLD.encounter_id AND attack_kind=OLD.attack_kind COLLATE NOCASE AND damage_type=OLD.damage_type;
    DELETE FROM damage_attack_type_summaries
    WHERE encounter_id=OLD.encounter_id AND attack_kind=OLD.attack_kind COLLATE NOCASE AND damage_type=OLD.damage_type AND hit_count=0;
    INSERT INTO damage_attack_type_summaries(encounter_id,attack_kind,damage_type,total_damage,hit_count,max_hit)
    SELECT NEW.encounter_id,NEW.attack_kind,NEW.damage_type,NEW.damage,1,NEW.damage
    WHERE NEW.damage>0 AND COALESCE(TRIM(NEW.attack_kind),'')<>''
    ON CONFLICT(encounter_id,attack_kind,damage_type) DO UPDATE SET
        total_damage=total_damage+NEW.damage, hit_count=hit_count+1, max_hit=MAX(max_hit,NEW.damage);
END;

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(35,'damage attack type summaries');
