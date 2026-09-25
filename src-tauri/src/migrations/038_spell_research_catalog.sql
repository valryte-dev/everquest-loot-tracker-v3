CREATE TABLE IF NOT EXISTS spell_research_recipes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_key TEXT NOT NULL UNIQUE,
    class_name TEXT NOT NULL CHECK(class_name IN ('Enchanter','Magician','Necromancer','Wizard')),
    spell_level INTEGER NOT NULL CHECK(spell_level > 0),
    spell_name TEXT NOT NULL,
    spell_item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    trivial TEXT NOT NULL DEFAULT '',
    research_only INTEGER NOT NULL DEFAULT 0 CHECK(research_only IN (0,1)),
    availability TEXT NOT NULL DEFAULT '',
    source_url TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS spell_research_components (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recipe_id INTEGER NOT NULL REFERENCES spell_research_recipes(id) ON DELETE CASCADE,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL,
    icon_id INTEGER,
    quantity INTEGER NOT NULL DEFAULT 1 CHECK(quantity > 0),
    component_kind TEXT NOT NULL CHECK(component_kind IN ('word','page','rune','spell','other')),
    UNIQUE(recipe_id,item_name)
);

CREATE INDEX IF NOT EXISTS idx_spell_research_class_level
    ON spell_research_recipes(class_name,spell_level,spell_name);
CREATE INDEX IF NOT EXISTS idx_spell_research_component_item
    ON spell_research_components(item_id,item_name);

INSERT OR IGNORE INTO schema_migrations(version,name)
VALUES(38,'spell research recipe catalog');