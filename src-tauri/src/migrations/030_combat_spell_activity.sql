CREATE TABLE IF NOT EXISTS combat_spell_activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    target_name TEXT NOT NULL COLLATE NOCASE,
    caster_name TEXT NOT NULL COLLATE NOCASE,
    source_kind TEXT NOT NULL DEFAULT 'unknown'
        CHECK(source_kind IN ('direct','proc','item_click','unknown')),
    source_name TEXT,
    happened_at TEXT NOT NULL,
    source_file TEXT NOT NULL,
    landing_source_offset INTEGER NOT NULL,
    attribution_source_offset INTEGER,
    UNIQUE(source_file, landing_source_offset, spell_name)
);

CREATE INDEX IF NOT EXISTS idx_combat_spell_activity_encounter
ON combat_spell_activity(encounter_id, happened_at DESC, id DESC);

-- Preserve recognized DoT landings from schema 29. Item names were not retained before schema 30.
INSERT OR IGNORE INTO combat_spell_activity(
    encounter_id,spell_name,target_name,caster_name,source_kind,source_name,
    happened_at,source_file,landing_source_offset,attribution_source_offset
)
SELECT encounter_id,spell_name,target_name,caster_name,
       CASE attribution_method
           WHEN 'direct_cast' THEN 'direct'
           WHEN 'proc' THEN 'proc'
           WHEN 'item_glow' THEN 'item_click'
           ELSE 'unknown'
       END,
       NULL,landed_at,source_file,source_offset,NULL
FROM dot_applications;

UPDATE combat_spell_activity
SET source_kind='proc',
    caster_name=(SELECT p.caster_name FROM proc_occurrences p
                 WHERE p.source_file=combat_spell_activity.source_file
                   AND p.landing_source_offset=combat_spell_activity.landing_source_offset
                   AND p.spell_name=combat_spell_activity.spell_name COLLATE NOCASE),
    attribution_source_offset=(SELECT p.attribution_source_offset FROM proc_occurrences p
                               WHERE p.source_file=combat_spell_activity.source_file
                                 AND p.landing_source_offset=combat_spell_activity.landing_source_offset
                                 AND p.spell_name=combat_spell_activity.spell_name COLLATE NOCASE)
WHERE EXISTS(SELECT 1 FROM proc_occurrences p
             WHERE p.source_file=combat_spell_activity.source_file
               AND p.landing_source_offset=combat_spell_activity.landing_source_offset
               AND p.spell_name=combat_spell_activity.spell_name COLLATE NOCASE);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(30, 'combat spell activity');