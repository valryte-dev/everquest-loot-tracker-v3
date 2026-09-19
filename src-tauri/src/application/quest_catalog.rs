use rusqlite::Connection;
use serde_json::{json, Value};

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
