CREATE TABLE IF NOT EXISTS spawn_timer_overrides (
    nickname TEXT PRIMARY KEY COLLATE NOCASE,
    mob_name TEXT NOT NULL COLLATE NOCASE,
    short_name TEXT COLLATE NOCASE,
    zone_name TEXT COLLATE NOCASE,
    continent TEXT,
    respawn_minutes INTEGER NOT NULL CHECK(respawn_minutes >= 0),
    window_minutes INTEGER NOT NULL DEFAULT 0 CHECK(window_minutes >= 0),
    windows_json TEXT,
    last_known_tod TEXT,
    source TEXT NOT NULL DEFAULT 'manual',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_spawn_timer_override_mob
ON spawn_timer_overrides(mob_name COLLATE NOCASE);

INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('kozz','Guardian Kozzalym',NULL,NULL,NULL,7200,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('wuoshi','Wuoshi',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('eashen','Eashen of the Sky',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('phinny','Phinigel Autropos',NULL,NULL,NULL,720,0,NULL,'2026-09-09T11:09:44.7036332-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('telk','Telkorenar',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('draco','Dracoliche',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('inny','Innoruuk',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('fright','Fright',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('cekenr','Cekenar',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('grink','Grink',NULL,NULL,NULL,70,0,NULL,'2025-06-19T15:00:51.5468547-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('ltk','Lendiniara the Keeper',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('brenn','Brenn',NULL,NULL,NULL,70,0,NULL,'2025-06-19T16:09:56.6514621-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('statue','The Statue of Rallos Zek',NULL,NULL,NULL,7200,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('gozz','Gozzrem',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('vindi','Derakor the Vindicator',NULL,NULL,NULL,420,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('vs','Venril Sathir',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('vyemm','Lord Vyemm',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('vilefang','Vilefang',NULL,NULL,NULL,1440,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('zlexak','Zlexak',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('trakanon','Trakanon',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('dagarn','Dagarn the Destroyer',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('jorrleag','Jorlleag',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('talendor','Talendor',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('ladym','Lady Mirenilla',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('ladyn','Lady Nevederia',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('glonk','Glonk',NULL,NULL,NULL,70,0,NULL,'2025-06-19T16:16:12.3125719-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('feshlak','Lord Feshlak',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('gore','Gorenaire',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('faydedar','Faydedar',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('sevalak','Sevalak',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('koi','Lord Koi''Doken',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('22','Twenty Two',NULL,NULL,NULL,70,0,NULL,'2025-06-19T16:30:18.4584884-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('lodi','Lodizal',NULL,NULL,NULL,1080,0,NULL,'2026-09-01T04:35:50.6050366-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('magi','Magi P''tasa',NULL,NULL,NULL,3600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('ikatiar','Ikatiar the Venom',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('velk','Velketor the Sorcerer',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('sev','Severilous',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('vulak','Vulak''Aerr',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('kreizenn','Lord Kreizenn',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('aary','Aaryonar',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('angry','an angry goblin',NULL,NULL,NULL,4320,0,NULL,'2025-09-12T06:32:30.8723929-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('narandi','Sentry Badain',NULL,NULL,NULL,4350,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('zlandicar','Zlandicar',NULL,NULL,NULL,6000,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('dread','Dread',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('terror','Terror',NULL,NULL,NULL,3840,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('scout','Scout',NULL,NULL,NULL,600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('kdt','Keldor Dek''Torek',NULL,NULL,NULL,1080,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('dozekar','Dozekar the Cursed',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('grenn','Grenn',NULL,NULL,NULL,70,0,NULL,'2025-06-19T16:14:17.7536342-06:00','bundled-json');
INSERT OR IGNORE INTO spawn_timer_overrides(nickname,mob_name,short_name,zone_name,continent,respawn_minutes,window_minutes,windows_json,last_known_tod,source) VALUES('kd','Kelorek''Dar',NULL,NULL,NULL,9600,0,NULL,NULL,'bundled-json');

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (43, 'spawn timer override catalog');
