-- Normalize wiki-documented respawns as nominal time plus per-side variance.
-- Preserve user-edited rows by only enriching the original bundled import.
UPDATE spawn_timer_overrides
SET respawn_minutes=10080, window_minutes=480, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND respawn_minutes=9600;

UPDATE spawn_timer_overrides
SET respawn_minutes=4320, window_minutes=480, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND respawn_minutes=3840;

UPDATE spawn_timer_overrides
SET window_minutes=480, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND nickname='statue';

UPDATE spawn_timer_overrides
SET respawn_minutes=480, window_minutes=60, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND nickname='vindi';

UPDATE spawn_timer_overrides
SET respawn_minutes=4320, window_minutes=720, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND nickname='magi';

UPDATE spawn_timer_overrides
SET respawn_minutes=6480, window_minutes=480, source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND nickname='zlandicar';

UPDATE spawn_timer_overrides
SET window_minutes=60,
    windows_json='{"kind":"cumulativeSkip","maxSkips":2,"sourceUrl":"https://wiki.project1999.com/Lodizal"}',
    zone_name=COALESCE(zone_name,'Iceclad Ocean'), source='p99-wiki', updated_at=CURRENT_TIMESTAMP
WHERE source='bundled-json' AND nickname='lodi';

INSERT OR IGNORE INTO spawn_timer_overrides(
    nickname,mob_name,zone_name,respawn_minutes,window_minutes,windows_json,source
) VALUES(
    'stormfeather','Stormfeather','Iceclad Ocean',1080,60,
    '{"kind":"cumulativeSkip","maxSkips":3,"sourceUrl":"https://wiki.project1999.com/Stormfeather"}',
    'p99-wiki'
);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(44,'spawn timer windows');
