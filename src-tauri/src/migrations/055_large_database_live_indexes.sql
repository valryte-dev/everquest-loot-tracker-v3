-- Keep live combat work proportional to the current encounter rather than total history.
CREATE INDEX IF NOT EXISTS idx_combat_spell_activity_live_attribution
ON combat_spell_activity(
    source_file,
    target_name COLLATE NOCASE,
    caster_name COLLATE NOCASE,
    happened_at DESC,
    id DESC
)
WHERE source_kind IN ('direct','item_click','proc');

CREATE INDEX IF NOT EXISTS idx_damage_encounters_last_damage
ON damage_encounters(last_damage_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_dot_applications_live_status
ON dot_applications(status, expires_at, id);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(55,'large database live indexes');
