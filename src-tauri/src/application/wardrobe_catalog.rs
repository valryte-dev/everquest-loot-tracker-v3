use rusqlite::{Connection, OpenFlags, OptionalExtension, Row};
use serde::Serialize;
use std::{collections::HashMap, fs, path::PathBuf, sync::OnceLock};

const CATALOG_BYTES: &[u8] = include_bytes!("../../assets/p99-item-catalog.sqlite");
const SET_CATALOG: &str = include_str!("../../assets/p99-item-sets.tsv");
static CATALOG_PATH: OnceLock<Result<PathBuf, String>> = OnceLock::new();
static SET_PIECES: OnceLock<Vec<SetPiece>> = OnceLock::new();

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardrobeCatalogItem {
    id: i64,
    peq_id: Option<i64>,
    name: String,
    icon_id: Option<i64>,
    item_type: Option<i64>,
    slots: i64,
    classes: i64,
    races: i64,
    weight: i64,
    ac: i64,
    hp: i64,
    mana: i64,
    strength: i64,
    stamina: i64,
    agility: i64,
    dexterity: i64,
    intelligence: i64,
    wisdom: i64,
    charisma: i64,
    magic_resist: i64,
    fire_resist: i64,
    cold_resist: i64,
    disease_resist: i64,
    poison_resist: i64,
    attack: i64,
    haste: i64,
    mana_regen: i64,
    damage_shield: i64,
    damage: i64,
    delay: i64,
    click_name: String,
    proc_name: String,
    worn_name: String,
    focus_name: String,
    material: Option<i64>,
    id_file: Option<String>,
    color: Option<i64>,
    set_names: Vec<String>,
}

