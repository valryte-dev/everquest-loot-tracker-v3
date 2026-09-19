use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;

const EMBEDDED_PROCS: &str = include_str!("../../assets/p99-weapon-procs.tsv");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcSource {
    pub spell_name: String,
    pub item_name: String,
}

/// Reconciles the bundled Project 1999 weapon-proc snapshot with the canonical item table.
/// The captured item name is retained even when a user's master catalog has not seen the item yet.
pub fn reconcile(connection: &Connection) -> rusqlite::Result<usize> {
    let mut changed = 0;
    for line in EMBEDDED_PROCS
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.split('\t');
        let (Some(item_name), Some(spell_name), Some(level)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let proc_level = level.parse::<i64>().unwrap_or(1);
        changed += connection.execute(
            "INSERT INTO item_proc_spells(item_id,item_name,spell_name,proc_level,source,updated_at)
             VALUES((SELECT item_id FROM item_name_resolutions WHERE item_name=?1 COLLATE NOCASE),
                    ?1,?2,?3,'p99-wiki',CURRENT_TIMESTAMP)
             ON CONFLICT(item_name,spell_name) DO UPDATE SET
                item_id=COALESCE(excluded.item_id,item_proc_spells.item_id),
                proc_level=excluded.proc_level,source=excluded.source,
                updated_at=CURRENT_TIMESTAMP
             WHERE item_proc_spells.item_id IS NOT COALESCE(excluded.item_id,item_proc_spells.item_id)
                OR item_proc_spells.proc_level<>excluded.proc_level
                OR item_proc_spells.source<>excluded.source",
            params![item_name, spell_name, proc_level],
        )?;
    }
    Ok(changed)
}

pub fn is_known_proc(connection: &Connection, spell_name: &str) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM item_proc_spells WHERE spell_name=? COLLATE NOCASE)",
        [spell_name],
        |row| row.get(0),
    )
}

pub fn resolve_item_spell(
    connection: &Connection,
    item_name: &str,
    candidate_spells: &[String],
) -> rusqlite::Result<Option<ProcSource>> {
    let candidates = candidate_spells
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut statement = connection.prepare(
        "SELECT spell_name,item_name FROM item_proc_spells
         WHERE item_name=? COLLATE NOCASE ORDER BY spell_name COLLATE NOCASE",
    )?;
    let found = statement
        .query_map([item_name], |row| {
            Ok(ProcSource {
                spell_name: row.get(0)?,
                item_name: row.get(1)?,
            })
        })?
        .filter_map(Result::ok)
        .filter(|source| candidates.contains(&source.spell_name.to_ascii_lowercase()))
        .collect::<Vec<_>>();
    Ok(match found.as_slice() {
        [source] => Some(source.clone()),
        _ => None,
    })
}

pub fn resolve_source(
    connection: &Connection,
    character: &str,
    candidate_spells: &[String],
) -> rusqlite::Result<Option<ProcSource>> {
    let candidates = candidate_spells
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let loadout = connection
        .query_row(
            "SELECT primary_item_id,primary_weapon_name,secondary_item_id,secondary_weapon_name
             FROM character_weapon_loadouts
             WHERE character_name=? COLLATE NOCASE
             ORDER BY captured_at DESC,id DESC LIMIT 1",
            [character],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;

    if let Some((primary_id, primary_name, secondary_id, secondary_name)) = loadout {
        let mut statement = connection.prepare(
            "SELECT spell_name,item_name FROM item_proc_spells
             WHERE (item_id IS NOT NULL AND (item_id=?1 OR item_id=?2))
                OR item_name=?3 COLLATE NOCASE OR item_name=?4 COLLATE NOCASE",
        )?;
        let found = statement
            .query_map(
                params![primary_id, secondary_id, primary_name, secondary_name],
                |row| {
                    Ok(ProcSource {
                        spell_name: row.get(0)?,
                        item_name: row.get(1)?,
                    })
                },
            )?
            .filter_map(Result::ok)
            .filter(|source| candidates.contains(&source.spell_name.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if let [source] = found.as_slice() {
            return Ok(Some(source.clone()));
        }
    }

    if candidate_spells.len() == 1 {
        let spell = &candidate_spells[0];
        let mut statement = connection.prepare(
            "SELECT spell_name,item_name FROM item_proc_spells
             WHERE spell_name=? COLLATE NOCASE ORDER BY item_name COLLATE NOCASE",
        )?;
        let sources = statement
            .query_map([spell], |row| {
                Ok(ProcSource {
                    spell_name: row.get(0)?,
                    item_name: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if let [source] = sources.as_slice() {
            return Ok(Some(source.clone()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;
    use tempfile::tempdir;

    #[test]
    fn bundled_catalog_is_complete_and_resolves_canonical_ids() {
        let folder = tempdir().unwrap();
        let database = Database::open(folder.path().join("loot.db")).unwrap();
        assert_eq!(database.migrate().unwrap(), 37);
        let connection = database.connect().unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM item_proc_spells", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            340
        );
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source,updated_at)
                 VALUES(12980,'Tranquil Staff','test',CURRENT_TIMESTAMP)",
                [],
            )
            .unwrap();
        crate::infrastructure::database::Database::refresh_item_values(&connection).unwrap();
        reconcile(&connection).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT item_id FROM item_proc_spells
                     WHERE item_name='Tranquil Staff' AND spell_name='One Hundred Blows'",
                    [],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .unwrap(),
            Some(12980)
        );
        assert_eq!(reconcile(&connection).unwrap(), 0);
    }

    #[test]
    fn equipped_weapon_disambiguates_a_shared_proc_spell() {
        let folder = tempdir().unwrap();
        let database = Database::open(folder.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO character_weapon_loadouts(
                    character_name,captured_at,primary_weapon_name,secondary_weapon_name,source_file
                 ) VALUES('Valryte','2026-09-11 12:00:00','Frostbringer','Tranquil Staff','inventory.txt')",
                [],
            )
            .unwrap();
        let resolved = resolve_source(
            &connection,
            "Valryte",
            &["Frostbite".to_owned(), "One Hundred Blows".to_owned()],
        )
        .unwrap();
        // Both equipped weapons are candidates, so ownership remains intentionally unresolved.
        assert_eq!(resolved, None);
        let resolved = resolve_source(&connection, "Valryte", &["Frostbite".to_owned()])
            .unwrap()
            .unwrap();
        assert_eq!(resolved.item_name, "Frostbringer");
        assert_eq!(resolved.spell_name, "Frostbite");
    }
}
