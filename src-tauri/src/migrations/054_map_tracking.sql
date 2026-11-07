CREATE TABLE IF NOT EXISTS character_map_state (
    character_name TEXT PRIMARY KEY COLLATE NOCASE,
    zone_name TEXT NOT NULL DEFAULT '',
    north_south REAL,
    east_west REAL,
    elevation REAL,
    heading_degrees REAL,
    happened_at TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(54, 'map_tracking');
