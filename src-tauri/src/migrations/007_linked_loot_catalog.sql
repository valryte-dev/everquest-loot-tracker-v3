INSERT OR IGNORE INTO master_items(item_id,item_name,source)
VALUES(4294,'Dusty Rusted Shackles','p99-wiki');

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (7, 'linked loot catalog corrections');
