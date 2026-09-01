CREATE TABLE IF NOT EXISTS completed_split_payouts (
    completed_split_item_id INTEGER NOT NULL REFERENCES completed_split_items(id) ON DELETE CASCADE,
    member_name TEXT NOT NULL COLLATE NOCASE,
    paid_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY(completed_split_item_id, member_name)
);
INSERT OR IGNORE INTO completed_split_payouts(completed_split_item_id,member_name,paid_at)
SELECT h.id,m.member_name,COALESCE(h.paid_at,h.completed_at,CURRENT_TIMESTAMP)
FROM completed_split_items h JOIN completed_split_members m ON m.completed_split_item_id=h.id
WHERE h.disposition='sold' AND h.payout_status='completed';
INSERT OR IGNORE INTO schema_migrations(version,name) VALUES(9,'individual split payouts');
