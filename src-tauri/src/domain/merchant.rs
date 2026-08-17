use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MerchantAction {
    Wts,
    Wtb,
}

impl MerchantAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wts => "wts",
            Self::Wtb => "wtb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogItem {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedListingItem {
    pub item_id: Option<i64>,
    pub item_name: String,
    pub asking_price_pp: Option<i64>,
}

#[derive(Debug)]
struct ItemMatch<'a> {
    start: usize,
    end: usize,
    item: &'a CatalogItem,
}

fn price() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"(?i)(?:^|[\s:=@-])(?<amount>\d[\d,]*(?:\.\d+)?)\s*(?<suffix>k|pp|p)?\b")
            .expect("valid merchant price regex")
    })
}

fn fallback_price() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"(?i)^(?<name>.+?)\s+(?:[-:=@]\s*)?(?<amount>\d[\d,]*(?:\.\d+)?)\s*(?<suffix>k|pp|p)?\s*$")
            .expect("valid fallback merchant price regex")
    })
}

fn delimiter() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"\s*(?:,|;|/|\||\s+-\s+)\s*").expect("valid merchant delimiter regex")
    })
}

pub fn parse_listing_items(message: &str, catalog: &[CatalogItem]) -> Vec<ParsedListingItem> {
    let body = listing_body(message);
    let lower = body.to_ascii_lowercase();
    let mut matches = Vec::new();

    for item in catalog {
        let needle = item.name.to_ascii_lowercase();
        if needle.is_empty() {
            continue;
        }
        let mut cursor = 0;
        while let Some(relative) = lower[cursor..].find(&needle) {
            let start = cursor + relative;
            let end = start + needle.len();
            if has_item_boundaries(&lower, start, end) {
                matches.push(ItemMatch { start, end, item });
            }
            cursor = end;
        }
    }

    matches.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| (right.end - right.start).cmp(&(left.end - left.start)))
    });
    let mut selected: Vec<ItemMatch<'_>> = Vec::new();
    for candidate in matches {
        if selected
            .iter()
            .all(|existing| candidate.end <= existing.start || candidate.start >= existing.end)
        {
            selected.push(candidate);
        }
    }
    selected.sort_by_key(|value| value.start);

    if !selected.is_empty() {
        return selected
            .iter()
            .enumerate()
            .map(|(index, matched)| {
                let next_start = selected
                    .get(index + 1)
                    .map(|next| next.start)
                    .unwrap_or(body.len());
                ParsedListingItem {
                    item_id: Some(matched.item.id),
                    item_name: matched.item.name.clone(),
                    asking_price_pp: parse_price(&body[matched.end..next_start]),
                }
            })
            .collect();
    }

    delimiter()
        .split(body)
        .filter_map(|part| parse_fallback_item(part, catalog))
        .collect()
}

fn listing_body(message: &str) -> &str {
    let trimmed = message.trim();
    for prefix in ["WTS", "WTB"] {
        if trimmed
            .get(..prefix.len())
            .is_some_and(|value| value.eq_ignore_ascii_case(prefix))
            && trimmed[prefix.len()..]
                .chars()
                .next()
                .is_none_or(|value| !value.is_ascii_alphanumeric())
        {
            return trimmed[prefix.len()..]
                .trim_start_matches(|value: char| value.is_whitespace() || ":-".contains(value));
        }
    }
    trimmed
}

fn has_item_boundaries(value: &str, start: usize, end: usize) -> bool {
    let before = value[..start].chars().next_back();
    let after = value[end..].chars().next();
    before.is_none_or(|value| !value.is_ascii_alphanumeric())
        && after.is_none_or(|value| !value.is_ascii_alphanumeric())
}

fn parse_price(value: &str) -> Option<i64> {
    let captures = price().captures(value)?;
    parse_amount(
        &captures["amount"],
        captures.name("suffix").map(|value| value.as_str()),
    )
}

fn parse_fallback_item(part: &str, catalog: &[CatalogItem]) -> Option<ParsedListingItem> {
    let part = part.trim_matches(|value: char| value.is_whitespace() || "'\".-".contains(value));
    if part.is_empty() {
        return None;
    }
    let (name, asking_price_pp) = if let Some(captures) = fallback_price().captures(part) {
        (
            captures["name"].trim().to_owned(),
            parse_amount(
                &captures["amount"],
                captures.name("suffix").map(|value| value.as_str()),
            ),
        )
    } else {
        (part.to_owned(), None)
    };
    if name.is_empty() {
        return None;
    }
    let known = catalog
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(&name));
    Some(ParsedListingItem {
        item_id: known.map(|item| item.id),
        item_name: known.map(|item| item.name.clone()).unwrap_or(name),
        asking_price_pp,
    })
}

fn parse_amount(value: &str, suffix: Option<&str>) -> Option<i64> {
    let number = value.replace(',', "").parse::<f64>().ok()?;
    let multiplier = if suffix.is_some_and(|value| value.eq_ignore_ascii_case("k")) {
        1_000.0
    } else {
        1.0
    };
    Some((number * multiplier).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<CatalogItem> {
        vec![
            CatalogItem {
                id: 1,
                name: "This Item".into(),
            },
            CatalogItem {
                id: 2,
                name: "That Item".into(),
            },
        ]
    }

    #[test]
    fn matches_catalog_items_and_optional_prices_across_delimiters() {
        let parsed = parse_listing_items("WTS This Item 1300 / That Item", &catalog());
        assert_eq!(
            parsed,
            vec![
                ParsedListingItem {
                    item_id: Some(1),
                    item_name: "This Item".into(),
                    asking_price_pp: Some(1300),
                },
                ParsedListingItem {
                    item_id: Some(2),
                    item_name: "That Item".into(),
                    asking_price_pp: None,
                },
            ]
        );
    }

    #[test]
    fn preserves_unknown_delimited_items_and_understands_k_prices() {
        let parsed = parse_listing_items("WTB Mystery Crown 1.5k, Odd Doll", &[]);
        assert_eq!(parsed[0].item_name, "Mystery Crown");
        assert_eq!(parsed[0].asking_price_pp, Some(1500));
        assert_eq!(parsed[1].item_name, "Odd Doll");
        assert_eq!(parsed[1].asking_price_pp, None);
    }
}
