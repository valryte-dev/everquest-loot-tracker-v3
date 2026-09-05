use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Map, Value};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::{
    domain::{
        inventory::parse_inventory,
        log_events::{parse_log_event, LogEvent},
    },
    infrastructure::database::Database,
};

fn names(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split('\u{1f}')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn payouts(value: Option<String>) -> Vec<Value> {
    value
        .unwrap_or_default()
        .split('\u{1f}')
        .filter_map(|entry| {
            let (name, paid_at) = entry.split_once('\u{1e}')?;
            Some(json!({"name":name,"paidAt":paid_at}))
        })
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
        "SELECT d.id,d.happened_at,d.item_name,COALESCE(m.name,d.mob_name),d.looter_name,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),rv.item_id,
                EXISTS(SELECT 1 FROM split_loot_items s WHERE s.loot_drop_id=d.id),
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM loot_drop_members lm WHERE lm.loot_drop_id=d.id)
         FROM loot_drops d
         LEFT JOIN mobs m ON m.id=d.mob_id
         LEFT JOIN item_name_resolutions ni ON ni.item_name=d.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY d.happened_at DESC,d.id DESC LIMIT 1000",
        |row| Ok(json!({
            "id": row.get::<_, i64>(0)?, "happenedAt": row.get::<_, String>(1)?,
            "itemName":row.get::<_,String>(2)?,"mobName":row.get::<_,Option<String>>(3)?,
            "looterName":row.get::<_,Option<String>>(4)?,"valuePp":row.get::<_,Option<i64>>(5)?,
            "valueBasis":row.get::<_,Option<String>>(6)?,"valueSamples":row.get::<_,i64>(7)?,
            "itemId":row.get::<_,Option<i64>>(8)?,"splitListed":row.get::<_,bool>(9)?,
            "attendees":names(row.get::<_,Option<String>>(10)?)
        })),
    )?;

    let splits = query_values(
        &connection,
        "SELECT 'manual:'||s.id,s.item_name,s.added_at,m.name,s.looter_name,s.payout_value_pp,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM manual_split_list_members x WHERE x.split_list_item_id=s.id)
         FROM manual_split_list_items s LEFT JOIN mobs m ON m.id=s.mob_id
         LEFT JOIN item_name_resolutions ni ON ni.item_name=s.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         UNION ALL
         SELECT 'loot:'||s.loot_drop_id,s.item_name,s.added_at,s.mob_name,s.looter_name,s.payout_value_pp,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM split_loot_members x WHERE x.split_loot_item_id=s.id)
         FROM split_loot_items s
         LEFT JOIN item_name_resolutions ni ON ni.item_name=s.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY 3 DESC",
        |row| Ok(json!({
            "key": row.get::<_, String>(0)?, "itemName": row.get::<_, String>(1)?, "addedAt": row.get::<_, String>(2)?,
            "mobName": row.get::<_, Option<String>>(3)?, "looterName": row.get::<_, Option<String>>(4)?,
            "payoutValuePp":row.get::<_,Option<i64>>(5)?,"marketValuePp":row.get::<_,Option<i64>>(6)?,
            "marketValueBasis":row.get::<_,Option<String>>(7)?,"marketValueSamples":row.get::<_,i64>(8)?,
            "attendees":names(row.get::<_,Option<String>>(9)?)
        })),
    )?;

    let tracked = query_values(
        &connection,
        "SELECT t.id,t.source_loot_id,t.happened_at,t.item_name,t.mob_name,t.looter_name,
                COALESCE(rv.value_pp,t.value_pp),t.tracked_at,
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM tracked_loot_members x WHERE x.tracked_loot_item_id=t.id),
                COALESCE(rv.value_basis,CASE WHEN t.value_pp>0 THEN 'saved estimate' END),COALESCE(rv.sample_count,0)
         FROM tracked_loot_items t LEFT JOIN item_name_resolutions ni ON ni.item_name=t.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY t.happened_at DESC,t.id DESC LIMIT 2000",
        |row| Ok(json!({
            "id":row.get::<_,i64>(0)?,"sourceLootId":row.get::<_,Option<i64>>(1)?,"happenedAt":row.get::<_,String>(2)?,
            "itemName":row.get::<_,String>(3)?,"mobName":row.get::<_,Option<String>>(4)?,"looterName":row.get::<_,Option<String>>(5)?,
            "valuePp":row.get::<_,Option<i64>>(6)?,"trackedAt":row.get::<_,String>(7)?,
            "attendees":names(row.get::<_,Option<String>>(8)?),"valueBasis":row.get::<_,Option<String>>(9)?,
            "valueSamples":row.get::<_,i64>(10)?
        })),
    )?;

    let history = query_values(
        &connection,
        "SELECT h.id,h.item_name,h.mob_name,h.looter_name,h.value_pp,h.disposition,h.note,h.completed_at,h.payout_status,h.paid_at,
                (SELECT GROUP_CONCAT(member_name,char(31)) FROM completed_split_members x WHERE x.completed_split_item_id=h.id),
                (SELECT GROUP_CONCAT(member_name||char(30)||paid_at,char(31)) FROM completed_split_payouts p WHERE p.completed_split_item_id=h.id)
         FROM completed_split_items h ORDER BY h.completed_at DESC,h.id DESC LIMIT 2000",
        |row| Ok(json!({
            "id":row.get::<_,i64>(0)?,"itemName":row.get::<_,String>(1)?,"mobName":row.get::<_,Option<String>>(2)?,
            "looterName":row.get::<_,Option<String>>(3)?,"valuePp":row.get::<_,i64>(4)?,"disposition":row.get::<_,String>(5)?,
            "note":row.get::<_,String>(6)?,"completedAt":row.get::<_,String>(7)?,"payoutStatus":row.get::<_,String>(8)?,
            "paidAt":row.get::<_,Option<String>>(9)?,"attendees":names(row.get::<_,Option<String>>(10)?),
            "payouts":payouts(row.get::<_,Option<String>>(11)?)
        })),
    )?;

    let items = query_values(
        &connection,
        "SELECT m.item_id,m.item_name,COALESCE(rv.value_pp,0),COALESCE(rv.sample_count,0),
                COALESCE(rv.last_seen,m.updated_at),COALESCE(rv.is_manual,0),m.source,rv.value_basis
         FROM master_items m LEFT JOIN resolved_item_values rv ON rv.item_id=m.item_id
         ORDER BY m.item_name COLLATE NOCASE LIMIT 10000",
        |row| {
            Ok(
                json!({"id":row.get::<_,i64>(0)?,"name":row.get::<_,String>(1)?,"valuePp":row.get::<_,i64>(2)?,
            "count30d":row.get::<_,i64>(3)?,"lastSeen":row.get::<_,String>(4)?,"manual":row.get::<_,bool>(5)?,
            "source":row.get::<_,String>(6)?,"valueBasis":row.get::<_,Option<String>>(7)?}),
            )
        },
    )?;

    let inventory = query_values(
        &connection,
        "SELECT c.name,c.imported_at,i.id,i.location,i.item_name,COALESCE(i.item_id,ni.item_id),i.item_count,i.slots,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0)
         FROM inventory_characters c JOIN inventory_items i ON i.character_id=c.id
         LEFT JOIN item_name_resolutions ni ON ni.item_name=i.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=COALESCE(i.item_id,ni.item_id)
         ORDER BY c.name COLLATE NOCASE,i.sort_order",
        |row| Ok(json!({"character":row.get::<_,String>(0)?,"importedAt":row.get::<_,String>(1)?,"id":row.get::<_,i64>(2)?,
            "location":row.get::<_,String>(3)?,"itemName":row.get::<_,String>(4)?,"itemId":row.get::<_,Option<i64>>(5)?,
            "count":row.get::<_,i64>(6)?,"slots":row.get::<_,Option<i64>>(7)?,"valuePp":row.get::<_,Option<i64>>(8)?,
            "valueBasis":row.get::<_,Option<String>>(9)?,"valueSamples":row.get::<_,i64>(10)?})),
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
        "SELECT i.merchant_message_id,i.id,i.item_name,COALESCE(i.item_id,ni.item_id),i.asking_price_pp,
                rv.value_pp,COALESCE(rv.sample_count,0),rv.value_basis
         FROM merchant_message_items i
         LEFT JOIN item_name_resolutions ni ON ni.item_name=i.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=COALESCE(i.item_id,ni.item_id)
         WHERE i.merchant_message_id IN (SELECT id FROM merchant_messages ORDER BY id DESC LIMIT 2000)
         ORDER BY i.merchant_message_id DESC,i.sort_order",
        |row| Ok(json!({"messageId":row.get::<_,i64>(0)?,"id":row.get::<_,i64>(1)?,
            "itemName":row.get::<_,String>(2)?,"itemId":row.get::<_,Option<i64>>(3)?,
            "askingPricePp":row.get::<_,Option<i64>>(4)?,"marketValuePp":row.get::<_,Option<i64>>(5)?,
            "marketCount30d":row.get::<_,i64>(6)?,"marketValueBasis":row.get::<_,Option<String>>(7)?})),
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
    let linked_loot = query_values(
        &connection,
        "SELECT l.id,l.happened_at,l.channel,l.speaker_name,l.item_name,ni.item_id,
                rv.value_pp,COALESCE(rv.sample_count,0),rv.value_basis
         FROM linked_loot_items l
         LEFT JOIN item_name_resolutions ni ON ni.item_name=l.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY l.happened_at DESC,l.id DESC LIMIT 5000",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "channel":row.get::<_,String>(2)?,"speakerName":row.get::<_,String>(3)?,
                "itemName":row.get::<_,String>(4)?,"itemId":row.get::<_,Option<i64>>(5)?,
                "valuePp":row.get::<_,Option<i64>>(6)?,"count30d":row.get::<_,i64>(7)?,
                "valueBasis":row.get::<_,Option<String>>(8)?
            }))
        },
    )?;
    let death_reports = query_values(
        &connection,
        "SELECT d.id,d.happened_at,d.character_name,d.killer_name,d.source_file,
                COUNT(e.sequence_number)
         FROM death_reports d
         LEFT JOIN death_report_entries e ON e.death_report_id=d.id
         GROUP BY d.id
         ORDER BY d.happened_at DESC,d.id DESC",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,"killerName":row.get::<_,String>(3)?,
                "sourceFile":row.get::<_,String>(4)?,"contextCount":row.get::<_,i64>(5)?
            }))
        },
    )?;

    let damage_encounters = query_values(
        &connection,
        "SELECT e.id,e.character_name,e.mob_name,e.started_at,e.ended_at,e.last_damage_at,
                e.total_damage,e.melee_damage,e.spell_damage,e.hit_count,e.max_hit,e.outcome,e.source_file,
                (SELECT GROUP_CONCAT(name,char(31)) FROM (
                    SELECT w.primary_weapon_name AS name
                    FROM damage_events de JOIN character_weapon_loadouts w ON w.id=de.weapon_loadout_id
                    WHERE de.encounter_id=e.id AND COALESCE(w.primary_weapon_name,'')<>''
                    UNION
                    SELECT w.secondary_weapon_name AS name
                    FROM damage_events de JOIN character_weapon_loadouts w ON w.id=de.weapon_loadout_id
                    WHERE de.encounter_id=e.id AND COALESCE(w.secondary_weapon_name,'')<>''
                )),
                (SELECT json_group_array(json_object(
                    'name',attacker_name,'totalDamage',total_damage,'hitCount',hit_count,
                    'firstDamageAt',first_damage_at,'lastDamageAt',last_damage_at
                )) FROM (
                    SELECT COALESCE(NULLIF(de.attacker_name,''),'Unknown') AS attacker_name,
                           SUM(de.damage) AS total_damage,COUNT(*) AS hit_count,
                           MIN(de.happened_at) AS first_damage_at,MAX(de.happened_at) AS last_damage_at
                    FROM damage_events de WHERE de.encounter_id=e.id
                    GROUP BY COALESCE(NULLIF(de.attacker_name,''),'Unknown') COLLATE NOCASE
                    ORDER BY total_damage DESC,attacker_name COLLATE NOCASE
                ))
         FROM damage_encounters e ORDER BY e.started_at DESC,e.id DESC LIMIT 5000",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"character":row.get::<_,String>(1)?,
                "mobName":row.get::<_,String>(2)?,"startedAt":row.get::<_,String>(3)?,
                "endedAt":row.get::<_,Option<String>>(4)?,"lastDamageAt":row.get::<_,String>(5)?,
                "totalDamage":row.get::<_,i64>(6)?,"meleeDamage":row.get::<_,i64>(7)?,
                "spellDamage":row.get::<_,i64>(8)?,"hitCount":row.get::<_,i64>(9)?,
                "maxHit":row.get::<_,i64>(10)?,"outcome":row.get::<_,String>(11)?,
                "sourceFile":row.get::<_,String>(12)?,
                "weapons":names(row.get::<_,Option<String>>(13)?),
                "players":row.get::<_,Option<String>>(14)?
                    .and_then(|value|serde_json::from_str::<Value>(&value).ok())
                    .unwrap_or_else(||json!([]))
            }))
        },
    )?;

    let current_weapon_loadout = if let Some(character) = settings
        .get("active_character")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        connection
            .query_row(
                "SELECT captured_at,primary_weapon_name,primary_item_id,secondary_weapon_name,secondary_item_id
                 FROM character_weapon_loadouts
                 WHERE character_name=? COLLATE NOCASE
                 ORDER BY captured_at DESC,id DESC LIMIT 1",
                [character],
                |row| {
                    Ok(json!({
                        "character":character,
                        "capturedAt":row.get::<_,String>(0)?,
                        "primary":row.get::<_,Option<String>>(1)?,
                        "primaryItemId":row.get::<_,Option<i64>>(2)?,
                        "secondary":row.get::<_,Option<String>>(3)?,
                        "secondaryItemId":row.get::<_,Option<i64>>(4)?
                    }))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?
    } else {
        None
    };

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
        "logs":logs,"imports":imports,"merchant":merchant,"linkedLoot":linked_loot,
        "deathReports":death_reports,"damageEncounters":damage_encounters,
        "currentWeaponLoadout":current_weapon_loadout,"compound":compound}),
    )
}

