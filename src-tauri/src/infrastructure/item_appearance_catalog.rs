use std::{collections::HashMap, sync::LazyLock};

const EMBEDDED_CATALOG: &str = include_str!("../../assets/p99-item-appearance.tsv");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemAppearance {
    pub material: i64,
    pub id_file: Option<String>,
    pub color: i64,
    pub item_type: Option<i64>,
}

struct ItemAppearanceCatalog {
    by_peq_id: HashMap<i64, ItemAppearance>,
    by_name: HashMap<String, ItemAppearance>,
}

static CATALOG: LazyLock<ItemAppearanceCatalog> = LazyLock::new(|| {
    let mut by_peq_id = HashMap::new();
    let mut by_name = HashMap::new();
    for line in EMBEDDED_CATALOG
        .lines()
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let mut fields = line.splitn(7, '\t');
        let _planner_id = fields.next();
        let peq_id = fields.next().and_then(|value| value.parse::<i64>().ok());
        let name = fields.next().unwrap_or_default().trim();
        let material = fields
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let id_file = fields
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let color = fields
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let item_type = normalized_item_type(
            id_file.as_deref(),
            fields.next().and_then(|value| value.parse().ok()),
        );
        let appearance = ItemAppearance {
            material,
            id_file,
            color,
            item_type,
        };
        if let Some(peq_id) = peq_id.filter(|value| *value > 0) {
            by_peq_id
                .entry(peq_id)
                .or_insert_with(|| appearance.clone());
        }
        if !name.is_empty() {
            by_name.entry(name.to_lowercase()).or_insert(appearance);
        }
    }
    ItemAppearanceCatalog { by_peq_id, by_name }
});

fn normalized_item_type(id_file: Option<&str>, source_item_type: Option<i64>) -> Option<i64> {
    let shield_model = id_file
        .and_then(|value| value.trim().strip_prefix("IT"))
        .and_then(|value| value.parse::<u16>().ok())
        .is_some_and(|model| matches!(model, 200..=223 | 226 | 228));
    if shield_model {
        // The P99 database commonly exports classic shield visuals as the
        // unknown item type 255. EQSage attaches these model families to the
        // dedicated shield point, including orb-style IT210 offhands.
        Some(8)
    } else {
        source_item_type
    }
}

pub fn appearance(item_id: Option<i64>, item_name: &str) -> Option<ItemAppearance> {
    item_id
        .and_then(|id| CATALOG.by_peq_id.get(&id).cloned())
        .or_else(|| {
            CATALOG
                .by_name
                .get(&item_name.trim().to_lowercase())
                .cloned()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_weapon_models_by_master_id_and_name() {
        assert_eq!(
            appearance(Some(10_383), "Rod of Oblations")
                .unwrap()
                .id_file
                .as_deref(),
            Some("IT47")
        );
        assert!(appearance(None, "Tranquil Staff")
            .unwrap()
            .id_file
            .is_some());
    }

    #[test]
    fn unknown_items_have_no_invented_appearance() {
        assert_eq!(appearance(Some(999_999_999), "Not A Real Item"), None);
    }

    #[test]
    fn exposes_authoritative_item_types_for_renderer_attachment() {
        assert_eq!(
            appearance(Some(6639), "Tranquil Staff").unwrap().item_type,
            Some(4)
        );
        assert_eq!(
            appearance(Some(25098), "Orb of the Infinite Void")
                .unwrap()
                .item_type,
            Some(8)
        );
        assert_eq!(
            appearance(Some(1044), "Velium Round Shield")
                .unwrap()
                .item_type,
            Some(8)
        );
    }
}
