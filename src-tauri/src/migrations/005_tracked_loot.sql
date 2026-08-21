CREATE TABLE IF NOT EXISTS tracked_loot_items (
    id INTEGER PRIMARY KEY,
    source_loot_id INTEGER UNIQUE,
    happened_at TEXT NOT NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT,
    looter_name TEXT,
    value_pp INTEGER,
    tracked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS tracked_loot_members (
    tracked_loot_item_id INTEGER NOT NULL REFERENCES tracked_loot_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY(tracked_loot_item_id, member_name)
);

CREATE INDEX IF NOT EXISTS idx_tracked_loot_time ON tracked_loot_items(happened_at DESC, id DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (5, 'independent tracked loot storage');
