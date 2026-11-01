INSERT OR IGNORE INTO spawn_timer_overrides(
    nickname,mob_name,short_name,zone_name,continent,respawn_minutes,
    window_minutes,windows_json,last_known_tod,last_known_spawn_at,source,updated_at
) VALUES(
    'tormax','King Tormax',NULL,'Kael Drakkel','Velious',10080,
    480,'{"sourceUrl":"https://wiki.project1999.com/King_Tormax"}',NULL,NULL,
    'p99-wiki',CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(48,'king tormax spawn timer');
