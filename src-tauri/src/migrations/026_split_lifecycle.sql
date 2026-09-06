CREATE TABLE IF NOT EXISTS split_lifecycle_snapshots (
    id INTEGER PRIMARY KEY,
    source_json TEXT NOT NULL UNIQUE,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS split_lifecycle_records (
    snapshot_id INTEGER NOT NULL REFERENCES split_lifecycle_snapshots(id) ON DELETE CASCADE,
    legacy_key TEXT NOT NULL,
    phase TEXT NOT NULL CHECK(phase IN ('looted','sold_pending','paid','consumed')),
    item_id INTEGER REFERENCES master_items(item_id) ON DELETE SET NULL,
    item_name TEXT NOT NULL COLLATE NOCASE,
    mob_name TEXT,
    looted_by TEXT,
    held_or_sold_by TEXT,
    value_pp INTEGER NOT NULL DEFAULT 0,
    note TEXT NOT NULL DEFAULT '',
    happened_at TEXT NOT NULL,
    paid_at TEXT,
    PRIMARY KEY(snapshot_id,legacy_key)
);
CREATE TABLE IF NOT EXISTS split_lifecycle_participants (
    snapshot_id INTEGER NOT NULL,
    legacy_key TEXT NOT NULL,
    member_name TEXT NOT NULL COLLATE NOCASE,
    paid_at TEXT,
    PRIMARY KEY(snapshot_id,legacy_key,member_name),
    FOREIGN KEY(snapshot_id,legacy_key) REFERENCES split_lifecycle_records(snapshot_id,legacy_key) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_split_lifecycle_phase ON split_lifecycle_records(snapshot_id,phase);
CREATE INDEX IF NOT EXISTS idx_split_lifecycle_item ON split_lifecycle_records(item_id);
CREATE INDEX IF NOT EXISTS idx_split_lifecycle_member ON split_lifecycle_participants(member_name COLLATE NOCASE);
INSERT OR IGNORE INTO schema_migrations(version,name) VALUES(26,'append-only unified split lifecycle snapshots');
