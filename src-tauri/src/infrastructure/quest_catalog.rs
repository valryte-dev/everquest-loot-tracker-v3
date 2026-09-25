use rusqlite::{params, Connection, OptionalExtension};

const EMBEDDED_CATALOG: &str = include_str!("../../assets/p99-quest-items.tsv");
const CATALOG_VERSION: &str = "p99-2026-09-11-v2";

pub fn reconcile(connection: &Connection) -> rusqlite::Result<usize> {
    let installed = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='quest_catalog_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if installed.as_deref() == Some(CATALOG_VERSION) {
        return Ok(0);
    }

    connection.execute("DELETE FROM quest_catalog_components", [])?;
    connection.execute("DELETE FROM quest_catalog_entries", [])?;
    let mut inserted = 0;
    for line in EMBEDDED_CATALOG
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 12 {
            continue;
        }
        let category = fields[0];
        let class_name = fields[1];
        let quest_name = fields[2];
        let reward_name = fields[3];
        let reward_icon_id = fields[4].parse::<i64>().ok();
        let component_name = fields[5];
        let component_icon_id = fields[6].parse::<i64>().ok();
        let quantity = fields[7].parse::<i64>().unwrap_or(1).max(1);
        let slot = fields[8];
        let faction = fields[9];
        let note = fields[10];
        let source_url = fields[11];
        let catalog_key =
            format!("{category}|{class_name}|{quest_name}|{reward_name}|{slot}|{faction}");
        connection.execute(
            "INSERT INTO quest_catalog_entries(
                catalog_key,category,class_name,quest_name,reward_item_id,reward_name,
                reward_icon_id,slot,faction,note,source_url,updated_at
             ) VALUES(
                ?1,?2,?3,?4,
                (SELECT item_id FROM item_name_resolutions WHERE item_name=?5 COLLATE NOCASE),
                ?5,?6,?7,?8,?9,?10,CURRENT_TIMESTAMP
             ) ON CONFLICT(catalog_key) DO UPDATE SET
                reward_item_id=COALESCE(excluded.reward_item_id,quest_catalog_entries.reward_item_id),
                reward_icon_id=excluded.reward_icon_id,note=excluded.note,
                source_url=excluded.source_url,updated_at=CURRENT_TIMESTAMP",
            params![
                catalog_key,
                category,
                class_name,
                quest_name,
                reward_name,
                reward_icon_id,
                slot,
                faction,
                note,
                source_url
            ],
        )?;
        let entry_id = connection.query_row(
            "SELECT id FROM quest_catalog_entries WHERE catalog_key=?",
            [catalog_key],
            |row| row.get::<_, i64>(0),
        )?;
        inserted += connection.execute(
            "INSERT INTO quest_catalog_components(
                quest_entry_id,item_id,item_name,icon_id,quantity
             ) VALUES(
                ?1,(SELECT item_id FROM item_name_resolutions WHERE item_name=?2 COLLATE NOCASE),
                ?2,?3,?4
             ) ON CONFLICT(quest_entry_id,item_name) DO UPDATE SET
                item_id=COALESCE(excluded.item_id,quest_catalog_components.item_id),
                icon_id=excluded.icon_id,quantity=excluded.quantity",
            params![entry_id, component_name, component_icon_id, quantity],
        )?;
    }
    connection.execute(
        "INSERT INTO app_settings(key,value) VALUES('quest_catalog_version',?)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [CATALOG_VERSION],
    )?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;
    use tempfile::tempdir;

    #[test]
    fn bundled_catalog_covers_every_requested_family_and_class() {
        let folder = tempdir().unwrap();
        let database = Database::open(folder.path().join("loot.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let counts = connection
            .prepare(
                "SELECT category,COUNT(DISTINCT class_name),COUNT(*)
                 FROM quest_catalog_entries e
                 JOIN quest_catalog_components c ON c.quest_entry_id=e.id
                 GROUP BY category ORDER BY category",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            counts,
            vec![
                ("epic".into(), 14, 351),
                ("plane_of_sky".into(), 14, 272),
                ("velious_armor".into(), 14, 294),
            ]
        );
        let bracelet: String = connection
            .query_row(
                "SELECT item_name FROM quest_catalog_components c
                 JOIN quest_catalog_entries e ON e.id=c.quest_entry_id
                 WHERE e.category='velious_armor' AND e.class_name='Warrior'
                   AND e.faction='Kael' AND e.slot='Wrist'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bracelet, "Ancient Tarnished Plate Bracelet");
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source,updated_at)
                 VALUES(777,'Ochre Tessera','test',CURRENT_TIMESTAMP)",
                [],
            )
            .unwrap();
        Database::refresh_item_values(&connection).unwrap();
        let canonical_id: Option<i64> = connection
            .query_row(
                "SELECT c.item_id FROM quest_catalog_components c
                 JOIN quest_catalog_entries e ON e.id=c.quest_entry_id
                 WHERE e.category='plane_of_sky' AND c.item_name='Ochre Tessera'
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(canonical_id, Some(777));
        assert_eq!(reconcile(&connection).unwrap(), 0);
    }
}
