CREATE TABLE IF NOT EXISTS compound_workspace_snapshots (
    id INTEGER PRIMARY KEY,
    source_json TEXT NOT NULL UNIQUE,
    active_project_id TEXT,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS compound_workspace_projects (
    snapshot_id INTEGER NOT NULL REFERENCES compound_workspace_snapshots(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    note TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'building',
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(snapshot_id,project_id)
);
CREATE TABLE IF NOT EXISTS compound_workspace_project_templates (
    snapshot_id INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    template_name TEXT NOT NULL COLLATE NOCASE,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(snapshot_id,project_id,template_name),
    FOREIGN KEY(snapshot_id,project_id) REFERENCES compound_workspace_projects(snapshot_id,project_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS compound_workspace_components (
    snapshot_id INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    required_count INTEGER NOT NULL DEFAULT 1,
    received_count INTEGER NOT NULL DEFAULT 0,
    unit_value_pp INTEGER NOT NULL DEFAULT 0,
    source_kind TEXT NOT NULL DEFAULT 'personal',
    source_reference TEXT,
    note TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(snapshot_id,project_id,component_id),
    FOREIGN KEY(snapshot_id,project_id) REFERENCES compound_workspace_projects(snapshot_id,project_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS compound_workspace_contributors (
    snapshot_id INTEGER NOT NULL,
    project_id TEXT NOT NULL,
    component_id TEXT NOT NULL,
    member_name TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY(snapshot_id,project_id,component_id,member_name),
    FOREIGN KEY(snapshot_id,project_id,component_id) REFERENCES compound_workspace_components(snapshot_id,project_id,component_id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS compound_workspace_templates (
    snapshot_id INTEGER NOT NULL REFERENCES compound_workspace_snapshots(id) ON DELETE CASCADE,
    template_id TEXT NOT NULL,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    name TEXT NOT NULL COLLATE NOCASE,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(snapshot_id,template_id)
);
CREATE TABLE IF NOT EXISTS compound_workspace_template_components (
    id INTEGER PRIMARY KEY,
    snapshot_id INTEGER NOT NULL,
    template_id TEXT NOT NULL,
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    required_count INTEGER NOT NULL DEFAULT 1,
    unit_value_pp INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(snapshot_id,template_id) REFERENCES compound_workspace_templates(snapshot_id,template_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_compound_workspace_projects_item ON compound_workspace_projects(item_id);
CREATE INDEX IF NOT EXISTS idx_compound_workspace_components_item ON compound_workspace_components(item_id);
CREATE INDEX IF NOT EXISTS idx_compound_workspace_contributors_name ON compound_workspace_contributors(member_name COLLATE NOCASE);
INSERT OR IGNORE INTO schema_migrations(version,name) VALUES(25,'append-only normalized compound workspace snapshots');
