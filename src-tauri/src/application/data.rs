use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, fs, path::Path};

use crate::{domain::inventory::parse_inventory, infrastructure::database::Database};

fn names(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split('\u{1f}')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn snapshot(database: &Database) -> Result<Value, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let mut settings = Map::new();
    {
        let mut statement = connection
            .prepare("SELECT key, value FROM app_settings ORDER BY key")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        for row in rows {
            let (key, value) = row.map_err(|error| error.to_string())?;
            settings.insert(key, Value::String(value));
        }
    }

    let members = query_values(
        &connection,
        "SELECT known_members.id, known_members.name, current_group.member_id IS NOT NULL
         FROM known_members LEFT JOIN current_group ON current_group.member_id = known_members.id
         ORDER BY current_group.member_id IS NULL, known_members.name COLLATE NOCASE",
        |row| {
            Ok(
                json!({"id": row.get::<_, i64>(0)?, "name": row.get::<_, String>(1)?, "active": row.get::<_, bool>(2)?}),
            )
        },
    )?;

    let loot = query_values(
        &connection,
        "SELECT d.id, d.happened_at, d.item_name, COALESCE(m.name,d.mob_name), d.looter_name,
                (SELECT average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE
                 AND v.transaction_type=0 AND v.item_name=d.item_name COLLATE NOCASE AND v.average_30d_pp>0
                 ORDER BY v.count_30d DESC, v.last_seen DESC LIMIT 1),
                EXISTS(SELECT 1 FROM split_loot_items s WHERE s.loot_drop_id=d.id),
                (SELECT GROUP_CONCAT(member_name, char(31)) FROM loot_drop_members lm WHERE lm.loot_drop_id=d.id)
         FROM loot_drops d LEFT JOIN mobs m ON m.id=d.mob_id
         ORDER BY d.happened_at DESC, d.id DESC LIMIT 1000",
        |row| Ok(json!({
            "id": row.get::<_, i64>(0)?, "happenedAt": row.get::<_, String>(1)?,
            "itemName": row.get::<_, String>(2)?, "mobName": row.get::<_, Option<String>>(3)?,
            "looterName": row.get::<_, Option<String>>(4)?, "valuePp": row.get::<_, Option<i64>>(5)?,
            "splitListed": row.get::<_, bool>(6)?, "attendees": names(row.get::<_, Option<String>>(7)?)
        })),
    )?;

    let splits = query_values(
        &connection,
        "SELECT 'manual:'||s.id, s.item_name, s.added_at, m.name, s.looter_name, s.payout_value_pp,
                (SELECT average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0
                 AND v.item_name=s.item_name COLLATE NOCASE AND v.average_30d_pp>0 ORDER BY v.count_30d DESC LIMIT 1),
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM manual_split_list_members x WHERE x.split_list_item_id=s.id)
         FROM manual_split_list_items s LEFT JOIN mobs m ON m.id=s.mob_id
         UNION ALL
         SELECT 'loot:'||s.loot_drop_id, s.item_name, s.added_at, s.mob_name, s.looter_name, s.payout_value_pp,
                (SELECT average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0
                 AND v.item_name=s.item_name COLLATE NOCASE AND v.average_30d_pp>0 ORDER BY v.count_30d DESC LIMIT 1),
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM split_loot_members x WHERE x.split_loot_item_id=s.id)
         FROM split_loot_items s ORDER BY 3 DESC",
        |row| Ok(json!({
            "key": row.get::<_, String>(0)?, "itemName": row.get::<_, String>(1)?, "addedAt": row.get::<_, String>(2)?,
            "mobName": row.get::<_, Option<String>>(3)?, "looterName": row.get::<_, Option<String>>(4)?,
            "payoutValuePp": row.get::<_, Option<i64>>(5)?, "marketValuePp": row.get::<_, Option<i64>>(6)?,
            "attendees": names(row.get::<_, Option<String>>(7)?)
        })),
    )?;

    let tracked = query_values(
        &connection,
        "SELECT t.id,t.source_loot_id,t.happened_at,t.item_name,t.mob_name,t.looter_name,t.value_pp,t.tracked_at,
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM tracked_loot_members x WHERE x.tracked_loot_item_id=t.id)
         FROM tracked_loot_items t ORDER BY t.happened_at DESC,t.id DESC LIMIT 2000",
        |row| Ok(json!({
            "id":row.get::<_,i64>(0)?,"sourceLootId":row.get::<_,Option<i64>>(1)?,"happenedAt":row.get::<_,String>(2)?,
            "itemName":row.get::<_,String>(3)?,"mobName":row.get::<_,Option<String>>(4)?,"looterName":row.get::<_,Option<String>>(5)?,
            "valuePp":row.get::<_,Option<i64>>(6)?,"trackedAt":row.get::<_,String>(7)?,"attendees":names(row.get::<_,Option<String>>(8)?)
        })),
    )?;

    let history = query_values(
        &connection,
        "SELECT h.id,h.item_name,h.mob_name,h.looter_name,h.value_pp,h.disposition,h.note,h.completed_at,
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM completed_split_members x WHERE x.completed_split_item_id=h.id)
         FROM completed_split_items h ORDER BY h.completed_at DESC,h.id DESC LIMIT 2000",
        |row| Ok(json!({
            "id":row.get::<_,i64>(0)?,"itemName":row.get::<_,String>(1)?,"mobName":row.get::<_,Option<String>>(2)?,
            "looterName":row.get::<_,Option<String>>(3)?,"valuePp":row.get::<_,i64>(4)?,"disposition":row.get::<_,String>(5)?,
            "note":row.get::<_,String>(6)?,"completedAt":row.get::<_,String>(7)?,"attendees":names(row.get::<_,Option<String>>(8)?)
        })),
    )?;

    let items = query_values(
        &connection,
        "SELECT m.item_id,m.item_name,
                COALESCE((SELECT v.average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.item_name=m.item_name COLLATE NOCASE ORDER BY v.count_30d DESC LIMIT 1),0),
                COALESCE((SELECT v.count_30d FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.item_name=m.item_name COLLATE NOCASE ORDER BY v.count_30d DESC LIMIT 1),0),
                COALESCE((SELECT v.last_seen FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.item_name=m.item_name COLLATE NOCASE ORDER BY v.count_30d DESC LIMIT 1),m.updated_at),
                m.source='manual',m.source
         FROM master_items m ORDER BY m.item_name COLLATE NOCASE LIMIT 10000",
        |row| {
            Ok(
                json!({"id":row.get::<_,i64>(0)?,"name":row.get::<_,String>(1)?,"valuePp":row.get::<_,i64>(2)?,
            "count30d":row.get::<_,i64>(3)?,"lastSeen":row.get::<_,String>(4)?,"manual":row.get::<_,bool>(5)?,"source":row.get::<_,String>(6)?}),
            )
        },
    )?;

    let inventory = query_values(
        &connection,
        "SELECT c.name,c.imported_at,i.id,i.location,i.item_name,i.item_id,i.item_count,i.slots,
                (SELECT average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0
                 AND v.item_name=i.item_name COLLATE NOCASE AND v.average_30d_pp>0 ORDER BY v.count_30d DESC LIMIT 1)
         FROM inventory_characters c JOIN inventory_items i ON i.character_id=c.id
         ORDER BY c.name COLLATE NOCASE,i.sort_order",
        |row| Ok(json!({"character":row.get::<_,String>(0)?,"importedAt":row.get::<_,String>(1)?,"id":row.get::<_,i64>(2)?,
            "location":row.get::<_,String>(3)?,"itemName":row.get::<_,String>(4)?,"itemId":row.get::<_,Option<i64>>(5)?,
            "count":row.get::<_,i64>(6)?,"slots":row.get::<_,Option<i64>>(7)?,"valuePp":row.get::<_,Option<i64>>(8)?})),
    )?;

    let spells = query_values(
        &connection,
        "SELECT c.name,c.imported_at,s.slot_number,s.spell_name FROM spellbook_characters c
         JOIN spellbook_spells s ON s.character_id=c.id ORDER BY c.name COLLATE NOCASE,s.sort_order",
        |row| Ok(json!({"character":row.get::<_,String>(0)?,"importedAt":row.get::<_,String>(1)?,
            "slot":row.get::<_,Option<i64>>(2)?,"spellName":row.get::<_,String>(3)?})),
    )?;

    let wts = query_values(
        &connection,
        "SELECT g.id,g.character_name,g.name,g.created_at,g.updated_at,
                (SELECT GROUP_CONCAT(item_name,char(31)) FROM wts_group_items i WHERE i.wts_group_id=g.id ORDER BY i.sort_order),
                (SELECT GROUP_CONCAT(COALESCE(item_id,0),char(31)) FROM wts_group_items i WHERE i.wts_group_id=g.id ORDER BY i.sort_order)
         FROM wts_groups g ORDER BY g.character_name COLLATE NOCASE,g.updated_at DESC",
        |row| Ok(json!({"id":row.get::<_,i64>(0)?,"character":row.get::<_,String>(1)?,"name":row.get::<_,String>(2)?,
            "createdAt":row.get::<_,String>(3)?,"updatedAt":row.get::<_,String>(4)?,"items":names(row.get::<_,Option<String>>(5)?),
            "itemIds":names(row.get::<_,Option<String>>(6)?).iter().map(|v|v.parse::<i64>().ok()).collect::<Vec<_>>()})),
    )?;

    let aliases = query_values(
        &connection,
        "SELECT alias_name,canonical_name FROM character_aliases ORDER BY canonical_name COLLATE NOCASE,alias_name COLLATE NOCASE",
        |row| Ok(json!({"alias":row.get::<_,String>(0)?,"canonical":row.get::<_,String>(1)?})),
    )?;
    let mobs = query_values(
        &connection,
        "SELECT name FROM mobs ORDER BY name COLLATE NOCASE",
        |row| Ok(json!(row.get::<_, String>(0)?)),
    )?;
    let logs = query_values(
        &connection,
        "SELECT id,happened_at,level,area,message FROM application_logs ORDER BY id DESC LIMIT 1000",
        |row| Ok(json!({"id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,"level":row.get::<_,String>(2)?,
            "area":row.get::<_,String>(3)?,"message":row.get::<_,String>(4)?})),
    )?;
    let imports = query_values(
        &connection,
        "SELECT id,happened_at,file_name,status,review_url,detail FROM import_uploads ORDER BY id DESC LIMIT 500",
        |row| Ok(json!({"id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
            "fileName":row.get::<_,String>(2)?,"status":row.get::<_,String>(3)?,
            "reviewUrl":row.get::<_,Option<String>>(4)?,"detail":row.get::<_,Option<String>>(5)?})),
    )?;
    let merchant_item_values = query_values(
        &connection,
        "SELECT i.merchant_message_id,i.id,i.item_name,i.item_id,i.asking_price_pp,
                COALESCE(
                    (SELECT v.average_30d_pp FROM item_market_values v
                     WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.average_30d_pp>0
                       AND v.source_item_id=i.item_id
                     ORDER BY v.count_30d DESC,v.last_seen DESC LIMIT 1),
                    (SELECT v.average_30d_pp FROM item_market_values v
                     WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.average_30d_pp>0
                       AND v.item_name=i.item_name COLLATE NOCASE
                     ORDER BY v.count_30d DESC,v.last_seen DESC LIMIT 1)),
                COALESCE(
                    (SELECT v.count_30d FROM item_market_values v
                     WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.average_30d_pp>0
                       AND v.source_item_id=i.item_id
                     ORDER BY v.count_30d DESC,v.last_seen DESC LIMIT 1),
                    (SELECT v.count_30d FROM item_market_values v
                     WHERE v.server='Green' COLLATE NOCASE AND v.transaction_type=0 AND v.average_30d_pp>0
                       AND v.item_name=i.item_name COLLATE NOCASE
                     ORDER BY v.count_30d DESC,v.last_seen DESC LIMIT 1),0)
         FROM merchant_message_items i
         WHERE i.merchant_message_id IN (SELECT id FROM merchant_messages ORDER BY id DESC LIMIT 2000)
         ORDER BY i.merchant_message_id DESC,i.sort_order",
        |row| Ok(json!({"messageId":row.get::<_,i64>(0)?,"id":row.get::<_,i64>(1)?,
            "itemName":row.get::<_,String>(2)?,"itemId":row.get::<_,Option<i64>>(3)?,
            "askingPricePp":row.get::<_,Option<i64>>(4)?,"marketValuePp":row.get::<_,Option<i64>>(5)?,
            "marketCount30d":row.get::<_,i64>(6)?})),
    )?;
    let mut merchant_items: HashMap<i64, Vec<Value>> = HashMap::new();
    for item in merchant_item_values {
        if let Some(message_id) = item.get("messageId").and_then(Value::as_i64) {
            merchant_items.entry(message_id).or_default().push(item);
        }
    }
    let merchant = query_values(
        &connection,
        "SELECT id,happened_at,kind,speaker_name,message FROM merchant_messages ORDER BY happened_at DESC,id DESC LIMIT 2000",
        |row| {
            let id = row.get::<_, i64>(0)?;
            Ok(json!({"id":id,"happenedAt":row.get::<_,String>(1)?,"kind":row.get::<_,String>(2)?,
                "speakerName":row.get::<_,String>(3)?,"message":row.get::<_,String>(4)?,
                "items":merchant_items.get(&id).cloned().unwrap_or_default()}))
        },
    )?;
    let compound = normalize_compound(
        settings
            .get("compound_workspace")
            .and_then(Value::as_str)
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_else(|| json!({"projects":[],"templates":[],"activeId":null})),
    );

    Ok(
        json!({"settings":settings,"members":members,"loot":loot,"splits":splits,"tracked":tracked,"history":history,
        "items":items,"inventory":inventory,"spells":spells,"wts":wts,"aliases":aliases,"mobs":mobs,
        "logs":logs,"imports":imports,"merchant":merchant,"compound":compound}),
    )
}

