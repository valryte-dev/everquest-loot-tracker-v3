INSERT OR IGNORE INTO master_items(item_id,item_name,source,updated_at)
SELECT source_item_id,item_name,CASE WHEN is_manual=1 THEN 'manual' ELSE 'market' END,fetched_at
FROM item_market_values
WHERE server='Green' COLLATE NOCASE AND source_item_id>0
ORDER BY is_manual DESC,transaction_type,count_30d DESC;

UPDATE manual_split_list_items SET payout_value_pp=NULL WHERE payout_value_pp<=0;
UPDATE split_loot_items SET payout_value_pp=NULL WHERE payout_value_pp<=0;

CREATE TABLE IF NOT EXISTS resolved_item_values (
 item_id INTEGER PRIMARY KEY,
 item_name TEXT NOT NULL COLLATE NOCASE,
 source TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 value_pp INTEGER,
 sample_count INTEGER NOT NULL DEFAULT 0,
 last_seen TEXT,
 value_basis TEXT,
 is_manual INTEGER NOT NULL DEFAULT 0
);
DELETE FROM resolved_item_values;
INSERT INTO resolved_item_values(item_id,item_name,source,updated_at,value_pp,sample_count,last_seen,value_basis,is_manual)
WITH candidates AS (
 SELECT m.item_id,v.last_seen,v.is_manual,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN 1 WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN 2 WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN 3 WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN 4 WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN 5 WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN 6 WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN 7 WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN 8 WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN 9 WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN 10 ELSE 99 END value_priority,
  CASE WHEN v.item_name=m.item_name COLLATE NOCASE THEN 0 ELSE 1 END match_priority,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN v.average_30d_pp WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN v.average_60d_pp WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN v.average_90d_pp WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN v.average_6m_pp WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN v.average_all_pp WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN v.average_30d_pp WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN v.average_60d_pp WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN v.average_90d_pp WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN v.average_6m_pp WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN v.average_all_pp END value_pp,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN v.count_30d WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN v.count_60d WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN v.count_90d WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN v.count_6m WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN v.count_all WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN v.count_30d WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN v.count_60d WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN v.count_90d WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN v.count_6m WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN v.count_all ELSE 0 END sample_count
 FROM master_items m JOIN item_market_values v ON v.server='Green' COLLATE NOCASE AND v.source_item_id=m.item_id
 UNION ALL
 SELECT m.item_id,v.last_seen,v.is_manual,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN 1 WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN 2 WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN 3 WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN 4 WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN 5 WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN 6 WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN 7 WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN 8 WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN 9 WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN 10 ELSE 99 END,0,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN v.average_30d_pp WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN v.average_60d_pp WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN v.average_90d_pp WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN v.average_6m_pp WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN v.average_all_pp WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN v.average_30d_pp WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN v.average_60d_pp WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN v.average_90d_pp WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN v.average_6m_pp WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN v.average_all_pp END,
  CASE WHEN v.transaction_type=0 AND v.average_30d_pp>0 THEN v.count_30d WHEN v.transaction_type=0 AND v.average_60d_pp>0 THEN v.count_60d WHEN v.transaction_type=0 AND v.average_90d_pp>0 THEN v.count_90d WHEN v.transaction_type=0 AND v.average_6m_pp>0 THEN v.count_6m WHEN v.transaction_type=0 AND v.average_all_pp>0 THEN v.count_all WHEN v.transaction_type=1 AND v.average_30d_pp>0 THEN v.count_30d WHEN v.transaction_type=1 AND v.average_60d_pp>0 THEN v.count_60d WHEN v.transaction_type=1 AND v.average_90d_pp>0 THEN v.count_90d WHEN v.transaction_type=1 AND v.average_6m_pp>0 THEN v.count_6m WHEN v.transaction_type=1 AND v.average_all_pp>0 THEN v.count_all ELSE 0 END
 FROM master_items m JOIN item_market_values v ON v.server='Green' COLLATE NOCASE AND v.item_name=m.item_name COLLATE NOCASE AND v.source_item_id<>m.item_id
), ranked AS (
 SELECT *,ROW_NUMBER() OVER(PARTITION BY item_id ORDER BY is_manual DESC,value_priority,match_priority,sample_count DESC,last_seen DESC) value_rank FROM candidates WHERE value_priority<99
)
SELECT m.item_id,m.item_name,m.source,m.updated_at,r.value_pp,COALESCE(r.sample_count,0) sample_count,r.last_seen,
 CASE r.value_priority WHEN 1 THEN '30-day WTS' WHEN 2 THEN '60-day WTS' WHEN 3 THEN '90-day WTS' WHEN 4 THEN '6-month WTS' WHEN 5 THEN 'all-time WTS' WHEN 6 THEN '30-day WTB' WHEN 7 THEN '60-day WTB' WHEN 8 THEN '90-day WTB' WHEN 9 THEN '6-month WTB' WHEN 10 THEN 'all-time WTB' END value_basis,
 COALESCE(r.is_manual,0) is_manual
FROM master_items m LEFT JOIN ranked r ON r.item_id=m.item_id AND r.value_rank=1;

CREATE TABLE IF NOT EXISTS item_name_resolutions (
 item_name TEXT PRIMARY KEY COLLATE NOCASE,
 item_id INTEGER NOT NULL
);
DELETE FROM item_name_resolutions;
INSERT INTO item_name_resolutions(item_name,item_id)
SELECT item_name,item_id FROM master_items
UNION ALL
SELECT v.item_name,MIN(m.item_id)
FROM item_market_values v JOIN master_items m ON m.item_id=v.source_item_id
WHERE v.server='Green' COLLATE NOCASE
 AND NOT EXISTS(SELECT 1 FROM master_items exact WHERE exact.item_name=v.item_name COLLATE NOCASE)
GROUP BY v.item_name COLLATE NOCASE;
CREATE INDEX IF NOT EXISTS idx_item_name_resolutions_item ON item_name_resolutions(item_id);

INSERT OR IGNORE INTO schema_migrations(version,name) VALUES(10,'unified master item value resolver');
