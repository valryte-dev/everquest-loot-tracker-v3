use std::{collections::HashMap, sync::LazyLock};

const EMBEDDED_CATALOG: &str = include_str!("../../assets/p99-item-icons.tsv");

struct ItemIconCatalog {
    by_peq_id: HashMap<i64, (String, i64)>,
    by_name: HashMap<String, i64>,
}

static CATALOG: LazyLock<ItemIconCatalog> = LazyLock::new(|| {
    let mut by_peq_id = HashMap::new();
    let mut by_name = HashMap::new();
    for line in EMBEDDED_CATALOG
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.splitn(4, '\t');
        let _planner_id = fields.next();
        let peq_id = fields.next().and_then(|value| value.parse::<i64>().ok());
        let name = fields.next().unwrap_or_default().trim();
        let icon_id = fields.next().and_then(|value| value.parse::<i64>().ok());
        let Some(icon_id) = icon_id.filter(|value| *value > 0) else {
            continue;
        };
        if let Some(peq_id) = peq_id.filter(|value| *value > 0) {
            by_peq_id
                .entry(peq_id)
                .or_insert_with(|| (name.to_lowercase(), icon_id));
        }
        if !name.is_empty() {
            by_name.entry(name.to_lowercase()).or_insert(icon_id);
        }
    }
    ItemIconCatalog { by_peq_id, by_name }
});

pub fn icon_id(item_id: Option<i64>, item_name: &str) -> Option<i64> {
    let normalized_name = item_name.trim().to_lowercase();
    item_id
        .and_then(|id| CATALOG.by_peq_id.get(&id))
        .filter(|(catalog_name, _)| catalog_name == &normalized_name)
        .map(|(_, icon_id)| *icon_id)
        .or_else(|| CATALOG.by_name.get(&normalized_name).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_inventory_peq_ids_and_name_fallbacks() {
        assert_eq!(icon_id(Some(10_383), "Rod of Oblations"), Some(822));
        assert_eq!(icon_id(None, "Tranquil Staff"), Some(601));
        assert_eq!(icon_id(Some(-1), "A Blue Crown"), Some(653));
    }

    #[test]
    fn rejects_cross_catalog_id_collisions_before_falling_back_to_name() {
        assert_eq!(icon_id(Some(2656), "Staff of the Serpent"), Some(2865));
    }
    #[test]
    fn unknown_items_have_no_invented_icon() {
        assert_eq!(icon_id(Some(999_999_999), "Not A Real Item"), None);
    }
}
