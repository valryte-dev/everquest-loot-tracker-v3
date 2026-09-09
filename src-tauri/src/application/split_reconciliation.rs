use std::collections::HashSet;

use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReconciledSale {
    key: String,
    value_pp: i64,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconciliationRequest {
    sales: Vec<ReconciledSale>,
}

pub(super) fn apply(connection: &mut Connection, payload: &Value) -> Result<usize, String> {
    let request: ReconciliationRequest =
        serde_json::from_value(payload.clone()).map_err(|error| error.to_string())?;
    if request.sales.is_empty() || request.sales.len() > 250 {
        return Err("Choose between 1 and 250 matched sales.".into());
    }
    let mut keys = HashSet::new();
    for sale in &request.sales {
        if sale.value_pp < 0 {
            return Err("Sale values cannot be negative.".into());
        }
        if sale.note.len() > 500 {
            return Err("Sale notes must be 500 characters or fewer.".into());
        }
        if !keys.insert(sale.key.to_lowercase()) {
            return Err(format!(
                "Split item {} was matched more than once.",
                sale.key
            ));
        }
    }
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    for sale in &request.sales {
        super::data::complete_split(
            &transaction,
            &json!({"key":sale.key,"valuePp":sale.value_pp,"disposition":"sold","note":sale.note}),
        )?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(request.sales.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;

    fn seeded() -> (tempfile::TempDir, Database) {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO manual_split_list_items(id,item_name,looter_name) VALUES(1,'Spell: Devouring Darkness','Seller'),(2,'Peacebringer','Seller')",[]).unwrap();
        connection.execute("INSERT INTO manual_split_list_members(split_list_item_id,member_name) VALUES(1,'Seller'),(1,'Friend'),(2,'Seller'),(2,'Friend')",[]).unwrap();
        drop(connection);
        (directory, database)
    }

    #[test]
    fn reconciled_sales_move_to_pending_payouts_atomically() {
        let (_directory, database) = seeded();
        let mut connection = database.connect().unwrap();
        assert_eq!(apply(&mut connection,&json!({"sales":[{"key":"manual:1","valuePp":950,"note":"pasted"},{"key":"manual:2","valuePp":900,"note":"pasted"}]})).unwrap(),2);
        let held: i64 = connection
            .query_row("SELECT COUNT(*) FROM manual_split_list_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        let completed:(i64,i64,i64)=connection.query_row("SELECT COUNT(*),SUM(value_pp),SUM(CASE WHEN payout_status='pending' THEN 1 ELSE 0 END) FROM completed_split_items",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        let members: i64 = connection
            .query_row("SELECT COUNT(*) FROM completed_split_members", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(held, 0);
        assert_eq!(completed, (2, 1850, 2));
        assert_eq!(members, 4);
    }

    #[test]
    fn invalid_match_rolls_back_the_whole_batch() {
        let (_directory, database) = seeded();
        let mut connection = database.connect().unwrap();
        assert!(apply(
            &mut connection,
            &json!({"sales":[{"key":"manual:1","valuePp":950},{"key":"manual:999","valuePp":900}]})
        )
        .is_err());
        let held: i64 = connection
            .query_row("SELECT COUNT(*) FROM manual_split_list_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        let completed: i64 = connection
            .query_row("SELECT COUNT(*) FROM completed_split_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((held, completed), (2, 0));
    }

    #[test]
    fn duplicate_names_consume_only_the_selected_occurrences() {
        let (_directory, database) = seeded();
        let mut connection = database.connect().unwrap();
        connection.execute("INSERT INTO manual_split_list_items(id,item_name,looter_name) VALUES(3,'Spell: Devouring Darkness','Seller'),(4,'Spell: Devouring Darkness','Seller')",[]).unwrap();
        connection.execute("INSERT INTO manual_split_list_members(split_list_item_id,member_name) VALUES(3,'Seller'),(4,'Seller')",[]).unwrap();
        assert_eq!(apply(&mut connection,&json!({"sales":[{"key":"manual:1","valuePp":950,"note":"first copy"},{"key":"manual:3","valuePp":1000,"note":"second copy"}]})).unwrap(),2);
        let held: Vec<i64> = connection.prepare("SELECT id FROM manual_split_list_items WHERE item_name='Spell: Devouring Darkness' ORDER BY id").unwrap().query_map([],|row|row.get(0)).unwrap().map(Result::unwrap).collect();
        let sold: Vec<String> = connection
            .prepare("SELECT note FROM completed_split_items ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(held, vec![4]);
        assert_eq!(
            sold,
            vec!["first copy".to_owned(), "second copy".to_owned()]
        );
    }
}