fn normalize_compound(mut workspace: Value) -> Value {
    if !workspace.is_object() {
        return json!({"projects":[],"templates":[],"activeId":null});
    }
    let root = workspace.as_object_mut().expect("object checked above");
    root.entry("projects").or_insert_with(|| json!([]));
    root.entry("templates").or_insert_with(|| json!([]));
    root.entry("activeId").or_insert(Value::Null);

    if let Some(projects) = root.get_mut("projects").and_then(Value::as_array_mut) {
        for project in projects {
            let Some(project) = project.as_object_mut() else {
                continue;
            };
            project.entry("templates").or_insert_with(|| json!([]));
            project.entry("note").or_insert_with(|| json!(""));
            if let Some(components) = project.get_mut("components").and_then(Value::as_array_mut) {
                for component in components {
                    if component.is_string() {
                        let name = component.as_str().unwrap_or_default().to_owned();
                        *component =
                            json!({"itemName":name,"required":1,"received":0,"contributors":[]});
                    }
                    let Some(component) = component.as_object_mut() else {
                        continue;
                    };
                    if !component.contains_key("itemName") {
                        let name = component.get("name").cloned().unwrap_or_else(|| json!(""));
                        component.insert("itemName".into(), name);
                    }
                    if !component.contains_key("contributors") {
                        let owners = component
                            .get("owners")
                            .cloned()
                            .unwrap_or_else(|| json!([]));
                        component.insert("contributors".into(), owners);
                    }
                    component.entry("required").or_insert_with(|| json!(1));
                    component.entry("received").or_insert_with(|| json!(0));
                }
            } else {
                project.insert("components".into(), json!([]));
            }
        }
    } else {
        root.insert("projects".into(), json!([]));
    }

    if let Some(templates) = root.get_mut("templates").and_then(Value::as_array_mut) {
        for template in templates {
            let Some(template) = template.as_object_mut() else {
                continue;
            };
            if let Some(components) = template.get_mut("components").and_then(Value::as_array_mut) {
                for component in components {
                    if component.is_string() {
                        let name = component.as_str().unwrap_or_default().to_owned();
                        *component = json!({"itemName":name,"required":1,"valuePp":0});
                    }
                    let Some(component) = component.as_object_mut() else {
                        continue;
                    };
                    if !component.contains_key("itemName") {
                        let name = component.get("name").cloned().unwrap_or_else(|| json!(""));
                        component.insert("itemName".into(), name);
                    }
                    if !component.contains_key("valuePp") {
                        let value = component.get("value").cloned().unwrap_or_else(|| json!(0));
                        component.insert("valuePp".into(), value);
                    }
                    component.entry("required").or_insert_with(|| json!(1));
                }
            }
        }
    }
    workspace
}

