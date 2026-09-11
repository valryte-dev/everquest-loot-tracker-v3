CREATE TABLE IF NOT EXISTS item_proc_spells (
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    proc_level INTEGER NOT NULL DEFAULT 1,
    source TEXT NOT NULL DEFAULT 'p99-wiki',
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(item_name, spell_name)
);

CREATE INDEX IF NOT EXISTS idx_item_proc_spells_spell
ON item_proc_spells(spell_name COLLATE NOCASE, item_name COLLATE NOCASE);

CREATE INDEX IF NOT EXISTS idx_item_proc_spells_item_id
ON item_proc_spells(item_id) WHERE item_id IS NOT NULL;

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(32, 'canonical item proc catalog');
