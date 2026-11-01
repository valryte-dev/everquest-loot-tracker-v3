-- Zero-damage combat attempts used to seed participant summaries and could
-- move first_damage_at earlier than the first actual hit. Remove those rows;
-- the startup summary worker will rebuild every real participant from the
-- canonical damage_events table in visible, progress-reported batches.
DELETE FROM damage_participant_summaries
WHERE NOT EXISTS (
    SELECT 1
    FROM damage_events event
    WHERE event.encounter_id=damage_participant_summaries.encounter_id
      AND COALESCE(NULLIF(event.attacker_name,''),'Unknown')
          =damage_participant_summaries.attacker_name COLLATE NOCASE
);

DELETE FROM app_settings WHERE key='damage_summary_backfill_cursor';

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(53,'repair damage DPS summaries');
