use rusqlite::{params, Connection, OptionalExtension};

const EMBEDDED_CATALOG: &str = include_str!("../../assets/p99-spell-research.tsv");
const CATALOG_VERSION: &str = "p99-2026-09-24-v1";

pub fn reconcile(connection: &Connection) -> rusqlite::Result<usize> {
    let installed = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='spell_research_catalog_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if installed.as_deref() == Some(CATALOG_VERSION) {
        return Ok(0);
    }

    connection.execute("DELETE FROM spell_research_components", [])?;
    connection.execute("DELETE FROM spell_research_recipes", [])?;
    let mut inserted = 0;
    for line in EMBEDDED_CATALOG
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 11 {
            continue;
        }
        let class_name = fields[0];
        let level = fields[1].parse::<i64>().unwrap_or_default();
        let spell_name = fields[2];
        let trivial = fields[3];
        let research_only = fields[4].parse::<i64>().unwrap_or_default();
        let availability = fields[5];
        let source_url = fields[6];
        let component_name = fields[7];
        let icon_id = fields[8].parse::<i64>().ok();
        let quantity = fields[9].parse::<i64>().unwrap_or(1).max(1);
        let component_kind = fields[10];
        let catalog_key = format!("{class_name}|{level}|{spell_name}");
        connection.execute(
            "INSERT INTO spell_research_recipes(
                catalog_key,class_name,spell_level,spell_name,spell_item_id,trivial,
                research_only,availability,source_url,updated_at
             ) VALUES(
                ?1,?2,?3,?4,
                (SELECT item_id FROM item_name_resolutions WHERE item_name=('Spell: '||?4) COLLATE NOCASE),
                ?5,?6,?7,?8,CURRENT_TIMESTAMP
             ) ON CONFLICT(catalog_key) DO UPDATE SET
                spell_item_id=COALESCE(excluded.spell_item_id,spell_research_recipes.spell_item_id),
                trivial=excluded.trivial,research_only=excluded.research_only,
                availability=excluded.availability,source_url=excluded.source_url,
                updated_at=CURRENT_TIMESTAMP",
            params![catalog_key,class_name,level,spell_name,trivial,research_only,availability,source_url],
        )?;
        let recipe_id = connection.query_row(
            "SELECT id FROM spell_research_recipes WHERE catalog_key=?",
            [catalog_key],
            |row| row.get::<_, i64>(0),
        )?;
        inserted += connection.execute(
            "INSERT INTO spell_research_components(
                recipe_id,item_id,item_name,icon_id,quantity,component_kind
             ) VALUES(
                ?1,(SELECT item_id FROM item_name_resolutions WHERE item_name=?2 COLLATE NOCASE),
                ?2,?3,?4,?5
             ) ON CONFLICT(recipe_id,item_name) DO UPDATE SET
                item_id=COALESCE(excluded.item_id,spell_research_components.item_id),
                icon_id=excluded.icon_id,quantity=excluded.quantity,
                component_kind=excluded.component_kind",
            params![recipe_id, component_name, icon_id, quantity, component_kind],
        )?;
    }
    connection.execute(
        "INSERT INTO app_settings(key,value) VALUES('spell_research_catalog_version',?)
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
    fn bundled_catalog_covers_all_research_classes_and_component_families() {
        let folder = tempdir().unwrap();
        let database = Database::open(folder.path().join("loot.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 38);
        let connection = database.connect().unwrap();
        let recipe_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM spell_research_recipes", [], |row| {
                row.get(0)
            })
            .unwrap();
        let component_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM spell_research_components",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let class_count: i64 = connection
            .query_row(
                "SELECT COUNT(DISTINCT class_name) FROM spell_research_recipes",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((recipe_count, component_count, class_count), (117, 275, 4));
        for kind in ["word", "page", "rune", "spell", "other"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM spell_research_components WHERE component_kind=?",
                    [kind],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(count > 0, "missing {kind} components");
        }
        let levitate_quantity:i64=connection.query_row(
            "SELECT c.quantity FROM spell_research_components c JOIN spell_research_recipes r ON r.id=c.recipe_id WHERE r.spell_name='Levitate' AND r.class_name='Enchanter'",
            [],|row|row.get(0)).unwrap();
        assert_eq!(levitate_quantity, 2);
        assert_eq!(reconcile(&connection).unwrap(), 0);
    }
}