fn query_values<F>(
    connection: &rusqlite::Connection,
    sql: &str,
    mut mapper: F,
) -> Result<Vec<Value>, String>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<Value>,
{
    let mut statement = connection.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| mapper(row))
        .map_err(|error| error.to_string())?;
    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

pub fn mutate(database: &Database, action: &str, payload: &Value) -> Result<Value, String> {
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    match action {
        "setting.save" => {
            let key = required(payload, "key")?;
            let value = required(payload, "value")?;
            connection.execute("INSERT INTO app_settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value]).map_err(err)?;
        }
        "member.add" => {
            let name = required(payload, "name")?;
            connection
                .execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [&name],
                )
                .map_err(err)?;
            if payload
                .get("active")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                connection.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE", [&name]).map_err(err)?;
            }
        }
        "member.active" => {
            let id = integer(payload, "id")?;
            if payload
                .get("active")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                connection
                    .execute(
                        "INSERT OR IGNORE INTO current_group(member_id) VALUES(?)",
                        [id],
                    )
                    .map_err(err)?;
            } else {
                connection
                    .execute("DELETE FROM current_group WHERE member_id=?", [id])
                    .map_err(err)?;
            }
        }
        "member.delete" => {
            connection
                .execute(
                    "DELETE FROM known_members WHERE id=?",
                    [integer(payload, "id")?],
                )
                .map_err(err)?;
        }
        "loot.delete" => {
            for id in integers(payload, "ids") {
                connection
                    .execute("DELETE FROM loot_drops WHERE id=?", [id])
                    .map_err(err)?;
            }
        }
        "loot.save" => save_loot(&mut connection, payload)?,
        "loot.split" => set_loot_split(
            &mut connection,
            integer(payload, "id")?,
            payload
                .get("listed")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        )?,
        "loot.track" => track_loot(&mut connection, integer(payload, "id")?)?,
        "tracked.delete" => {
            for id in integers(payload, "ids") {
                connection
                    .execute("DELETE FROM tracked_loot_items WHERE id=?", [id])
                    .map_err(err)?;
            }
        }
        "split.add" => add_split(&mut connection, payload)?,
        "split.save" => save_split(&mut connection, payload)?,
        "split.delete" => delete_split(&mut connection, required(payload, "key")?)?,
        "split.complete" => complete_split(&mut connection, payload)?,
        "history.save" => {
            connection
                .execute(
                    "UPDATE completed_split_items SET disposition=?,value_pp=?,note=? WHERE id=?",
                    params![
                        required(payload, "disposition")?,
                        integer(payload, "valuePp")?,
                        payload.get("note").and_then(Value::as_str).unwrap_or(""),
                        integer(payload, "id")?
                    ],
                )
                .map_err(err)?;
        }
        "history.delete" => {
            for id in integers(payload, "ids") {
                connection
                    .execute("DELETE FROM completed_split_items WHERE id=?", [id])
                    .map_err(err)?;
            }
        }
        "item.save" => save_item(&mut connection, payload)?,
        "item.delete" => {
            connection.execute("DELETE FROM item_market_values WHERE server='Green' AND transaction_type=0 AND source_item_id=? AND is_manual=1",[integer(payload,"id")?]).map_err(err)?;
            connection
                .execute(
                    "DELETE FROM master_items WHERE item_id=? AND source='manual'",
                    [integer(payload, "id")?],
                )
                .map_err(err)?;
        }
        "aliases.save" => save_aliases(&mut connection, payload)?,
        "compound.save" => {
            let raw = serde_json::to_string(payload.get("workspace").unwrap_or(&json!({})))
                .map_err(|e| e.to_string())?;
            connection.execute("INSERT INTO app_settings(key,value) VALUES('compound_workspace',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[raw]).map_err(err)?;
        }
        "wts.save" => save_wts(&mut connection, payload)?,
        "wts.delete" => {
            connection
                .execute(
                    "DELETE FROM wts_groups WHERE id=?",
                    [integer(payload, "id")?],
                )
                .map_err(err)?;
        }
        "merchant.clear" => {
            connection
                .execute("DELETE FROM merchant_messages", [])
                .map_err(err)?;
            connection
                .execute(
                    "DELETE FROM app_settings WHERE key='merchant_last_capture_at'",
                    [],
                )
                .map_err(err)?;
        }
        "merchant.delete" => {
            let kind = optional(payload, "kind");
            let speaker = optional(payload, "speakerName");
            if kind
                .as_deref()
                .is_some_and(|value| !matches!(value, "wts" | "wtb" | "tell"))
            {
                return Err("kind must be wts, wtb, or tell".into());
            }
            match (kind, speaker) {
                (Some(kind), Some(speaker)) => {
                    connection
                        .execute(
                            "DELETE FROM merchant_messages WHERE kind=? AND speaker_name=? COLLATE NOCASE",
                            params![kind, speaker],
                        )
                        .map_err(err)?;
                }
                (Some(kind), None) => {
                    connection
                        .execute("DELETE FROM merchant_messages WHERE kind=?", [kind])
                        .map_err(err)?;
                }
                (None, Some(speaker)) => {
                    connection
                        .execute(
                            "DELETE FROM merchant_messages WHERE speaker_name=? COLLATE NOCASE",
                            [speaker],
                        )
                        .map_err(err)?;
                }
                (None, None) => return Err("kind or speakerName is required".into()),
            }
        }
        "inventory.import" => {
            import_export(&mut connection, Path::new(&required(payload, "path")?))?
        }
        "inventory.importFiles" => {
            let files = payload
                .get("files")
                .and_then(Value::as_array)
                .ok_or("files are required")?;
            if files.is_empty() {
                return Err("Choose at least one export file".into());
            }
            for file in files {
                let name = required(file, "name")?;
                let text = file
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or("file text is required")?;
                import_export_text(
                    &mut connection,
                    &name,
                    text,
                    &format!("Dropped file: {name}"),
                )?;
            }
        }
        _ => return Err(format!("Unknown action: {action}")),
    }
    connection
        .execute(
            "INSERT INTO application_logs(level,area,message) VALUES('info','action',?)",
            [action],
        )
        .map_err(err)?;
    Ok(json!({"ok":true}))
}

