ALTER TABLE completed_split_items ADD COLUMN payout_status TEXT NOT NULL DEFAULT 'pending' CHECK(payout_status IN ('pending', 'completed'));
ALTER TABLE completed_split_items ADD COLUMN paid_at TEXT;
UPDATE completed_split_items SET payout_status='completed', paid_at=COALESCE(paid_at,completed_at) WHERE disposition='consumed';
INSERT OR IGNORE INTO schema_migrations(version, name) VALUES(8, 'split payout phases');
