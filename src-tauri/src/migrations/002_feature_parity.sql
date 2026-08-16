CREATE TABLE IF NOT EXISTS spellbook_characters (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    source_file TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS spellbook_spells (
    id INTEGER PRIMARY KEY,
    character_id INTEGER NOT NULL REFERENCES spellbook_characters(id) ON DELETE CASCADE,
    slot_number INTEGER,
    spell_name TEXT NOT NULL COLLATE NOCASE,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS recipe_templates (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    output_item_id INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    is_builtin INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS recipe_components (
    id INTEGER PRIMARY KEY,
    template_id INTEGER NOT NULL REFERENCES recipe_templates(id) ON DELETE CASCADE,
    item_name TEXT NOT NULL COLLATE NOCASE,
    item_id INTEGER,
    required_count INTEGER NOT NULL DEFAULT 1,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS application_logs (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    level TEXT NOT NULL DEFAULT 'info',
    area TEXT NOT NULL DEFAULT 'application',
    message TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS import_uploads (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    file_name TEXT NOT NULL,
    status TEXT NOT NULL,
    review_url TEXT,
    detail TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS idx_spellbook_character ON spellbook_spells(character_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_recipe_component_template ON recipe_components(template_id, sort_order);
CREATE INDEX IF NOT EXISTS idx_application_logs_time ON application_logs(happened_at DESC);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (2, 'feature parity storage');
