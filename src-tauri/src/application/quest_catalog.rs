use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestUsage {
    pub entry_id: i64,
    pub category: String,
    pub class_name: String,
    pub quest_name: String,
    pub reward_name: String,
    pub slot: String,
    pub faction: String,
    pub source_url: String,
}

#[derive(Default)]
pub struct QuestUsageIndex {
    by_item_id: HashMap<i64, Vec<QuestUsage>>,
    by_item_name: HashMap<String, Vec<QuestUsage>>,
}

impl QuestUsageIndex {
    pub fn lookup(&self, item_id: Option<i64>, item_name: &str) -> Vec<QuestUsage> {
        if let Some(usages) = item_id.and_then(|id| self.by_item_id.get(&id)) {
            return usages.clone();
        }
        self.by_item_name
            .get(&item_name.trim().to_lowercase())
            .cloned()
            .unwrap_or_default()
    }
}

pub fn usage_index(connection: &Connection) -> Result<QuestUsageIndex, String> {
    let mut index = QuestUsageIndex::default();
    let mut statement = connection
        .prepare(
            "SELECT c.item_id,c.item_name,e.id,e.category,e.class_name,e.quest_name,
                    e.reward_name,e.slot,e.faction,e.source_url
             FROM quest_catalog_components c
             JOIN quest_catalog_entries e ON e.id=c.quest_entry_id
             ORDER BY e.category,e.class_name,e.quest_name,e.id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, String>(1)?,
                QuestUsage {
                    entry_id: row.get(2)?,
                    category: row.get(3)?,
                    class_name: row.get(4)?,
                    quest_name: row.get(5)?,
                    reward_name: row.get(6)?,
                    slot: row.get(7)?,
                    faction: row.get(8)?,
                    source_url: row.get(9)?,
                },
            ))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (item_id, item_name, usage) = row.map_err(|error| error.to_string())?;
        if let Some(item_id) = item_id {
            index
                .by_item_id
                .entry(item_id)
                .or_default()
                .push(usage.clone());
        }
        index
            .by_item_name
            .entry(item_name.trim().to_lowercase())
            .or_default()
            .push(usage);
    }
    Ok(index)
}

pub fn snapshot(connection: &Connection) -> Result<Vec<Value>, String> {
    let mut statement = connection
        .prepare(
            "SELECT e.id,e.category,e.class_name,e.quest_name,e.reward_item_id,e.reward_name,
                    e.reward_icon_id,e.slot,e.faction,e.note,e.source_url,
                    c.id,c.item_id,c.item_name,c.icon_id,c.quantity,
                    rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0)
             FROM quest_catalog_entries e
             JOIN quest_catalog_components c ON c.quest_entry_id=e.id
             LEFT JOIN item_name_resolutions ni ON ni.item_name=c.item_name COLLATE NOCASE
             LEFT JOIN resolved_item_values rv ON rv.item_id=COALESCE(c.item_id,ni.item_id)
             ORDER BY e.category,e.class_name,e.quest_name,e.slot,c.item_name COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let values = statement
        .query_map([], |row| {
            Ok(json!({
                "entryId":row.get::<_,i64>(0)?,
                "category":row.get::<_,String>(1)?,
                "className":row.get::<_,String>(2)?,
                "questName":row.get::<_,String>(3)?,
                "rewardItemId":row.get::<_,Option<i64>>(4)?,
                "rewardName":row.get::<_,String>(5)?,
                "rewardIconId":row.get::<_,Option<i64>>(6)?,
                "slot":row.get::<_,String>(7)?,
                "faction":row.get::<_,String>(8)?,
                "note":row.get::<_,String>(9)?,
                "sourceUrl":row.get::<_,String>(10)?,
                "componentId":row.get::<_,i64>(11)?,
                "itemId":row.get::<_,Option<i64>>(12)?,
                "itemName":row.get::<_,String>(13)?,
                "iconId":row.get::<_,Option<i64>>(14)?,
                "quantity":row.get::<_,i64>(15)?,
                "valuePp":row.get::<_,Option<i64>>(16)?,
                "valueBasis":row.get::<_,Option<String>>(17)?,
                "valueSamples":row.get::<_,i64>(18)?,
            }))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_usage_prefers_canonical_item_id_and_falls_back_to_name() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE quest_catalog_entries(
                    id INTEGER PRIMARY KEY, category TEXT, class_name TEXT, quest_name TEXT,
                    reward_name TEXT, slot TEXT, faction TEXT, source_url TEXT
                 );
                 CREATE TABLE quest_catalog_components(
                    item_id INTEGER, item_name TEXT, quest_entry_id INTEGER
                 );
                 INSERT INTO quest_catalog_entries VALUES
                    (1,'epic','Druid','Druid Epic','Nature Walker''s Scimitar','','','https://example/epic'),
                    (2,'plane_of_sky','Monk','Monk Test','Sky Reward','','','https://example/sky');
                 INSERT INTO quest_catalog_components VALUES
                    (42,'Jade Reaver',1),
                    (NULL,'Jade Reaver',2);",
            )
            .unwrap();

        let index = usage_index(&connection).unwrap();
        assert_eq!(index.lookup(Some(42), "Jade Reaver").len(), 1);
        assert_eq!(index.lookup(None, "  jade reaver ").len(), 2);
        assert_eq!(index.lookup(None, "Unrelated Item").len(), 0);
    }
}
