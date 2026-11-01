ALTER TABLE spawn_timer_overrides ADD COLUMN last_known_spawn_at TEXT;

INSERT OR IGNORE INTO schema_migrations (version, name)
VALUES (45, 'spawn timer manual anchors');