pub fn activity_history_snapshot(database: &Database) -> Result<Value, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let loot = query_values(
        &connection,
        "SELECT h.id,h.happened_at,h.character_name,h.item_name,h.looter_name,h.source_file,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),rv.item_id
         FROM activity_loot_history h
         LEFT JOIN item_name_resolutions ni ON ni.item_name=h.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY h.happened_at DESC,h.id DESC",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,"itemName":row.get::<_,String>(3)?,
                "looterName":row.get::<_,String>(4)?,"sourceFile":row.get::<_,String>(5)?,
                "valuePp":row.get::<_,Option<i64>>(6)?,"valueBasis":row.get::<_,Option<String>>(7)?,
                "valueSamples":row.get::<_,i64>(8)?,"itemId":row.get::<_,Option<i64>>(9)?
            }))
        },
    )?;
    let mobs = query_values(
        &connection,
        "SELECT id,happened_at,character_name,mob_name,killer_name,source_file
         FROM activity_mob_history ORDER BY happened_at DESC,id DESC",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,"mobName":row.get::<_,String>(3)?,
                "killerName":row.get::<_,Option<String>>(4)?,"sourceFile":row.get::<_,String>(5)?
            }))
        },
    )?;
    let offers = query_values(
        &connection,
        "SELECT h.id,h.happened_at,h.character_name,h.offerer_name,h.item_name,h.source_file,
                rv.value_pp,rv.value_basis,COALESCE(rv.sample_count,0),rv.item_id
         FROM activity_offer_history h
         LEFT JOIN item_name_resolutions ni ON ni.item_name=h.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         ORDER BY h.happened_at DESC,h.id DESC",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,"offererName":row.get::<_,String>(3)?,
                "itemName":row.get::<_,String>(4)?,"sourceFile":row.get::<_,String>(5)?,
                "valuePp":row.get::<_,Option<i64>>(6)?,"valueBasis":row.get::<_,Option<String>>(7)?,
                "valueSamples":row.get::<_,i64>(8)?,"itemId":row.get::<_,Option<i64>>(9)?
            }))
        },
    )?;
    let levels = query_values(
        &connection,
        "SELECT id,happened_at,character_name,level,direction,source_file
         FROM activity_level_history ORDER BY happened_at DESC,id DESC",
        |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,"level":row.get::<_,i64>(3)?,
                "direction":row.get::<_,String>(4)?,"sourceFile":row.get::<_,String>(5)?
            }))
        },
    )?;
    Ok(json!({"loot":loot,"mobs":mobs,"offers":offers,"levels":levels}))
}

