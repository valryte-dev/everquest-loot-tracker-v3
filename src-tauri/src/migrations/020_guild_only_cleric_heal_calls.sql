DELETE FROM cleric_heal_calls
WHERE LOWER(channel) <> 'guild';

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(20, 'guild-only cleric heal calls');