#[derive(Debug, Clone)]
struct SetPiece {
    set_name: String,
    source: String,
    classes: Vec<String>,
    item_id: i64,
    slot: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardrobeSetSummary {
    name: String,
    source: String,
    classes: Vec<String>,
    item_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WardrobeSetItem {
    slot: String,
    item: WardrobeCatalogItem,
}

fn set_pieces() -> &'static [SetPiece] {
    SET_PIECES.get_or_init(|| {
        SET_CATALOG
            .lines()
            .skip(1)
            .filter_map(|line| {
                let mut fields = line.split('\t');
                let set_name = fields.next()?.trim().to_owned();
                let source = fields.next()?.trim().to_owned();
                let classes = fields
                    .next()?
                    .split(',')
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
                    .collect();
                let item_id = fields.next()?.parse().ok()?;
                let _item_name = fields.next()?;
                let slot = fields.next()?.trim().to_owned();
                (!set_name.is_empty() && !slot.is_empty()).then_some(SetPiece {
                    set_name,
                    source,
                    classes,
                    item_id,
                    slot,
                })
            })
            .collect()
    })
}

fn set_names_by_item() -> HashMap<i64, Vec<String>> {
    let mut result: HashMap<i64, Vec<String>> = HashMap::new();
    for piece in set_pieces() {
        result
            .entry(piece.item_id)
            .or_default()
            .push(piece.set_name.clone());
    }
    result
}

fn read_item(
    row: &Row<'_>,
    set_names: &HashMap<i64, Vec<String>>,
) -> rusqlite::Result<WardrobeCatalogItem> {
    let id = row.get(0)?;
    Ok(WardrobeCatalogItem {
        id,
        peq_id: row.get(1)?,
        name: row.get(2)?,
        icon_id: row.get(3)?,
        item_type: row.get(4)?,
        slots: row.get(5)?,
        classes: row.get(6)?,
        races: row.get(7)?,
        weight: row.get(8)?,
        ac: row.get(9)?,
        hp: row.get(10)?,
        mana: row.get(11)?,
        strength: row.get(12)?,
        stamina: row.get(13)?,
        agility: row.get(14)?,
        dexterity: row.get(15)?,
        intelligence: row.get(16)?,
        wisdom: row.get(17)?,
        charisma: row.get(18)?,
        magic_resist: row.get(19)?,
        fire_resist: row.get(20)?,
        cold_resist: row.get(21)?,
        disease_resist: row.get(22)?,
        poison_resist: row.get(23)?,
        attack: row.get(24)?,
        haste: row.get(25)?,
        mana_regen: row.get(26)?,
        damage_shield: row.get(27)?,
        damage: row.get(28)?,
        delay: row.get(29)?,
        click_name: row.get(30)?,
        proc_name: row.get(31)?,
        worn_name: row.get(32)?,
        focus_name: row.get(33)?,
        material: row.get(34)?,
        id_file: row.get(35)?,
        color: row.get(36)?,
        set_names: set_names.get(&id).cloned().unwrap_or_default(),
    })
}

const ITEM_SELECT: &str =
    "SELECT id, NULLIF(peqId,0), name, NULLIF(icon,0), itemType, slots, classes, races,
 weight, ac, hp, mana, astr, asta, aagi, adex, aint, awis, acha, mr, fr, cr, dr, pr, attack, haste,
 manaregen, damageshield, damage, delay, clickName, procName, wornName, focusName,
 NULLIF(material,0), NULLIF(idfile,''), color FROM items";

fn catalog_path() -> Result<PathBuf, String> {
    CATALOG_PATH
        .get_or_init(|| {
            let path = std::env::temp_dir().join(format!(
                "everquest-loot-tracker-p99-items-{}.sqlite",
                CATALOG_BYTES.len()
            ));
            let current_size = fs::metadata(&path).map(|value| value.len()).unwrap_or(0);
            if current_size != CATALOG_BYTES.len() as u64 {
                fs::write(&path, CATALOG_BYTES).map_err(|error| {
                    format!("Could not prepare the embedded Wardrobe catalog: {error}")
                })?;
            }
            Ok(path)
        })
        .clone()
}

fn load(slot_bit: i64, class_bit: i64, race_bit: i64) -> Result<Vec<WardrobeCatalogItem>, String> {
    if slot_bit <= 0 {
        return Err("A valid equipment slot is required".to_owned());
    }
    let connection = Connection::open_with_flags(
        catalog_path()?,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Could not open the embedded Wardrobe catalog: {error}"))?;
    let set_names = set_names_by_item();
    let mut statement = connection
        .prepare(&format!(
            "{ITEM_SELECT}
             WHERE (slots & ?1) <> 0
               AND (?2 = 0 OR (classes & ?2) <> 0)
               AND (?3 = 0 OR (races & ?3) <> 0)
             ORDER BY name COLLATE NOCASE"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map((slot_bit, class_bit, race_bit), |row| {
            read_item(row, &set_names)
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn sets() -> Vec<WardrobeSetSummary> {
    let mut groups: HashMap<String, WardrobeSetSummary> = HashMap::new();
    for piece in set_pieces() {
        groups
            .entry(piece.set_name.clone())
            .or_insert_with(|| WardrobeSetSummary {
                name: piece.set_name.clone(),
                source: piece.source.clone(),
                classes: piece.classes.clone(),
                item_count: 0,
            })
            .item_count += 1;
    }
    let mut result: Vec<_> = groups.into_values().collect();
    result.sort_by_key(|set| set.name.to_lowercase());
    result
}

fn load_set(name: &str, class_bit: i64, race_bit: i64) -> Result<Vec<WardrobeSetItem>, String> {
    let connection = Connection::open_with_flags(
        catalog_path()?,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    let set_names = set_names_by_item();
    let sql = format!(
        "{ITEM_SELECT} WHERE id=?1 AND (?2=0 OR (classes & ?2)<>0) AND (?3=0 OR (races & ?3)<>0)"
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let mut result = Vec::new();
    for piece in set_pieces()
        .iter()
        .filter(|piece| piece.set_name.eq_ignore_ascii_case(name))
    {
        let item = statement
            .query_row((piece.item_id, class_bit, race_bit), |row| {
                read_item(row, &set_names)
            })
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(item) = item {
            result.push(WardrobeSetItem {
                slot: piece.slot.clone(),
                item,
            });
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn wardrobe_catalog_items(
    slot_bit: i64,
    class_bit: i64,
    race_bit: i64,
) -> Result<Vec<WardrobeCatalogItem>, String> {
    tauri::async_runtime::spawn_blocking(move || load(slot_bit, class_bit, race_bit))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn wardrobe_catalog_sets() -> Vec<WardrobeSetSummary> {
    sets()
}

#[tauri::command]
pub async fn wardrobe_catalog_set_items(
    set_name: String,
    class_bit: i64,
    race_bit: i64,
) -> Result<Vec<WardrobeSetItem>, String> {
    tauri::async_runtime::spawn_blocking(move || load_set(&set_name, class_bit, race_bit))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::{load, load_set, sets};

    #[test]
    fn filters_the_embedded_catalog_by_slot_and_exposes_weapon_stats() {
        let primary = load(8192, 1, 1).unwrap();
        assert!(!primary.is_empty());
        assert!(primary.iter().all(|item| item.slots & 8192 != 0));
        assert!(primary.iter().any(|item| item.damage > 0 && item.delay > 0));
    }

    #[test]
    fn bundled_sets_resolve_requested_examples_and_complete_pieces() {
        let summaries = sets();
        for name in ["Haze Panther Armor", "Nathsar Armor", "Netted Kelp Armor"] {
            assert!(summaries.iter().any(|set| set.name == name));
        }
        let haze = load_set("Haze Panther Armor", 0, 0).unwrap();
        assert_eq!(haze.len(), 12);
        assert!(haze
            .iter()
            .any(|piece| piece.slot == "chest" && piece.item.name == "Haze Panther Tunic"));
    }
}
