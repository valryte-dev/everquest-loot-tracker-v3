CREATE TABLE IF NOT EXISTS known_members (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS current_group (
    member_id INTEGER PRIMARY KEY REFERENCES known_members(id) ON DELETE CASCADE,
    joined_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS character_aliases (
    alias_name TEXT PRIMARY KEY COLLATE NOCASE,
    canonical_name TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS mobs (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS loot_drops (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    item_name TEXT NOT NULL,
    mob_name TEXT,
    mob_id INTEGER REFERENCES mobs(id) ON DELETE SET NULL,
    looter_name TEXT,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_file, source_offset, raw_line)
);
CREATE TABLE IF NOT EXISTS loot_drop_members (
    loot_drop_id INTEGER NOT NULL REFERENCES loot_drops(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY (loot_drop_id, member_name)
);
CREATE TABLE IF NOT EXISTS split_loot_items (
    id INTEGER PRIMARY KEY,
    loot_drop_id INTEGER NOT NULL UNIQUE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT,
    looter_name TEXT,
    payout_value_pp INTEGER,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS split_loot_members (
    split_loot_item_id INTEGER NOT NULL REFERENCES split_loot_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY(split_loot_item_id, member_name)
);
CREATE TABLE IF NOT EXISTS manual_split_list_items (
    id INTEGER PRIMARY KEY,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_id INTEGER REFERENCES mobs(id) ON DELETE SET NULL,
    looter_name TEXT,
    payout_value_pp INTEGER,
    added_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS manual_split_list_members (
    split_list_item_id INTEGER NOT NULL REFERENCES manual_split_list_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY(split_list_item_id, member_name)
);
CREATE TABLE IF NOT EXISTS completed_split_items (
    id INTEGER PRIMARY KEY,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT,
    looter_name TEXT,
    value_pp INTEGER NOT NULL DEFAULT 0,
    disposition TEXT NOT NULL CHECK(disposition IN ('sold', 'consumed')),
    note TEXT NOT NULL DEFAULT '',
    completed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS completed_split_members (
    completed_split_item_id INTEGER NOT NULL REFERENCES completed_split_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL,
    PRIMARY KEY(completed_split_item_id, member_name)
);
CREATE TABLE IF NOT EXISTS item_market_values (
    server TEXT NOT NULL COLLATE NOCASE,
    source_item_id INTEGER NOT NULL,
    transaction_type INTEGER NOT NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    last_seen TEXT NOT NULL,
    current_count INTEGER NOT NULL DEFAULT 0,
    current_average_pp INTEGER NOT NULL DEFAULT 0,
    count_30d INTEGER NOT NULL DEFAULT 0,
    average_30d_pp INTEGER NOT NULL DEFAULT 0,
    count_60d INTEGER NOT NULL DEFAULT 0,
    average_60d_pp INTEGER NOT NULL DEFAULT 0,
    count_90d INTEGER NOT NULL DEFAULT 0,
    average_90d_pp INTEGER NOT NULL DEFAULT 0,
    count_6m INTEGER NOT NULL DEFAULT 0,
    average_6m_pp INTEGER NOT NULL DEFAULT 0,
    count_all INTEGER NOT NULL DEFAULT 0,
    average_all_pp INTEGER NOT NULL DEFAULT 0,
    fetched_at TEXT NOT NULL,
    is_manual INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(server, source_item_id, transaction_type)
);
CREATE TABLE IF NOT EXISTS inventory_characters (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    source_file TEXT NOT NULL,
    imported_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS inventory_items (
    id INTEGER PRIMARY KEY,
    character_id INTEGER NOT NULL REFERENCES inventory_characters(id) ON DELETE CASCADE,
    location TEXT NOT NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    item_id INTEGER,
    item_count INTEGER NOT NULL DEFAULT 1,
    slots INTEGER,
    sort_order INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS wts_groups (
    id INTEGER PRIMARY KEY,
    character_name TEXT NOT NULL COLLATE NOCASE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS wts_group_items (
    wts_group_id INTEGER NOT NULL REFERENCES wts_groups(id) ON DELETE CASCADE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    item_id INTEGER,
    sort_order INTEGER NOT NULL,
    PRIMARY KEY(wts_group_id, item_name)
);
CREATE INDEX IF NOT EXISTS idx_loot_drops_happened_at ON loot_drops(happened_at DESC);
CREATE INDEX IF NOT EXISTS idx_market_item_match ON item_market_values(server, item_name, transaction_type);
CREATE INDEX IF NOT EXISTS idx_inventory_character ON inventory_items(character_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_wts_groups_character ON wts_groups(character_name, updated_at DESC);
