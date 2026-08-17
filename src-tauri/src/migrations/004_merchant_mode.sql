CREATE TABLE IF NOT EXISTS merchant_messages (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('wts', 'wtb', 'tell')),
    speaker_name TEXT NOT NULL COLLATE NOCASE,
    message TEXT NOT NULL,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_file, source_offset, raw_line)
);

CREATE TABLE IF NOT EXISTS merchant_message_items (
    id INTEGER PRIMARY KEY,
    merchant_message_id INTEGER NOT NULL REFERENCES merchant_messages(id) ON DELETE CASCADE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    asking_price_pp INTEGER,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_merchant_messages_kind_time
    ON merchant_messages(kind, happened_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_merchant_message_items_message
    ON merchant_message_items(merchant_message_id, sort_order);

INSERT OR IGNORE INTO app_settings(key, value)
VALUES ('merchant_mode_enabled', 'false');

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (4, 'merchant mode activity');
