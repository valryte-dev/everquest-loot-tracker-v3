use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Map, Value};
use std::{fs, path::Path};

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
        "SELECT source_item_id,item_name,average_30d_pp,count_30d,last_seen,is_manual
         FROM item_market_values WHERE server='Green' COLLATE NOCASE AND transaction_type=0
         ORDER BY item_name COLLATE NOCASE LIMIT 10000",
        |row| {
            Ok(
                json!({"id":row.get::<_,i64>(0)?,"name":row.get::<_,String>(1)?,"valuePp":row.get::<_,i64>(2)?,
            "count30d":row.get::<_,i64>(3)?,"lastSeen":row.get::<_,String>(4)?,"manual":row.get::<_,bool>(5)?}),
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
    let compound = normalize_compound(
        settings
            .get("compound_workspace")
            .and_then(Value::as_str)
            .and_then(|raw| serde_json::from_str(raw).ok())
            .unwrap_or_else(|| json!({"projects":[],"templates":[],"activeId":null})),
    );

    Ok(
        json!({"settings":settings,"members":members,"loot":loot,"splits":splits,"history":history,
        "items":items,"inventory":inventory,"spells":spells,"wts":wts,"aliases":aliases,"mobs":mobs,
        "logs":logs,"imports":imports,"compound":compound}),
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
                    if let Some(object) = component.as_object() {
                        *component = object
                            .get("itemName")
                            .or_else(|| object.get("name"))
                            .cloned()
                            .unwrap_or_else(|| json!(""));
                    }
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
    let original = payload.get("originalId").and_then(Value::as_i64);
    if let Some(old) = original {
        connection.execute("UPDATE item_market_values SET source_item_id=?,item_name=?,average_30d_pp=?,is_manual=1,fetched_at=? WHERE server='Green' AND transaction_type=0 AND source_item_id=?",params![id,required(payload,"name")?,integer(payload,"valuePp")?,Local::now().to_rfc3339(),old]).map_err(err)?;
    } else {
        connection.execute("INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,fetched_at,is_manual) VALUES('Green',?,0,?,'Manual entry',?,?,1)",params![id,required(payload,"name")?,integer(payload,"valuePp")?,Local::now().to_rfc3339()]).map_err(err)?;
    }
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
        let item_id=connection.query_row("SELECT source_item_id FROM item_market_values WHERE server='Green' AND transaction_type=0 AND item_name=? COLLATE NOCASE ORDER BY is_manual DESC LIMIT 1",[&item],|r|r.get::<_,i64>(0)).optional().map_err(err)?;
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
    connection
        .execute(
            "INSERT INTO import_uploads(file_name,status,detail) VALUES(?,'imported',?)",
            params![filename, detail],
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
    use super::normalize_compound;
    use serde_json::json;

    #[test]
    fn normalizes_v2_compound_components_without_losing_metadata() {
        let value = normalize_compound(json!({
            "projects":[{"id":"p1","name":"Cloak of Confusion","itemId":21597,"components":[{
                "name":"A Blue Throne","itemId":18359,"required":1,"received":0,
                "value":22915,"owners":["Youngman"]
            }]}],
            "templates":[],"activeId":"p1"
        }));
        let component = &value["projects"][0]["components"][0];
        assert_eq!(component["itemName"], "A Blue Throne");
        assert_eq!(component["contributors"], json!(["Youngman"]));
        assert_eq!(component["itemId"], 18359);
        assert_eq!(component["value"], 22915);
        assert_eq!(value["activeId"], "p1");
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
