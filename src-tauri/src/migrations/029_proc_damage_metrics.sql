CREATE TABLE IF NOT EXISTS proc_occurrences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    dot_application_id INTEGER REFERENCES dot_applications(id) ON DELETE SET NULL,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    target_name TEXT NOT NULL COLLATE NOCASE,
    caster_name TEXT NOT NULL COLLATE NOCASE,
    happened_at TEXT NOT NULL,
    direct_damage INTEGER NOT NULL DEFAULT 0,
    damage_event_id INTEGER REFERENCES damage_events(id) ON DELETE SET NULL,
    source_file TEXT NOT NULL,
    landing_source_offset INTEGER NOT NULL,
    attribution_source_offset INTEGER NOT NULL,
    UNIQUE(source_file, landing_source_offset, spell_name)
);

CREATE INDEX IF NOT EXISTS idx_proc_occurrences_encounter
ON proc_occurrences(encounter_id, caster_name COLLATE NOCASE, spell_name COLLATE NOCASE);

CREATE TABLE IF NOT EXISTS damage_spell_summaries (
    encounter_id INTEGER NOT NULL REFERENCES damage_encounters(id) ON DELETE CASCADE,
    caster_name TEXT NOT NULL COLLATE NOCASE,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    proc_count INTEGER NOT NULL DEFAULT 0,
    direct_proc_damage INTEGER NOT NULL DEFAULT 0,
    dot_damage INTEGER NOT NULL DEFAULT 0,
    dot_tick_count INTEGER NOT NULL DEFAULT 0,
    proc_dot_damage INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(encounter_id, caster_name, spell_name)
);

CREATE INDEX IF NOT EXISTS idx_damage_spell_summaries_encounter
ON damage_spell_summaries(encounter_id, caster_name COLLATE NOCASE);

-- Preserve metrics already inferable from schema 28 without requiring a destructive rescan.
INSERT OR IGNORE INTO proc_occurrences(
    encounter_id,dot_application_id,spell_name,target_name,caster_name,happened_at,
    direct_damage,source_file,landing_source_offset,attribution_source_offset
)
SELECT encounter_id,id,spell_name,target_name,caster_name,landed_at,0,
       source_file,source_offset,source_offset
FROM dot_applications
WHERE attribution_method='proc';

INSERT INTO damage_spell_summaries(
    encounter_id,caster_name,spell_name,dot_damage,dot_tick_count,proc_dot_damage
)
SELECT a.encounter_id,a.caster_name,a.spell_name,
       COALESCE(SUM(e.damage),0),COUNT(e.id),
       COALESCE(SUM(CASE WHEN a.attribution_method='proc' THEN e.damage ELSE 0 END),0)
FROM dot_applications a
JOIN damage_events e ON e.source_file='dot://' || a.id
GROUP BY a.encounter_id,a.caster_name,a.spell_name
ON CONFLICT(encounter_id,caster_name,spell_name) DO UPDATE SET
    dot_damage=excluded.dot_damage,
    dot_tick_count=excluded.dot_tick_count,
    proc_dot_damage=excluded.proc_dot_damage;

INSERT INTO damage_spell_summaries(encounter_id,caster_name,spell_name,proc_count)
SELECT encounter_id,caster_name,spell_name,COUNT(*)
FROM proc_occurrences
GROUP BY encounter_id,caster_name,spell_name
ON CONFLICT(encounter_id,caster_name,spell_name) DO UPDATE SET
    proc_count=excluded.proc_count;
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(29, 'proc damage metrics');