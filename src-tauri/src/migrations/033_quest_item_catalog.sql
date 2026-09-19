CREATE TABLE IF NOT EXISTS quest_catalog_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_key TEXT NOT NULL UNIQUE,
    category TEXT NOT NULL CHECK(category IN ('plane_of_sky','velious_armor','epic')),
    class_name TEXT NOT NULL,
    quest_name TEXT NOT NULL,
    reward_item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    reward_name TEXT NOT NULL,
    reward_icon_id INTEGER,
    slot TEXT NOT NULL DEFAULT '',
    faction TEXT NOT NULL DEFAULT '',
    note TEXT NOT NULL DEFAULT '',
    source_url TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS quest_catalog_components (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    quest_entry_id INTEGER NOT NULL REFERENCES quest_catalog_entries(id) ON DELETE CASCADE,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL,
    icon_id INTEGER,
    quantity INTEGER NOT NULL DEFAULT 1 CHECK(quantity > 0),
    UNIQUE(quest_entry_id,item_name)
);

CREATE INDEX IF NOT EXISTS idx_quest_catalog_category_class
    ON quest_catalog_entries(category,class_name,quest_name);
CREATE INDEX IF NOT EXISTS idx_quest_catalog_reward_item
    ON quest_catalog_entries(reward_item_id);
CREATE INDEX IF NOT EXISTS idx_quest_components_item
    ON quest_catalog_components(item_id,item_name);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(33,'quest item catalog');
