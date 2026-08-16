CREATE TABLE IF NOT EXISTS master_items (
    item_id INTEGER PRIMARY KEY,
    item_name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    source TEXT NOT NULL DEFAULT 'market',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO master_items(item_id,item_name,source,updated_at)
SELECT source_item_id,item_name,CASE WHEN is_manual=1 THEN 'manual' ELSE 'market' END,fetched_at
FROM item_market_values
WHERE server='Green' COLLATE NOCASE AND transaction_type=0
ORDER BY is_manual DESC,count_30d DESC;

INSERT OR REPLACE INTO master_items(item_id,item_name,source,updated_at)
SELECT i.item_id,i.item_name,'inventory',MAX(c.imported_at)
FROM inventory_items i
JOIN inventory_characters c ON c.id=i.character_id
WHERE i.item_id IS NOT NULL AND i.item_id>0 AND TRIM(i.item_name)<>''
GROUP BY i.item_id,i.item_name;

UPDATE wts_group_items
SET item_id=(SELECT m.item_id FROM master_items m WHERE m.item_name=wts_group_items.item_name COLLATE NOCASE LIMIT 1)
WHERE EXISTS(SELECT 1 FROM master_items m WHERE m.item_name=wts_group_items.item_name COLLATE NOCASE);

UPDATE recipe_components
SET item_id=(SELECT m.item_id FROM master_items m WHERE m.item_name=recipe_components.item_name COLLATE NOCASE LIMIT 1)
WHERE EXISTS(SELECT 1 FROM master_items m WHERE m.item_name=recipe_components.item_name COLLATE NOCASE);

UPDATE recipe_templates
SET output_item_id=(SELECT m.item_id FROM master_items m WHERE m.item_name=recipe_templates.name COLLATE NOCASE LIMIT 1)
WHERE EXISTS(SELECT 1 FROM master_items m WHERE m.item_name=recipe_templates.name COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_master_items_name ON master_items(item_name COLLATE NOCASE);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (3, 'authoritative master item catalog');
