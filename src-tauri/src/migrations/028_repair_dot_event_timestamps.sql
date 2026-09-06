CREATE TEMP TABLE affected_dot_encounters AS
SELECT DISTINCT encounter_id
FROM damage_events
WHERE source_file LIKE 'dot://%' AND happened_at='spell';

UPDATE damage_events
SET happened_at=COALESCE((
        SELECT datetime(
            application.landed_at,
            '+' || (damage_events.source_offset * application.tick_interval_seconds) || ' seconds'
        )
        FROM dot_applications application
        WHERE damage_events.source_file='dot://' || application.id
    ), happened_at),
    damage_type='spell'
WHERE source_file LIKE 'dot://%' AND happened_at='spell';

DELETE FROM damage_participant_summaries
WHERE encounter_id IN (SELECT encounter_id FROM affected_dot_encounters);

INSERT INTO damage_participant_summaries(
    encounter_id,attacker_name,total_damage,hit_count,first_damage_at,last_damage_at
)
SELECT encounter_id,COALESCE(NULLIF(attacker_name,''),'Unknown'),SUM(damage),COUNT(*),
       MIN(happened_at),MAX(happened_at)
FROM damage_events
WHERE encounter_id IN (SELECT encounter_id FROM affected_dot_encounters)
GROUP BY encounter_id,COALESCE(NULLIF(attacker_name,''),'Unknown') COLLATE NOCASE;

DROP TABLE affected_dot_encounters;

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(28,'repair inferred dot event timestamps');