UPDATE spawn_timer_overrides
SET short_name='TwentyTwo',updated_at=CURRENT_TIMESTAMP
WHERE nickname='22'
  AND (short_name IS NULL OR TRIM(short_name)='')
  AND source IN ('bundled-json','p99-wiki');

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(50,'twenty two combat alias');
