use chrono::NaiveDateTime;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupChangeKind {
    Joined,
    Left,
    Spoke,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LogEvent {
    Loot {
        happened_at: NaiveDateTime,
        looter: String,
        item_name: String,
    },
    MobSlain {
        happened_at: NaiveDateTime,
        mob_name: String,
        killer: Option<String>,
    },
    GroupChange {
        happened_at: NaiveDateTime,
        character: String,
        change: GroupChangeKind,
    },
}

fn envelope() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^\[(?<time>[^]]+)]\s*(?<body>.*)$").expect("valid envelope regex")
    })
}

fn loot() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(
            r"^--(?<who>You|[A-Za-z][A-Za-z'_-]*) (?:have|has) looted (?:an? )?(?<item>.+?)\.--$",
        )
        .expect("valid loot regex")
    })
}

fn local_kill() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You have slain (?<mob>.+)!$").expect("valid local kill regex")
    })
}

fn remote_kill() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^(?<mob>.+) has been slain by (?<killer>[A-Za-z][A-Za-z'_-]*)!$")
            .expect("valid remote kill regex")
    })
}

fn joined() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^(?<name>[A-Za-z][A-Za-z'_-]*) has joined the group\.$")
            .expect("valid join regex")
    })
}

fn left() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^(?<name>[A-Za-z][A-Za-z'_-]*) has left the group\.$")
            .expect("valid leave regex")
    })
}

fn group_tell() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^(?<name>[A-Za-z][A-Za-z'_-]*) tells the group, .+$")
            .expect("valid group tell regex")
    })
}

pub fn parse_log_event(line: &str, active_character: &str) -> Option<LogEvent> {
    let outer = envelope().captures(line.trim())?;
    let happened_at = NaiveDateTime::parse_from_str(&outer["time"], "%a %b %d %H:%M:%S %Y").ok()?;
    let body = outer.name("body")?.as_str();
    if let Some(value) = loot().captures(body) {
        let who = value.name("who")?.as_str();
        return Some(LogEvent::Loot {
            happened_at,
            looter: if who.eq_ignore_ascii_case("You") {
                active_character.to_owned()
            } else {
                who.to_owned()
            },
            item_name: value.name("item")?.as_str().to_owned(),
        });
    }
    if let Some(value) = local_kill().captures(body) {
        return Some(LogEvent::MobSlain {
            happened_at,
            mob_name: value["mob"].to_owned(),
            killer: Some(active_character.to_owned()),
        });
    }
    if let Some(value) = remote_kill().captures(body) {
        return Some(LogEvent::MobSlain {
            happened_at,
            mob_name: value["mob"].to_owned(),
            killer: Some(value["killer"].to_owned()),
        });
    }
    for (pattern, change) in [
        (joined(), GroupChangeKind::Joined),
        (left(), GroupChangeKind::Left),
        (group_tell(), GroupChangeKind::Spoke),
    ] {
        if let Some(value) = pattern.captures(body) {
            return Some(LogEvent::GroupChange {
                happened_at,
                character: value["name"].to_owned(),
                change,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_loot_with_character_resolution() {
        let event = parse_log_event(
            "[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--",
            "Youngman",
        );
        assert!(
            matches!(event, Some(LogEvent::Loot { ref looter, ref item_name, .. }) if looter == "Youngman" && item_name == "Tears of Prexus"),
            "{event:?}"
        );
    }

    #[test]
    fn parses_named_loot_without_article_in_item() {
        let event = parse_log_event(
            "[Mon Aug 03 07:09:18 2026] --Vinkledoo has looted a Tears of Prexus.--",
            "Youngman",
        );
        assert!(
            matches!(event, Some(LogEvent::Loot { looter, item_name, .. }) if looter == "Vinkledoo" && item_name == "Tears of Prexus")
        );
    }

    #[test]
    fn parses_group_tell_as_presence_evidence() {
        let event = parse_log_event(
            "[Mon Aug 03 07:16:27 2026] Posed tells the group, 'hello'",
            "Youngman",
        );
        assert!(
            matches!(event, Some(LogEvent::GroupChange { character, change: GroupChangeKind::Spoke, .. }) if character == "Posed")
        );
    }
}