fn save_loot(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let id = integer(payload, "id")?;
    let item = required(payload, "itemName")?;
    let mob = optional(payload, "mobName");
    let looter = optional(payload, "looterName");
    connection
        .execute(
            "UPDATE loot_drops SET item_name=?,mob_name=?,mob_id=NULL,looter_name=? WHERE id=?",
            params![item, mob, looter, id],
        )
        .map_err(err)?;
    connection
        .execute("DELETE FROM loot_drop_members WHERE loot_drop_id=?", [id])
        .map_err(err)?;
    for name in strings(payload, "attendees") {
        remember(connection, &name)?;
        connection
            .execute(
                "INSERT OR IGNORE INTO loot_drop_members(loot_drop_id,member_name) VALUES(?,?)",
                params![id, name],
            )
            .map_err(err)?;
    }
    Ok(())
}

fn set_loot_split(
    connection: &mut rusqlite::Connection,
    id: i64,
    listed: bool,
) -> Result<(), String> {
    if listed {
        connection.execute("INSERT OR IGNORE INTO split_loot_items(loot_drop_id,item_name,mob_name,looter_name) SELECT id,item_name,mob_name,looter_name FROM loot_drops WHERE id=?",[id]).map_err(err)?;
        connection.execute("INSERT OR IGNORE INTO split_loot_members(split_loot_item_id,member_name) SELECT s.id,m.member_name FROM split_loot_items s JOIN loot_drop_members m ON m.loot_drop_id=s.loot_drop_id WHERE s.loot_drop_id=?",[id]).map_err(err)?;
    } else {
        connection
            .execute("DELETE FROM split_loot_items WHERE loot_drop_id=?", [id])
            .map_err(err)?;
    }
    Ok(())
}