pub fn death_report_details(database: &Database, id: i64) -> Result<Value, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let report = connection
        .query_row(
            "SELECT id,happened_at,character_name,killer_name,raw_line,source_file,source_offset
             FROM death_reports WHERE id=?",
            [id],
            |row| {
                Ok(json!({
                    "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                    "character":row.get::<_,String>(2)?,"killerName":row.get::<_,String>(3)?,
                    "rawLine":row.get::<_,String>(4)?,"sourceFile":row.get::<_,String>(5)?,
                    "sourceOffset":row.get::<_,i64>(6)?
                }))
            },
        )
        .optional()
        .map_err(err)?
        .ok_or("Death report was not found")?;
    let entries = {
        let mut statement = connection
            .prepare(
                "SELECT sequence_number,raw_line FROM death_report_entries
                 WHERE death_report_id=? ORDER BY sequence_number",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([id], |row| {
                Ok(json!({
                    "sequenceNumber":row.get::<_,i64>(0)?,
                    "rawLine":row.get::<_,String>(1)?
                }))
            })
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)?
    };
    let mut value = report;
    value
        .as_object_mut()
        .expect("death report query returns an object")
        .insert("entries".into(), Value::Array(entries));
    Ok(value)
}

pub fn damage_encounter_details(database: &Database, id: i64) -> Result<Value, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let encounter = connection
        .query_row(
            "SELECT e.id,e.character_name,e.mob_name,e.started_at,e.ended_at,e.last_damage_at,
                    e.total_damage,e.melee_damage,e.spell_damage,e.hit_count,e.max_hit,e.outcome,e.source_file,
                    (SELECT GROUP_CONCAT(name,char(31)) FROM (
                        SELECT w.primary_weapon_name AS name
                        FROM damage_events de JOIN character_weapon_loadouts w ON w.id=de.weapon_loadout_id
                        WHERE de.encounter_id=e.id AND COALESCE(w.primary_weapon_name,'')<>''
                        UNION
                        SELECT w.secondary_weapon_name AS name
                        FROM damage_events de JOIN character_weapon_loadouts w ON w.id=de.weapon_loadout_id
                        WHERE de.encounter_id=e.id AND COALESCE(w.secondary_weapon_name,'')<>''
                    )),
                    (SELECT json_group_array(json_object(
                                        'name',attacker_name,'totalDamage',total_damage,'hitCount',hit_count,
                                        'firstDamageAt',first_damage_at,'lastDamageAt',last_damage_at
                                    )) FROM (
                                        SELECT COALESCE(NULLIF(de.attacker_name,''),'Unknown') AS attacker_name,
                                               SUM(de.damage) AS total_damage,COUNT(*) AS hit_count,
                                               MIN(de.happened_at) AS first_damage_at,MAX(de.happened_at) AS last_damage_at
                                        FROM damage_events de WHERE de.encounter_id=e.id
                                        GROUP BY COALESCE(NULLIF(de.attacker_name,''),'Unknown') COLLATE NOCASE
                                        ORDER BY total_damage DESC,attacker_name COLLATE NOCASE
                                    ))
             FROM damage_encounters e WHERE e.id=?",
            [id],
            |row| {
                Ok(json!({
                    "id":row.get::<_,i64>(0)?,"character":row.get::<_,String>(1)?,
                    "mobName":row.get::<_,String>(2)?,"startedAt":row.get::<_,String>(3)?,
                    "endedAt":row.get::<_,Option<String>>(4)?,"lastDamageAt":row.get::<_,String>(5)?,
                    "totalDamage":row.get::<_,i64>(6)?,"meleeDamage":row.get::<_,i64>(7)?,
                    "spellDamage":row.get::<_,i64>(8)?,"hitCount":row.get::<_,i64>(9)?,
                    "maxHit":row.get::<_,i64>(10)?,"outcome":row.get::<_,String>(11)?,
                    "sourceFile":row.get::<_,String>(12)?,
                    "weapons":names(row.get::<_,Option<String>>(13)?),
                    "players":row.get::<_,Option<String>>(14)?
                        .and_then(|value|serde_json::from_str::<Value>(&value).ok())
                        .unwrap_or_else(||json!([]))
                }))
            },
        )
        .optional()
        .map_err(err)?
        .ok_or("Damage encounter was not found")?;
    let events = {
        let mut statement = connection
            .prepare(
                "SELECT e.id,e.happened_at,e.damage_type,e.attack_kind,e.damage,
                        w.primary_weapon_name,w.primary_item_id,
                        w.secondary_weapon_name,w.secondary_item_id,
                        COALESCE(e.attacker_name,encounter.character_name,'Unknown')
                 FROM damage_events e
                 LEFT JOIN character_weapon_loadouts w ON w.id=e.weapon_loadout_id
                 LEFT JOIN damage_encounters encounter ON encounter.id=e.encounter_id
                 WHERE e.encounter_id=? ORDER BY e.happened_at,e.id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([id], |row| {
                Ok(json!({
                    "id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,
                    "damageType":row.get::<_,String>(2)?,"attack":row.get::<_,String>(3)?,
                    "damage":row.get::<_,i64>(4)?,
                    "primaryWeapon":row.get::<_,Option<String>>(5)?,
                    "primaryItemId":row.get::<_,Option<i64>>(6)?,
                    "secondaryWeapon":row.get::<_,Option<String>>(7)?,
                    "secondaryItemId":row.get::<_,Option<i64>>(8)?,
                    "attacker":row.get::<_,String>(9)?
                }))
            })
            .map_err(err)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(err)?
    };
    let mut value = encounter;
    value
        .as_object_mut()
        .expect("damage encounter query returns an object")
        .insert("events".into(), Value::Array(events));
    Ok(value)
}

