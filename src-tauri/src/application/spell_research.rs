use rusqlite::Connection;
use serde_json::{json, Value};

pub fn snapshot(connection: &Connection) -> Result<Vec<Value>, String> {
    let mut statement=connection.prepare(
        "SELECT r.id,r.class_name,r.spell_level,r.spell_name,r.spell_item_id,
                r.trivial,r.research_only,r.availability,r.source_url,
                c.id,c.item_id,c.item_name,c.icon_id,c.quantity,c.component_kind,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),
                spell_value.value_pp
         FROM spell_research_recipes r
         JOIN spell_research_components c ON c.recipe_id=r.id
         LEFT JOIN item_name_resolutions ni ON ni.item_name=c.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=COALESCE(c.item_id,ni.item_id)
         LEFT JOIN resolved_item_values spell_value ON spell_value.item_id=r.spell_item_id
         ORDER BY r.class_name,r.spell_level,r.spell_name,c.component_kind,c.item_name COLLATE NOCASE"
    ).map_err(|error|error.to_string())?;
    let values = statement
        .query_map([], |row| {
            Ok(json!({
                "recipeId":row.get::<_,i64>(0)?,"className":row.get::<_,String>(1)?,
                "level":row.get::<_,i64>(2)?,"spellName":row.get::<_,String>(3)?,
                "spellItemId":row.get::<_,Option<i64>>(4)?,"trivial":row.get::<_,String>(5)?,
                "researchOnly":row.get::<_,bool>(6)?,"availability":row.get::<_,String>(7)?,
                "sourceUrl":row.get::<_,String>(8)?,"componentId":row.get::<_,i64>(9)?,
                "itemId":row.get::<_,Option<i64>>(10)?,"itemName":row.get::<_,String>(11)?,
                "iconId":row.get::<_,Option<i64>>(12)?,"quantity":row.get::<_,i64>(13)?,
                "componentKind":row.get::<_,String>(14)?,"valuePp":row.get::<_,Option<i64>>(15)?,
                "valueBasis":row.get::<_,Option<String>>(16)?,"valueSamples":row.get::<_,i64>(17)?,
                "spellValuePp":row.get::<_,Option<i64>>(18)?,
            }))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(values)
}
