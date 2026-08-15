use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    pub location: String,
    pub item_name: String,
    pub item_id: Option<i64>,
    pub count: i64,
    pub slots: Option<i64>,
}

pub fn parse_inventory(text: &str) -> Vec<InventoryItem> {
    text.lines()
        .filter_map(|line| {
            let columns: Vec<_> = line.split('\t').map(str::trim).collect();
            if columns.len() < 3 || columns[0].eq_ignore_ascii_case("location") {
                return None;
            }
            Some(InventoryItem {
                location: columns[0].to_owned(),
                item_name: columns[1].to_owned(),
                item_id: columns.get(2).and_then(|value| value.parse().ok()),
                count: columns
                    .get(3)
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1),
                slots: columns.get(4).and_then(|value| value.parse().ok()),
            })
        })
        .collect()
}
