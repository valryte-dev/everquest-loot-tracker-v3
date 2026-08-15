CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS v3_runtime_state (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS v3_compound_projects (
    id TEXT PRIMARY KEY,
    item_id INTEGER,
    item_name TEXT NOT NULL COLLATE NOCASE,
    note TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'building',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS v3_compound_components (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES v3_compound_projects(id) ON DELETE CASCADE,
    item_id INTEGER,
    item_name TEXT NOT NULL COLLATE NOCASE,
    required_count INTEGER NOT NULL DEFAULT 1,
    received_count INTEGER NOT NULL DEFAULT 0,
    unit_value_pp INTEGER NOT NULL DEFAULT 0,
    source_kind TEXT NOT NULL DEFAULT 'personal',
    source_reference TEXT,
    note TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_v3_compound_components_project
    ON v3_compound_components(project_id, sort_order);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES (1, 'v3 foundation');
