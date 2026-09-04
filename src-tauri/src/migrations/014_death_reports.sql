CREATE TABLE IF NOT EXISTS death_reports (
    id INTEGER PRIMARY KEY,
    happened_at TEXT NOT NULL,
    character_name TEXT NOT NULL COLLATE NOCASE,
    killer_name TEXT NOT NULL COLLATE NOCASE,
    raw_line TEXT NOT NULL,
    source_file TEXT NOT NULL,
    source_offset INTEGER NOT NULL,
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_file, source_offset)
);

CREATE INDEX IF NOT EXISTS idx_death_reports_happened
ON death_reports(happened_at DESC);

CREATE INDEX IF NOT EXISTS idx_death_reports_character
ON death_reports(character_name COLLATE NOCASE, happened_at DESC);

CREATE TABLE IF NOT EXISTS death_report_entries (
    death_report_id INTEGER NOT NULL REFERENCES death_reports(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL,
    raw_line TEXT NOT NULL,
    PRIMARY KEY(death_report_id, sequence_number)
);

CREATE TABLE IF NOT EXISTS death_report_scan_cursors (
    source_file TEXT PRIMARY KEY,
    character_name TEXT NOT NULL COLLATE NOCASE,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    context_json TEXT NOT NULL DEFAULT '[]',
    scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(14, 'death reports with preceding log context');
