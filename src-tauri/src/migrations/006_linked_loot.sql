CREATE TABLE IF NOT EXISTS linked_loot_items (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    channel TEXT NOT NULL CHECK(channel IN ('group', 'guild')),
    speaker_name TEXT NOT NULL COLLATE NOCASE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    link_index INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_file, source_offset, link_index)
);

CREATE INDEX IF NOT EXISTS idx_linked_loot_time
ON linked_loot_items(happened_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_linked_loot_item
ON linked_loot_items(item_name COLLATE NOCASE);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (6, 'group and guild linked loot');
