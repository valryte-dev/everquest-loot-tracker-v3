CREATE TABLE IF NOT EXISTS planner_upload_jobs (
    id INTEGER PRIMARY KEY,
    file_name TEXT NOT NULL,
    payload_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','running','completed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_error TEXT NOT NULL DEFAULT '',
    review_url TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_planner_upload_jobs_pending
ON planner_upload_jobs(status, next_attempt_at, id);
INSERT OR IGNORE INTO schema_migrations(version, name)
VALUES(22, 'durable planner upload jobs');