fn track_loot(connection: &mut rusqlite::Connection, loot_id: i64) -> Result<(), String> {
    connection.execute(
        "INSERT OR IGNORE INTO tracked_loot_items(source_loot_id,happened_at,item_name,mob_name,looter_name,value_pp)
         SELECT d.id,d.happened_at,d.item_name,COALESCE(m.name,d.mob_name),d.looter_name,
                (SELECT average_30d_pp FROM item_market_values v WHERE v.server='Green' COLLATE NOCASE
                 AND v.transaction_type=0 AND v.item_name=d.item_name COLLATE NOCASE AND v.average_30d_pp>0
                 ORDER BY v.count_30d DESC,v.last_seen DESC LIMIT 1)
         FROM loot_drops d LEFT JOIN mobs m ON m.id=d.mob_id WHERE d.id=?",
        [loot_id],
    ).map_err(err)?;
    let tracked_id: Option<i64> = connection
        .query_row(
            "SELECT id FROM tracked_loot_items WHERE source_loot_id=?",
            [loot_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    let Some(tracked_id) = tracked_id else {
        return Err("Loot drop was not found".into());
    };
    connection
        .execute(
            "INSERT OR IGNORE INTO tracked_loot_members(tracked_loot_item_id,member_name)
         SELECT ?,member_name FROM loot_drop_members WHERE loot_drop_id=?",
            params![tracked_id, loot_id],
        )
        .map_err(err)?;
    Ok(())
}

fn add_split(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let item = required(payload, "itemName")?;
    let mob = optional(payload, "mobName");
    let looter = optional(payload, "looterName");
    let mob_id = if let Some(ref name) = mob {
        connection
            .execute(
                "INSERT INTO mobs(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                [name],
            )
            .map_err(err)?;
        connection
            .query_row(
                "SELECT id FROM mobs WHERE name=? COLLATE NOCASE",
                [name],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(err)?
    } else {
        None
    };
    connection.execute("INSERT INTO manual_split_list_items(item_name,mob_id,looter_name,payout_value_pp) VALUES(?,?,?,?)",params![item,mob_id,looter,payload.get("payoutValuePp").and_then(Value::as_i64)]).map_err(err)?;
    let id = connection.last_insert_rowid();
    for name in strings(payload, "attendees") {
        remember(connection, &name)?;
        connection.execute("INSERT OR IGNORE INTO manual_split_list_members(split_list_item_id,member_name) VALUES(?,?)",params![id,name]).map_err(err)?;
    }
    Ok(())
}

fn save_split(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let key = required(payload, "key")?;
    let item = required(payload, "itemName")?;
    let mob = optional(payload, "mobName");
    let looter = optional(payload, "looterName");
    let payout = payload.get("payoutValuePp").and_then(Value::as_i64);
    if let Some(id) = key
        .strip_prefix("manual:")
        .and_then(|v| v.parse::<i64>().ok())
    {
        connection.execute("UPDATE manual_split_list_items SET item_name=?,mob_id=NULL,looter_name=?,payout_value_pp=? WHERE id=?",params![item,looter,payout,id]).map_err(err)?;
        if let Some(m) = mob {
            connection
                .execute(
                    "INSERT INTO mobs(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [&m],
                )
                .map_err(err)?;
            connection.execute("UPDATE manual_split_list_items SET mob_id=(SELECT id FROM mobs WHERE name=? COLLATE NOCASE) WHERE id=?",params![m,id]).map_err(err)?;
        }
        connection
            .execute(
                "DELETE FROM manual_split_list_members WHERE split_list_item_id=?",
                [id],
            )
            .map_err(err)?;
        for n in strings(payload, "attendees") {
            remember(connection, &n)?;
            connection
                .execute(
                    "INSERT OR IGNORE INTO manual_split_list_members VALUES(?,?)",
                    params![id, n],
                )
                .map_err(err)?;
        }
    } else if let Some(id) = key
        .strip_prefix("loot:")
        .and_then(|v| v.parse::<i64>().ok())
    {
        connection.execute("UPDATE split_loot_items SET item_name=?,mob_name=?,looter_name=?,payout_value_pp=? WHERE loot_drop_id=?",params![item,mob,looter,payout,id]).map_err(err)?;
        let split_id: i64 = connection
            .query_row(
                "SELECT id FROM split_loot_items WHERE loot_drop_id=?",
                [id],
                |r| r.get(0),
            )
            .map_err(err)?;
        connection
            .execute(
                "DELETE FROM split_loot_members WHERE split_loot_item_id=?",
                [split_id],
            )
            .map_err(err)?;
        for n in strings(payload, "attendees") {
            remember(connection, &n)?;
            connection
                .execute(
                    "INSERT OR IGNORE INTO split_loot_members VALUES(?,?)",
                    params![split_id, n],
                )
                .map_err(err)?;
        }
    }
    Ok(())
}

fn delete_split(connection: &mut rusqlite::Connection, key: String) -> Result<(), String> {
    if let Some(id) = key.strip_prefix("manual:") {
        connection
            .execute("DELETE FROM manual_split_list_items WHERE id=?", [id])
            .map_err(err)?;
    } else if let Some(id) = key.strip_prefix("loot:") {
        connection
            .execute("DELETE FROM split_loot_items WHERE loot_drop_id=?", [id])
            .map_err(err)?;
    }
    Ok(())
}

fn complete_split(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let key = required(payload, "key")?;
    let (table, id_col, members_table, fk, id) = if let Some(v) = key.strip_prefix("manual:") {
        (
            "manual_split_list_items",
            "id",
            "manual_split_list_members",
            "split_list_item_id",
            v,
        )
    } else {
        (
            "split_loot_items",
            "loot_drop_id",
            "split_loot_members",
            "split_loot_item_id",
            key.strip_prefix("loot:").ok_or("Invalid split key")?,
        )
    };
    let sql = format!(
        "SELECT item_name,looter_name,{} FROM {} WHERE {}=?",
        if table.starts_with("manual") {
            "(SELECT name FROM mobs WHERE id=mob_id)"
        } else {
            "mob_name"
        },
        table,
        id_col
    );
    let row: (String, Option<String>, Option<String>) = connection
        .query_row(&sql, [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(err)?;
    connection.execute("INSERT INTO completed_split_items(item_name,mob_name,looter_name,value_pp,disposition,note) VALUES(?,?,?,?,?,?)",params![row.0,row.2,row.1,integer(payload,"valuePp")?,required(payload,"disposition")?,payload.get("note").and_then(Value::as_str).unwrap_or("")]).map_err(err)?;
    let completed = connection.last_insert_rowid();
    let member_sql = if table.starts_with("manual") {
        format!("SELECT member_name FROM {members_table} WHERE {fk}=?")
    } else {
        "SELECT x.member_name FROM split_loot_members x JOIN split_loot_items s ON s.id=x.split_loot_item_id WHERE s.loot_drop_id=?".to_owned()
    };
    let member_names = {
        let mut st = connection.prepare(&member_sql).map_err(err)?;
        let values = st
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(err)?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        values
    };
    for n in member_names {
        connection
            .execute(
                "INSERT INTO completed_split_members VALUES(?,?)",
                params![completed, n],
            )
            .map_err(err)?;
    }
    delete_split(connection, key)?;
    Ok(())
}

fn save_item(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let id = integer(payload, "id")?;
    let name = required(payload, "name")?;
    let now = Local::now().to_rfc3339();
    let original = payload.get("originalId").and_then(Value::as_i64);
    connection.execute("DELETE FROM master_items WHERE (item_id=? OR item_name=? COLLATE NOCASE) AND item_id<>?",params![id,name,id]).map_err(err)?;
    if let Some(old) = original {
        connection.execute("UPDATE master_items SET item_id=?,item_name=?,source='manual',updated_at=? WHERE item_id=?",params![id,name,now,old]).map_err(err)?;
    }
    connection.execute("INSERT INTO master_items(item_id,item_name,source,updated_at) VALUES(?,?,'manual',?) ON CONFLICT(item_id) DO UPDATE SET item_name=excluded.item_name,source='manual',updated_at=excluded.updated_at",params![id,name,now]).map_err(err)?;
    if let Some(old) = original {
        connection.execute("UPDATE item_market_values SET source_item_id=?,item_name=?,average_30d_pp=?,is_manual=1,fetched_at=? WHERE server='Green' AND transaction_type=0 AND source_item_id=?",params![id,name,integer(payload,"valuePp")?,now,old]).map_err(err)?;
    } else {
        connection.execute("INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,fetched_at,is_manual) VALUES('Green',?,0,?,'Manual entry',?,?,1) ON CONFLICT(server,source_item_id,transaction_type) DO UPDATE SET item_name=excluded.item_name,average_30d_pp=excluded.average_30d_pp,fetched_at=excluded.fetched_at,is_manual=1",params![id,name,integer(payload,"valuePp")?,now]).map_err(err)?;
    }
    connection
        .execute(
            "UPDATE wts_group_items SET item_id=? WHERE item_name=? COLLATE NOCASE",
            params![id, name],
        )
        .map_err(err)?;
    Ok(())
}

fn save_aliases(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    connection
        .execute("DELETE FROM character_aliases", [])
        .map_err(err)?;
    for group in payload
        .get("groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        let canonical = required(&group, "canonical")?;
        for alias in strings(&group, "aliases") {
            connection.execute("INSERT OR REPLACE INTO character_aliases(alias_name,canonical_name) VALUES(?,?)",params![alias,canonical]).map_err(err)?;
        }
    }
    Ok(())
}

fn save_wts(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let character = required(payload, "character")?;
    let name = required(payload, "name")?;
    let now = Local::now().to_rfc3339();
    let id = payload.get("id").and_then(Value::as_i64);
    let group_id = if let Some(id) = id {
        connection
            .execute(
                "UPDATE wts_groups SET name=?,updated_at=? WHERE id=?",
                params![name, now, id],
            )
            .map_err(err)?;
        connection
            .execute("DELETE FROM wts_group_items WHERE wts_group_id=?", [id])
            .map_err(err)?;
        id
    } else {
        connection
            .execute(
                "INSERT INTO wts_groups(character_name,name,created_at,updated_at) VALUES(?,?,?,?)",
                params![character, name, now, now],
            )
            .map_err(err)?;
        connection.last_insert_rowid()
    };
    for (order, item) in strings(payload, "items").into_iter().enumerate() {
        let item_id = connection
            .query_row(
                "SELECT item_id FROM master_items WHERE item_name=? COLLATE NOCASE LIMIT 1",
                [&item],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map_err(err)?;
        connection.execute("INSERT INTO wts_group_items(wts_group_id,item_name,item_id,sort_order) VALUES(?,?,?,?)",params![group_id,item,item_id,order as i64]).map_err(err)?;
    }
    Ok(())
}

fn import_export(connection: &mut rusqlite::Connection, path: &Path) -> Result<(), String> {
    let filename = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("Invalid export path")?;
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    import_export_text(connection, filename, &text, &path.display().to_string())
}

fn import_export_text(
    connection: &mut rusqlite::Connection,
    filename: &str,
    text: &str,
    source: &str,
) -> Result<(), String> {
    let character = filename.split('-').next().unwrap_or("").trim();
    if character.is_empty() {
        return Err("Export filename must begin with a character name".into());
    }
    let now = Local::now().to_rfc3339();
    let detail = if filename.to_ascii_lowercase().ends_with("-inventory.txt") {
        let items = parse_inventory(&text);
        connection.execute("INSERT INTO inventory_characters(name,source_file,imported_at) VALUES(?,?,?) ON CONFLICT(name) DO UPDATE SET source_file=excluded.source_file,imported_at=excluded.imported_at",params![character,source,now]).map_err(err)?;
        let id: i64 = connection
            .query_row(
                "SELECT id FROM inventory_characters WHERE name=? COLLATE NOCASE",
                [character],
                |r| r.get(0),
            )
            .map_err(err)?;
        connection
            .execute("DELETE FROM inventory_items WHERE character_id=?", [id])
            .map_err(err)?;
        for (order, item) in items.iter().enumerate() {
            if let Some(item_id) = item.item_id.filter(|value| *value > 0) {
                upsert_inventory_master_item(connection, item_id, &item.item_name, &now)?;
            }
            connection.execute("INSERT INTO inventory_items(character_id,location,item_name,item_id,item_count,slots,sort_order) VALUES(?,?,?,?,?,?,?)",params![id,item.location,item.item_name,item.item_id,item.count,item.slots,order as i64]).map_err(err)?;
        }
        format!("Imported {} inventory rows for {character}", items.len())
    } else if filename.to_ascii_lowercase().ends_with("-spellbook.txt") {
        connection.execute("INSERT INTO spellbook_characters(name,source_file,imported_at) VALUES(?,?,?) ON CONFLICT(name) DO UPDATE SET source_file=excluded.source_file,imported_at=excluded.imported_at",params![character,source,now]).map_err(err)?;
        let id: i64 = connection
            .query_row(
                "SELECT id FROM spellbook_characters WHERE name=? COLLATE NOCASE",
                [character],
                |r| r.get(0),
            )
            .map_err(err)?;
        connection
            .execute("DELETE FROM spellbook_spells WHERE character_id=?", [id])
            .map_err(err)?;
        let mut count = 0;
        for (order, line) in text.lines().enumerate() {
            let mut cells = line.split('\t');
            let first = cells.next().unwrap_or("").trim();
            let second = cells.next().unwrap_or(first).trim();
            if second.is_empty() {
                continue;
            }
            connection.execute("INSERT INTO spellbook_spells(character_id,slot_number,spell_name,sort_order) VALUES(?,?,?,?)",params![id,first.parse::<i64>().ok(),second.trim_start_matches("Spell: "),order as i64]).map_err(err)?;
            count += 1;
        }
        format!("Imported {count} spellbook rows for {character}")
    } else {
        return Err("Choose an *-Inventory.txt or *-Spellbook.txt file".into());
    };
    let status = if source.starts_with("Dropped file:") {
        "manual import"
    } else {
        "auto import"
    };
    connection
        .execute(
            "INSERT INTO import_uploads(file_name,status,detail) VALUES(?,?,?)",
            params![filename, status, detail],
        )
        .map_err(err)?;
    Ok(())
}

fn upsert_inventory_master_item(
    connection: &rusqlite::Connection,
    item_id: i64,
    item_name: &str,
    now: &str,
) -> Result<(), String> {
    if item_name.trim().is_empty() {
        return Ok(());
    }
    connection.execute("DELETE FROM master_items WHERE (item_id=? OR item_name=? COLLATE NOCASE) AND NOT (item_id=? AND item_name=? COLLATE NOCASE)",params![item_id,item_name,item_id,item_name]).map_err(err)?;
    connection.execute("INSERT INTO master_items(item_id,item_name,source,updated_at) VALUES(?,?,'inventory',?) ON CONFLICT(item_id) DO UPDATE SET item_name=excluded.item_name,source='inventory',updated_at=excluded.updated_at",params![item_id,item_name,now]).map_err(err)?;
    connection
        .execute(
            "UPDATE wts_group_items SET item_id=? WHERE item_name=? COLLATE NOCASE",
            params![item_id, item_name],
        )
        .map_err(err)?;
    connection
        .execute(
            "UPDATE recipe_components SET item_id=? WHERE item_name=? COLLATE NOCASE",
            params![item_id, item_name],
        )
        .map_err(err)?;
    connection
        .execute(
            "UPDATE recipe_templates SET output_item_id=? WHERE name=? COLLATE NOCASE",
            params![item_id, item_name],
        )
        .map_err(err)?;
    Ok(())
}

fn remember(connection: &rusqlite::Connection, name: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
            [name],
        )
        .map_err(err)?;
    Ok(())
}
fn required(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} is required"))
}
fn optional(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
}
fn integer(value: &Value, key: &str) -> Result<i64, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("{key} is required"))
}
fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{mutate, normalize_compound, snapshot};
    use crate::infrastructure::database::Database;
    use rusqlite::params;
    use serde_json::json;

    #[test]
    fn normalizes_v2_compound_components_without_losing_metadata() {
        let value = normalize_compound(json!({
            "projects":[{"id":"p1","name":"Cloak of Confusion","itemId":21597,"components":[{
                "name":"A Blue Throne","itemId":18359,"required":1,"received":0,
                "value":22915,"owners":["Youngman"]
            }]}],
            "templates":[{"id":"t1","name":"Saved cloak","components":[{
                "name":"A Blue Throne","itemId":18359,"required":2,"value":22915
            }]}],"activeId":"p1"
        }));
        let component = &value["projects"][0]["components"][0];
        assert_eq!(component["itemName"], "A Blue Throne");
        assert_eq!(component["contributors"], json!(["Youngman"]));
        assert_eq!(component["itemId"], 18359);
        assert_eq!(component["value"], 22915);
        assert_eq!(value["activeId"], "p1");
        let template_component = &value["templates"][0]["components"][0];
        assert_eq!(template_component["itemName"], "A Blue Throne");
        assert_eq!(template_component["itemId"], 18359);
        assert_eq!(template_component["required"], 2);
        assert_eq!(template_component["valuePp"], 22915);
    }

    #[test]
    fn inventory_ids_become_authoritative_for_master_items_and_wts() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO wts_groups(character_name,name,created_at,updated_at) VALUES('Khards','Tunnel',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)",[]).unwrap();
        let group_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO wts_group_items(wts_group_id,item_name,item_id,sort_order) VALUES(?,?,999,0)",params![group_id,"A Blue Crown"]).unwrap();
        drop(connection);

        mutate(&database,"inventory.importFiles",&json!({"files":[{"name":"Khards-Inventory.txt","text":"General1\tA Blue Crown\t12345\t1\n"}]})).unwrap();

        let connection = database.connect().unwrap();
        let master_id: i64 = connection
            .query_row(
                "SELECT item_id FROM master_items WHERE item_name='A Blue Crown' COLLATE NOCASE",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let wts_id: i64 = connection
            .query_row(
                "SELECT item_id FROM wts_group_items WHERE wts_group_id=?",
                [group_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((master_id, wts_id), (12345, 12345));
    }

    #[test]
    fn merchant_snapshot_compares_asking_and_pigparse_prices() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO master_items(item_id,item_name,source) VALUES(42,'This Item','market')",[]).unwrap();
        connection.execute("INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,fetched_at) VALUES('Green',42,0,'This Item','Today',1750,CURRENT_TIMESTAMP)",[]).unwrap();
        connection.execute("INSERT INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(CURRENT_TIMESTAMP,'wts','Trader','WTS This Item 1300','raw','eqlog_Test_P1999Green.txt',1)",[]).unwrap();
        let message_id = connection.last_insert_rowid();
        connection.execute("INSERT INTO merchant_message_items(merchant_message_id,item_name,item_id,asking_price_pp,sort_order) VALUES(?,'This Item',42,1300,0)",[message_id]).unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        assert_eq!(value["merchant"][0]["items"][0]["askingPricePp"], 1300);
        assert_eq!(value["merchant"][0]["items"][0]["marketValuePp"], 1750);
        assert_eq!(value["merchant"][0]["items"][0]["marketCount30d"], 0);
    }

    #[test]
    fn merchant_deletes_can_be_scoped_by_panel_and_person() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        for (offset, kind, speaker) in [
            (1, "wts", "Trader"),
            (2, "wts", "Other"),
            (3, "wtb", "Trader"),
            (4, "tell", "Trader"),
        ] {
            connection.execute("INSERT INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(CURRENT_TIMESTAMP,?,?,?,?,'eqlog_Test_P1999Green.txt',?)",params![kind,speaker,format!("{kind} message"),format!("raw {offset}"),offset]).unwrap();
        }
        drop(connection);

        mutate(
            &database,
            "merchant.delete",
            &json!({"kind":"wts","speakerName":"Trader"}),
        )
        .unwrap();
        let connection = database.connect().unwrap();
        let remaining_wts: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM merchant_messages WHERE kind='wts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let trader_other_panels: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM merchant_messages WHERE speaker_name='Trader'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((remaining_wts, trader_other_panels), (1, 2));
        drop(connection);

        mutate(&database, "merchant.delete", &json!({"kind":"wts"})).unwrap();
        let connection = database.connect().unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM merchant_messages WHERE kind='wts'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
    }

    #[test]
    fn tracked_loot_survives_deleting_the_source_loot() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO loot_drops(happened_at,item_name,mob_name,looter_name,raw_line,source_file,source_offset)
             VALUES('2026-08-19 12:00:00','A Blue Crown','a mortiferous golem','Youngman','raw','eqlog_Youngman_P1999Green.txt',42)",
            [],
        ).unwrap();
        let loot_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO loot_drop_members(loot_drop_id,member_name) VALUES(?,'Youngman'),(?,'Posed')",
            params![loot_id, loot_id],
        ).unwrap();
        drop(connection);

        mutate(&database, "loot.track", &json!({"id":loot_id})).unwrap();
        mutate(&database, "loot.track", &json!({"id":loot_id})).unwrap();
        mutate(&database, "loot.delete", &json!({"ids":[loot_id]})).unwrap();

        let value = snapshot(&database).unwrap();
        assert_eq!(value["loot"].as_array().unwrap().len(), 0);
        assert_eq!(value["tracked"].as_array().unwrap().len(), 1);
        let tracked = &value["tracked"][0];
        assert_eq!(tracked["itemName"], "A Blue Crown");
        assert_eq!(tracked["mobName"], "a mortiferous golem");
        assert_eq!(tracked["looterName"], "Youngman");
        assert_eq!(tracked["attendees"], json!(["Posed", "Youngman"]));
    }
}
fn integers(value: &Value, key: &str) -> Vec<i64> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .collect()
}
fn err(error: rusqlite::Error) -> String {
    error.to_string()
}
