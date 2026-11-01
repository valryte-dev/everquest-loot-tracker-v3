-- Angry's variance begins at the nominal respawn time; it does not open early.
UPDATE spawn_timer_overrides
SET windows_json='{"shape":"after","sourceUrl":"https://wiki.project1999.com/An_angry_goblin"}',
    updated_at=CURRENT_TIMESTAMP
WHERE nickname='angry' COLLATE NOCASE
  AND (windows_json IS NULL OR TRIM(windows_json)='');

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(47,'spawn timer window shapes');