pub fn page_snapshot(database: &Database, page: &str) -> Result<Value, String> {
    let mut value = snapshot(database)?;
    let Some(root) = value.as_object_mut() else {
        return Ok(value);
    };
    let keep: &[&str] = match page {
        "live" => &["loot", "items", "mobs"],
        "linked" => &["linkedLoot"],
        "tracked" => &["tracked"],
        "death-reports" => &["deathReports"],
        "damage" => &["damageEncounters"],
        "merchant" => &["merchant"],
        "splits" => &["splits", "history", "aliases", "items", "mobs"],
        "compounds" => &["compound", "items", "inventory", "members"],
        "characters" => &["inventory", "spells", "items", "compound"],
        "spells" => &["spells", "items"],
        "gems" => &["inventory"],
        "imports" => &["imports"],
        "wts" => &["wts", "inventory", "items"],
        "items" => &["items"],
        "system" => &["aliases", "imports"],
        "logs" => &["logs"],
        _ => &[],
    };
    for key in [
        "loot",
        "splits",
        "tracked",
        "linkedLoot",
        "deathReports",
        "damageEncounters",
        "history",
        "items",
        "inventory",
        "spells",
        "wts",
        "aliases",
        "mobs",
        "logs",
        "imports",
        "merchant",
    ] {
        if key != "members" && !keep.contains(&key) {
            root.insert(key.into(), json!([]));
        }
    }
    if !keep.contains(&"compound") {
        root.insert(
            "compound".into(),
            json!({"projects":[],"templates":[],"activeId":null}),
        );
    }
    Ok(value)
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
        "linked.delete" => {
            for id in integers(payload, "ids") {
                connection
                    .execute("DELETE FROM linked_loot_items WHERE id=?", [id])
                    .map_err(err)?;
            }
        }
        "linked.clear" => {
            connection
                .execute("DELETE FROM linked_loot_items", [])
                .map_err(err)?;
        }
        "linked.rescan" => return rescan_linked_loot(&connection),
        "split.add" => add_split(&mut connection, payload)?,
        "split.save" => save_split(&mut connection, payload)?,
        "split.delete" => delete_split(&mut connection, required(payload, "key")?)?,
        "split.complete" => complete_split(&mut connection, payload)?,
        "history.save" => save_history(&mut connection, payload)?,
        "history.payout.member.complete" => {
            set_split_member_payout(&mut connection, payload, true)?;
        }
        "history.payout.member.reopen" => {
            set_split_member_payout(&mut connection, payload, false)?;
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
    if matches!(
        action,
        "item.save" | "item.delete" | "inventory.import" | "inventory.importFiles"
    ) {
        Database::refresh_item_values(&connection).map_err(|error| error.to_string())?;
    }
    connection
        .execute(
            "INSERT INTO application_logs(level,area,message) VALUES('info','action',?)",
            [action],
        )
        .map_err(err)?;
    Ok(json!({"ok":true}))
}

fn rescan_linked_loot(connection: &rusqlite::Connection) -> Result<Value, String> {
    let path = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='active_log_path'",
            [],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| "No active log file is available to rescan".to_owned())?;
    let character = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='active_character'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "Unknown".to_owned());
    let file =
        fs::File::open(&path).map_err(|error| format!("Could not open active log: {error}"))?;
    let mut reader = BufReader::new(file);
    let mut line_bytes = Vec::new();
    let mut source_offset = 0_i64;
    let mut found = 0_i64;
    let mut inserted = 0_i64;

    loop {
        line_bytes.clear();
        let read = reader
            .read_until(b'\n', &mut line_bytes)
            .map_err(|error| format!("Could not read active log: {error}"))?;
        if read == 0 {
            break;
        }
        if !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(&line_bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        if let Some(LogEvent::LinkedItems {
            happened_at,
            speaker,
            channel,
            message,
            mut item_names,
        }) = parse_log_event(line, &character)
        {
            item_names = super::runtime::resolve_linked_items(connection, &message, &item_names)?;
            found += item_names.len() as i64;
            for (link_index, item_name) in item_names.iter().enumerate() {
                inserted += connection
                    .execute(
                        "INSERT INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                         VALUES(?,?,?,?,?,?,?,?)
                         ON CONFLICT(source_file,source_offset,link_index) DO UPDATE SET
                           happened_at=excluded.happened_at,
                           channel=excluded.channel,
                           speaker_name=excluded.speaker_name,
                           item_name=excluded.item_name,
                           raw_line=excluded.raw_line
                         WHERE linked_loot_items.item_name<>excluded.item_name COLLATE NOCASE",
                        params![
                            happened_at.to_string(),
                            channel.as_str(),
                            speaker,
                            item_name,
                            line,
                            path,
                            source_offset,
                            link_index as i64
                        ],
                    )
                    .map_err(err)? as i64;
            }
        }
        source_offset += read as i64;
    }
    connection
        .execute(
            "INSERT INTO application_logs(level,area,message) VALUES('info','linked-loot',?)",
            [format!(
                "Rescanned active log: found {found} linked items, recovered {inserted} missing records"
            )],
        )
        .map_err(err)?;
    Ok(json!({"ok":true,"found":found,"inserted":inserted}))
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
         SELECT d.id,d.happened_at,d.item_name,COALESCE(m.name,d.mob_name),d.looter_name,rv.value_pp
         FROM loot_drops d LEFT JOIN mobs m ON m.id=d.mob_id
         LEFT JOIN item_name_resolutions ni ON ni.item_name=d.item_name COLLATE NOCASE
         LEFT JOIN resolved_item_values rv ON rv.item_id=ni.item_id
         WHERE d.id=?",
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
    connection.execute("INSERT INTO manual_split_list_items(item_name,mob_id,looter_name,payout_value_pp) VALUES(?,?,?,?)",params![item,mob_id,looter,payload.get("payoutValuePp").and_then(Value::as_i64).filter(|value| *value > 0)]).map_err(err)?;
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
    let payout = payload
        .get("payoutValuePp")
        .and_then(Value::as_i64)
        .filter(|value| *value > 0);
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

fn save_history(connection: &mut rusqlite::Connection, payload: &Value) -> Result<(), String> {
    let id = integer(payload, "id")?;
    let disposition = required(payload, "disposition")?;
    if !matches!(disposition.as_str(), "sold" | "consumed") {
        return Err("disposition must be sold or consumed".into());
    }
    let attendees = payload.get("attendees").map(|_| {
        let mut unique = HashMap::new();
        for name in strings(payload, "attendees") {
            unique.entry(name.to_lowercase()).or_insert(name);
        }
        let mut names = unique.into_values().collect::<Vec<_>>();
        names.sort_by_key(|name| name.to_lowercase());
        names
    });
    if attendees.as_ref().is_some_and(Vec::is_empty) {
        return Err("Choose at least one payout participant".into());
    }

    let transaction = connection.transaction().map_err(err)?;
    let existing_payouts = {
        let mut statement = transaction
            .prepare(
                "SELECT member_name,paid_at FROM completed_split_payouts
                 WHERE completed_split_item_id=?",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(err)?
            .filter_map(Result::ok)
            .collect::<HashMap<_, _>>();
        rows
    };

    let updated = transaction
        .execute(
            "UPDATE completed_split_items SET disposition=?,value_pp=?,note=? WHERE id=?",
            params![
                disposition,
                integer(payload, "valuePp")?,
                payload.get("note").and_then(Value::as_str).unwrap_or(""),
                id
            ],
        )
        .map_err(err)?;
    if updated == 0 {
        return Err("Sale record was not found".into());
    }

    if let Some(attendees) = attendees {
        transaction
            .execute(
                "DELETE FROM completed_split_members WHERE completed_split_item_id=?",
                [id],
            )
            .map_err(err)?;
        transaction
            .execute(
                "DELETE FROM completed_split_payouts WHERE completed_split_item_id=?",
                [id],
            )
            .map_err(err)?;
        for name in attendees {
            remember(&transaction, &name)?;
            transaction
                .execute(
                    "INSERT INTO completed_split_members(completed_split_item_id,member_name)
                     VALUES(?,?)",
                    params![id, name],
                )
                .map_err(err)?;
            if disposition == "sold" {
                if let Some((_, paid_at)) = existing_payouts
                    .iter()
                    .find(|(paid_name, _)| paid_name.eq_ignore_ascii_case(&name))
                {
                    transaction
                        .execute(
                            "INSERT INTO completed_split_payouts(
                                completed_split_item_id,member_name,paid_at
                             ) VALUES(?,?,?)",
                            params![id, name, paid_at],
                        )
                        .map_err(err)?;
                }
            }
        }
    }

    if disposition == "consumed" {
        transaction
            .execute(
                "DELETE FROM completed_split_payouts WHERE completed_split_item_id=?",
                [id],
            )
            .map_err(err)?;
        transaction
            .execute(
                "UPDATE completed_split_items
                 SET payout_status='completed',paid_at=COALESCE(paid_at,CURRENT_TIMESTAMP)
                 WHERE id=?",
                [id],
            )
            .map_err(err)?;
    } else {
        let unpaid: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM completed_split_members m
                 LEFT JOIN completed_split_payouts p
                   ON p.completed_split_item_id=m.completed_split_item_id
                  AND p.member_name=m.member_name COLLATE NOCASE
                 WHERE m.completed_split_item_id=? AND p.member_name IS NULL",
                [id],
                |row| row.get(0),
            )
            .map_err(err)?;
        transaction
            .execute(
                "UPDATE completed_split_items
                 SET payout_status=CASE WHEN ?=0 THEN 'completed' ELSE 'pending' END,
                     paid_at=CASE WHEN ?=0 THEN COALESCE(paid_at,CURRENT_TIMESTAMP) ELSE NULL END
                 WHERE id=?",
                params![unpaid, unpaid, id],
            )
            .map_err(err)?;
    }
    transaction.commit().map_err(err)?;
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

fn set_split_member_payout(
    connection: &mut rusqlite::Connection,
    payload: &Value,
    paid: bool,
) -> Result<(), String> {
    let id = integer(payload, "id")?;
    let canonical = required(payload, "memberName")?;
    if paid {
        connection
            .execute(
                "INSERT OR REPLACE INTO completed_split_payouts(completed_split_item_id,member_name,paid_at)
                 SELECT m.completed_split_item_id,m.member_name,CURRENT_TIMESTAMP
                 FROM completed_split_members m
                 JOIN completed_split_items h ON h.id=m.completed_split_item_id
                 WHERE m.completed_split_item_id=? AND h.disposition='sold'
                   AND COALESCE((SELECT canonical_name FROM character_aliases a WHERE a.alias_name=m.member_name COLLATE NOCASE),m.member_name)=? COLLATE NOCASE",
                params![id, canonical],
            )
            .map_err(err)?;
    } else {
        connection
            .execute(
                "DELETE FROM completed_split_payouts
                 WHERE completed_split_item_id=? AND member_name IN (
                   SELECT m.member_name FROM completed_split_members m
                   WHERE m.completed_split_item_id=?
                     AND COALESCE((SELECT canonical_name FROM character_aliases a WHERE a.alias_name=m.member_name COLLATE NOCASE),m.member_name)=? COLLATE NOCASE
                 )",
                params![id, id, canonical],
            )
            .map_err(err)?;
    }
    let unpaid: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM completed_split_members m
             LEFT JOIN completed_split_payouts p ON p.completed_split_item_id=m.completed_split_item_id AND p.member_name=m.member_name COLLATE NOCASE
             WHERE m.completed_split_item_id=? AND p.member_name IS NULL",
            [id],
            |row| row.get(0),
        )
        .map_err(err)?;
    let all_paid = unpaid == 0;
    connection
        .execute(
            "UPDATE completed_split_items
             SET payout_status=CASE WHEN ? THEN 'completed' ELSE 'pending' END,
                 paid_at=CASE WHEN ? THEN COALESCE(paid_at,CURRENT_TIMESTAMP) ELSE NULL END
             WHERE id=? AND disposition='sold'",
            params![all_paid, all_paid, id],
        )
        .map_err(err)?;
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
    connection.execute("INSERT INTO completed_split_items(item_name,mob_name,looter_name,value_pp,disposition,note,payout_status,paid_at) VALUES(?,?,?,?,?,?,CASE WHEN ?='consumed' THEN 'completed' ELSE 'pending' END,CASE WHEN ?='consumed' THEN CURRENT_TIMESTAMP ELSE NULL END)",params![row.0,row.2,row.1,integer(payload,"valuePp")?,required(payload,"disposition")?,payload.get("note").and_then(Value::as_str).unwrap_or(""),required(payload,"disposition")?,required(payload,"disposition")?]).map_err(err)?;
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
        let items = parse_inventory(text);
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
        record_weapon_loadout(connection, character, source, &items)?;
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

fn record_weapon_loadout(
    connection: &rusqlite::Connection,
    character: &str,
    source: &str,
    items: &[crate::domain::inventory::InventoryItem],
) -> Result<(), String> {
    let equipped = |slot: &str| {
        items.iter().find(|item| {
            item.location.eq_ignore_ascii_case(slot) && !item.item_name.trim().is_empty()
        })
    };
    let primary = equipped("Primary");
    let secondary = equipped("Secondary");
    let captured_at = Local::now()
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    connection
        .execute(
            "INSERT INTO character_weapon_loadouts(
                character_name,captured_at,primary_weapon_name,primary_item_id,
                secondary_weapon_name,secondary_item_id,source_file
             ) VALUES(?,?,?,?,?,?,?)",
            params![
                character,
                captured_at,
                primary.map(|item| item.item_name.trim()),
                primary.and_then(|item| item.item_id),
                secondary.map(|item| item.item_name.trim()),
                secondary.and_then(|item| item.item_id),
                source
            ],
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

#[cfg(test)]
mod tests {
    use super::{
        activity_history_snapshot, damage_encounter_details, mutate, normalize_compound, snapshot,
    };
    use crate::infrastructure::database::Database;
    use rusqlite::params;
    use serde_json::json;
    use std::io::Write;

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
    fn damage_snapshot_and_detail_share_the_same_persisted_encounter() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO damage_encounters(
                character_name,mob_name,started_at,last_damage_at,total_damage,melee_damage,
                spell_damage,hit_count,max_hit,outcome,source_file,first_source_offset,last_source_offset
             ) VALUES('Youngman','a frost giant','2026-09-05 10:00:00','2026-09-05 10:00:02',
                      350,100,250,2,250,'slain','eqlog_Youngman_P1999Green.txt',1,2)",
            [],
        ).unwrap();
        let encounter_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO damage_events(
                encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,source_offset,attacker_name
             ) VALUES(?,'2026-09-05 10:00:00','melee','slash',100,'raw one',
                      'eqlog_Youngman_P1999Green.txt',1,'Youngman'),
                     (?,'2026-09-05 10:00:02','spell','non-melee',250,'raw two',
                      'eqlog_Youngman_P1999Green.txt',2,'Legiteral')",
            [encounter_id, encounter_id],
        ).unwrap();
        drop(connection);

        let overview = snapshot(&database).unwrap();
        assert_eq!(overview["damageEncounters"][0]["totalDamage"], 350);
        assert_eq!(
            overview["damageEncounters"][0]["players"][0]["name"],
            "Legiteral"
        );
        assert_eq!(
            overview["damageEncounters"][0]["players"][0]["totalDamage"],
            250
        );
        assert_eq!(
            overview["damageEncounters"][0]["players"][1]["name"],
            "Youngman"
        );
        let detail = damage_encounter_details(&database, encounter_id).unwrap();
        assert_eq!(detail["mobName"], "a frost giant");
        assert_eq!(detail["events"].as_array().unwrap().len(), 2);
        assert_eq!(detail["events"][1]["damage"], 250);
        assert_eq!(detail["events"][1]["attacker"], "Legiteral");
    }

    #[test]
    fn damage_snapshot_exposes_the_active_characters_latest_weapon_loadout() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_character','Youngman')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO character_weapon_loadouts(
                    character_name,captured_at,primary_weapon_name,primary_item_id,
                    secondary_weapon_name,secondary_item_id,source_file
                 ) VALUES
                    ('Youngman','2026-09-05 09:00:00','Old Sword',1,NULL,NULL,'old.txt'),
                    ('Youngman','2026-09-05 10:00:00','New Sword',2,'New Dagger',3,'new.txt'),
                    ('SomeoneElse','2026-09-05 11:00:00','Wrong Sword',4,NULL,NULL,'wrong.txt')",
                [],
            )
            .unwrap();
        drop(connection);

        let overview = snapshot(&database).unwrap();
        assert_eq!(overview["currentWeaponLoadout"]["character"], "Youngman");
        assert_eq!(overview["currentWeaponLoadout"]["primary"], "New Sword");
        assert_eq!(overview["currentWeaponLoadout"]["primaryItemId"], 2);
        assert_eq!(overview["currentWeaponLoadout"]["secondary"], "New Dagger");
        assert_eq!(overview["currentWeaponLoadout"]["secondaryItemId"], 3);
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
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        mutate(&database,"inventory.importFiles",&json!({"files":[{"name":"Khards-Inventory.txt","text":"Primary\tA Blue Crown\t12345\t1\nSecondary\tOffhand Test\t54321\t1\n"}]})).unwrap();

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
        let loadout: (Option<String>, Option<String>) = connection
            .query_row(
                "SELECT primary_weapon_name,secondary_weapon_name
                 FROM character_weapon_loadouts WHERE character_name='Khards'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            loadout,
            (Some("A Blue Crown".into()), Some("Offhand Test".into()))
        );
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
        Database::refresh_item_values(&connection).unwrap();
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
        Database::refresh_item_values(&connection).unwrap();
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
        Database::refresh_item_values(&connection).unwrap();
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
    fn linked_loot_snapshot_associates_primary_value_by_item_name() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO master_items(item_id,item_name,source) VALUES(42,'A Blue Crown','market')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,count_30d,fetched_at)
             VALUES('Green',42,0,'A Blue Crown','Today',1750,9,CURRENT_TIMESTAMP)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
             VALUES('2026-08-19 12:00:00','guild','Posed','a blue crown','raw','eqlog_Youngman_P1999Green.txt',42,0)",
            [],
        ).unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        let linked = &value["linkedLoot"][0];
        assert_eq!(linked["speakerName"], "Posed");
        assert_eq!(linked["channel"], "guild");
        assert_eq!(linked["itemId"], 42);
        assert_eq!(linked["valuePp"], 1750);
        assert_eq!(linked["count30d"], 9);
    }

    #[test]
    fn linked_loot_snapshot_prefers_exact_name_when_inventory_and_market_ids_differ() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source) VALUES(10383,'Rod of Oblations','inventory')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,count_30d,fetched_at)
                 VALUES('Green',15549,0,'Rod of Oblations','Today',135,40,CURRENT_TIMESTAMP)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                 VALUES('2026-08-29 13:21:50','group','Youngman','Rod of Oblations','raw','eqlog_Youngman_P1999Green.txt',4000,0)",
                [],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        let linked = &value["linkedLoot"][0];
        assert_eq!(linked["itemId"], 10383);
        assert_eq!(linked["valuePp"], 135);
        assert_eq!(linked["count30d"], 40);
        assert_eq!(linked["valueBasis"], "30-day WTS");
    }

    #[test]
    fn linked_loot_snapshot_associates_price_by_master_id_across_name_variants() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source) VALUES(20819,'Elders Earring','inventory')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,average_30d_pp,count_30d,average_60d_pp,count_60d,fetched_at)
                 VALUES('Green',20819,1,'Elder''s Earring','Today',0,0,7400,11,CURRENT_TIMESTAMP)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                 VALUES('2026-08-28 23:33:12','group','Lith','Elders Earring','raw','eqlog_Gnorby_P1999Green.txt',3571,0)",
                [],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        let linked = &value["linkedLoot"][0];
        assert_eq!(linked["itemId"], 20819);
        assert_eq!(linked["valuePp"], 7400);
        assert_eq!(linked["count30d"], 11);
        assert_eq!(linked["valueBasis"], "60-day WTB");
    }

    #[test]
    fn linked_loot_rescan_recovers_real_clickable_links_without_replaying_group_state() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let first = format!("\u{12}{}A Blue Crown\u{12}", "0".repeat(45));
        let second = format!("\u{12}{}White Dragon Scale\u{12}", "F".repeat(45));
        std::fs::write(
            &log,
            format!(
                "[Thu Aug 27 12:41:43 2026] Dubbyl tells the group, 'Look: {first} / {second}'\r\n"
            ),
        )
        .unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source) VALUES
                 (1,'A Blue Crown','test'),
                 (2,'White Dragon Scale','test'),
                 (3,'Crown','test'),
                 (4,'Dark Ember','test'),
                 (5,'Gauntlets of the Black','test')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_log_path',?),('active_character','Youngman')",
                [log.display().to_string()],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(
                b"[Thu Aug 27 12:41:44 2026] Dubbyl tells the guild, 'Anyone need A Blue Crown or White Dragon Scale tonight?'\r\n",
            )
            .unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(
                b"[Thu Aug 27 12:41:45 2026] You tell your party, 'Dark Ember, Gauntlets of the BlackDark Ember'\r\n",
            )
            .unwrap();

        let first_result = mutate(&database, "linked.rescan", &json!({})).unwrap();
        let second_result = mutate(&database, "linked.rescan", &json!({})).unwrap();
        assert_eq!(first_result["found"], 7);
        assert_eq!(first_result["inserted"], 7);
        assert_eq!(second_result["inserted"], 0);

        let connection = database.connect().unwrap();
        let linked = connection
            .query_row("SELECT COUNT(*) FROM linked_loot_items", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let group = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        assert_eq!((linked, group), (7, 0));
    }

    #[test]
    fn linked_loot_rescan_corrects_a_previous_short_suffix_match() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let raw = "[Fri Aug 28 12:47:23 2026] Balbazak tells the group, 'Dusty Rusted Shackles where did that other lizard go'";
        std::fs::write(&log, format!("{raw}\r\n")).unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_log_path',?),('active_character','Youngman')",
                [log.display().to_string()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                 VALUES('2026-08-28 12:47:23','group','Balbazak','Shackles',?,?,0,0)",
                params![raw, log.display().to_string()],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        let result = mutate(&database, "linked.rescan", &json!({})).unwrap();
        assert_eq!(result["inserted"], 1);
        let connection = database.connect().unwrap();
        let corrected = connection
            .query_row("SELECT item_name FROM linked_loot_items", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(corrected, "Dusty Rusted Shackles");
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
        Database::refresh_item_values(&connection).unwrap();
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

    #[test]
    fn split_payouts_complete_independently_and_respect_aliases() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO completed_split_items(item_name,value_pp,disposition,note,payout_status) VALUES('Shared Item',300,'sold','','pending')",
            [],
        ).unwrap();
        let item_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO completed_split_members(completed_split_item_id,member_name) VALUES(?,'Main'),(?,'Alt'),(?,'Friend')",
            params![item_id,item_id,item_id],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO character_aliases(alias_name,canonical_name) VALUES('Alt','Main')",
                [],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        mutate(
            &database,
            "history.payout.member.complete",
            &json!({"id":item_id,"memberName":"Main"}),
        )
        .unwrap();
        let connection = database.connect().unwrap();
        let paid_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM completed_split_payouts WHERE completed_split_item_id=?",
                [item_id],
                |row| row.get(0),
            )
            .unwrap();
        let status: String = connection
            .query_row(
                "SELECT payout_status FROM completed_split_items WHERE id=?",
                [item_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((paid_count, status), (2, "pending".to_owned()));
        Database::refresh_item_values(&connection).unwrap();
        drop(connection);

        mutate(
            &database,
            "history.payout.member.complete",
            &json!({"id":item_id,"memberName":"Friend"}),
        )
        .unwrap();
        let value = snapshot(&database).unwrap();
        assert_eq!(value["history"][0]["payoutStatus"], "completed");
        assert_eq!(value["history"][0]["payouts"].as_array().unwrap().len(), 3);

        mutate(
            &database,
            "history.payout.member.reopen",
            &json!({"id":item_id,"memberName":"Main"}),
        )
        .unwrap();
        let value = snapshot(&database).unwrap();
        assert_eq!(value["history"][0]["payoutStatus"], "pending");
        assert_eq!(value["history"][0]["payouts"].as_array().unwrap().len(), 1);
        assert_eq!(value["history"][0]["payouts"][0]["name"], "Friend");
    }
    #[test]
    fn editing_a_sale_can_add_a_pending_participant_without_losing_paid_players() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO completed_split_items(
                item_name,value_pp,disposition,note,payout_status,paid_at
             ) VALUES('Shared Item',300,'sold','original','completed','2026-09-01')",
                [],
            )
            .unwrap();
        let item_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO completed_split_members(completed_split_item_id,member_name)
             VALUES(?,'Main'),(?,'Friend')",
                params![item_id, item_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO completed_split_payouts(completed_split_item_id,member_name,paid_at)
             VALUES(?,'Main','2026-09-02'),(?,'Friend','2026-09-03')",
                params![item_id, item_id],
            )
            .unwrap();
        drop(connection);

        mutate(
            &database,
            "history.save",
            &json!({
                "id":item_id,
                "disposition":"sold",
                "valuePp":300,
                "note":"added late participant",
                "attendees":["Main","Friend","Newperson"]
            }),
        )
        .unwrap();

        let value = snapshot(&database).unwrap();
        let history = &value["history"][0];
        assert_eq!(history["attendees"], json!(["Friend", "Main", "Newperson"]));
        assert_eq!(history["payoutStatus"], "pending");
        assert_eq!(history["paidAt"], json!(null));
        assert_eq!(history["payouts"].as_array().unwrap().len(), 2);
        assert!(history["payouts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|payout| payout["name"] != "Newperson"));
    }

    #[test]
    fn activity_history_snapshot_resolves_shared_item_values() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute(
            "INSERT INTO master_items(item_id,item_name,source) VALUES(77,'Blue Diamond','test')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO item_market_values(server,source_item_id,transaction_type,item_name,last_seen,count_30d,average_30d_pp,fetched_at)
             VALUES('Green',77,0,'Blue Diamond','2026-09-04',4,250,'2026-09-04')",
            [],
        ).unwrap();
        Database::refresh_item_values(&connection).unwrap();
        connection.execute(
            "INSERT INTO activity_loot_history(happened_at,character_name,item_name,looter_name,raw_line,source_file,source_offset)
             VALUES('2026-09-04 12:00:00','Youngman','Blue Diamond','Youngman','loot raw','eqlog_Youngman_P1999Green.txt',1)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO activity_offer_history(happened_at,character_name,offerer_name,item_name,item_index,raw_line,source_file,source_offset)
             VALUES('2026-09-04 12:01:00','Posed','Youngman','Blue Diamond',0,'offer raw','eqlog_Posed_P1999Green.txt',1)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO activity_level_history(happened_at,character_name,level,direction,raw_line,source_file,source_offset)
             VALUES('2026-09-04 12:02:00','Posed',54,'gained','level raw','eqlog_Posed_P1999Green.txt',2)",
            [],
        ).unwrap();
        drop(connection);

        let value = activity_history_snapshot(&database).unwrap();
        assert_eq!(value["loot"][0]["valuePp"], 250);
        assert_eq!(value["loot"][0]["valueBasis"], "30-day WTS");
        assert_eq!(value["offers"][0]["itemId"], 77);
        assert_eq!(value["offers"][0]["valueSamples"], 4);
        assert_eq!(value["levels"][0]["character"], "Posed");
        assert_eq!(value["levels"][0]["level"], 54);
        assert_eq!(value["levels"][0]["direction"], "gained");
    }

    #[test]
    fn snapshot_uses_zero_samples_for_items_without_a_catalog_match() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();

        connection.execute(
            "INSERT INTO loot_drops(happened_at,item_name,raw_line,source_file,source_offset) VALUES('2026-09-04 12:00:00','Unknown Loot','raw','eqlog_Test_P1999Green.txt',1)",
            [],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO manual_split_list_items(item_name) VALUES('Unknown Split')",
                [],
            )
            .unwrap();
        connection.execute(
            "INSERT INTO inventory_characters(name,source_file,imported_at) VALUES('Test','Test-Inventory.txt','2026-09-04 12:00:00')",
            [],
        ).unwrap();
        let character_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO inventory_items(character_id,location,item_name,item_count,sort_order) VALUES(?1,'Carried','Unknown Inventory',1,1)",
            [character_id],
        ).unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        assert_eq!(value["loot"][0]["valueSamples"], 0);
        assert_eq!(value["splits"][0]["marketValueSamples"], 0);
        assert_eq!(value["inventory"][0]["valueSamples"], 0);
    }
}
