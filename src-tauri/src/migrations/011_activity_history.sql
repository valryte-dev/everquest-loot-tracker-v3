CREATE TABLE IF NOT EXISTS log_history_cursors (
    source_file TEXT PRIMARY KEY,
    character_name TEXT NOT NULL COLLATE NOCASE,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS activity_loot_history (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL COLLATE NOCASE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    looter_name TEXT NOT NULL COLLATE NOCASE,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(character_name, happened_at, raw_line)
);

CREATE TABLE IF NOT EXISTS activity_mob_history (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT NOT NULL COLLATE NOCASE,
    killer_name TEXT COLLATE NOCASE,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(character_name, happened_at, raw_line)
);

CREATE TABLE IF NOT EXISTS activity_offer_history (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL COLLATE NOCASE,
    offerer_name TEXT NOT NULL COLLATE NOCASE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    item_index INTEGER NOT NULL DEFAULT 0,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(character_name, happened_at, raw_line, item_index)
);

CREATE INDEX IF NOT EXISTS idx_activity_loot_time
ON activity_loot_history(happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_loot_item
ON activity_loot_history(item_name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_activity_mob_time
ON activity_mob_history(happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_mob_name
ON activity_mob_history(mob_name COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_activity_offer_time
ON activity_offer_history(happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_activity_offer_item
ON activity_offer_history(item_name COLLATE NOCASE);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (11, 'cross-character activity history');
