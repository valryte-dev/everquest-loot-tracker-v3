use chrono::NaiveDateTime;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use super::merchant::MerchantAction;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupChangeKind {
    Joined,
    Left,
    Spoke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatChannel {
    Group,
    Guild,
}

impl ChatChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Group => "group",
            Self::Guild => "guild",
        }
    }
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
    GroupCleared {
        happened_at: NaiveDateTime,
    },
    MerchantListing {
        happened_at: NaiveDateTime,
        speaker: String,
        action: MerchantAction,
        message: String,
    },
    DirectTell {
        happened_at: NaiveDateTime,
        speaker: String,
        message: String,
    },
    LinkedItems {
        happened_at: NaiveDateTime,
        speaker: String,
        channel: ChatChannel,
        message: String,
        item_names: Vec<String>,
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
        Regex::new(r"^(?<name>You|[A-Za-z][A-Za-z'_-]*) tell(?:s)? the group, .+$")
            .expect("valid group tell regex")
    })
}

fn linked_chat() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(
            r"^(?<name>You|[A-Za-z][A-Za-z'_-]*) tell(?:s)? the (?<channel>group|guild),\s*(?<message>.+)$",
        )
        .expect("valid linked chat regex")
    })
}

fn removed_from_group() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You have been removed from the group\.$").expect("valid group removal regex")
    })
}

fn auction() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^(?<name>You|[A-Za-z][A-Za-z'_-]*) auction(?:s)?,\s*(?<message>.+)$")
            .expect("valid auction regex")
    })
}

fn direct_tell() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r#"^\[?(?<name>[A-Za-z][A-Za-z'_-]*)\]? tells you,\s*(?<message>.+)$"#)
            .expect("valid direct tell regex")
    })
}

fn merchant_intent() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?<action>WTS|WTB)\b").expect("valid merchant intent regex")
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
    if let Some(value) = auction().captures(body) {
        let message = unquote(value.name("message")?.as_str());
        if let Some(intent) = merchant_intent().captures(&message) {
            let action = if intent["action"].eq_ignore_ascii_case("WTS") {
                MerchantAction::Wts
            } else {
                MerchantAction::Wtb
            };
            let who = value.name("name")?.as_str();
            return Some(LogEvent::MerchantListing {
                happened_at,
                speaker: if who.eq_ignore_ascii_case("You") {
                    active_character.to_owned()
                } else {
                    who.to_owned()
                },
                action,
                message,
            });
        }
    }
    if let Some(value) = direct_tell().captures(body) {
        return Some(LogEvent::DirectTell {
            happened_at,
            speaker: value["name"].to_owned(),
            message: unquote(value.name("message")?.as_str()),
        });
    }
    if let Some(value) = linked_chat().captures(body) {
        let message = value.name("message")?.as_str().to_owned();
        let item_names = extract_item_links(&message);
        let who = value.name("name")?.as_str();
        return Some(LogEvent::LinkedItems {
            happened_at,
            speaker: if who.eq_ignore_ascii_case("You") {
                active_character.to_owned()
            } else {
                who.to_owned()
            },
            channel: if &value["channel"] == "group" {
                ChatChannel::Group
            } else {
                ChatChannel::Guild
            },
            message,
            item_names,
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
    if removed_from_group().is_match(body) {
        return Some(LogEvent::GroupCleared { happened_at });
    }
    for (pattern, change) in [
        (joined(), GroupChangeKind::Joined),
        (left(), GroupChangeKind::Left),
        (group_tell(), GroupChangeKind::Spoke),
    ] {
        if let Some(value) = pattern.captures(body) {
            let name = &value["name"];
            return Some(LogEvent::GroupChange {
                happened_at,
                character: if change == GroupChangeKind::Spoke && name.eq_ignore_ascii_case("You") {
                    active_character.to_owned()
                } else {
                    name.to_owned()
                },
                change,
            });
        }
    }
    None
}

pub fn extract_item_links(message: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut remaining = message;
    while let Some(start) = remaining.find('\u{12}') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find('\u{12}') else {
            break;
        };
        let payload = &remaining[..end];
        remaining = &remaining[end + 1..];
        let bytes = payload.as_bytes();
        if bytes.len() < 52
            || !bytes[..45].iter().all(u8::is_ascii_hexdigit)
            || bytes[45..51] != *b"      "
        {
            continue;
        }
        let name = payload[51..].trim();
        if !name.is_empty() {
            items.push(name.to_owned());
        }
    }
    items
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    let paired = (value.starts_with('\'') && value.ends_with('\''))
        || (value.starts_with('"') && value.ends_with('"'));
    if paired && value.len() >= 2 {
        value[1..value.len() - 1].trim().to_owned()
    } else {
        value.to_owned()
    }
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
    fn parses_plain_group_chat_as_an_item_candidate_and_presence_evidence() {
        let event = parse_log_event(
            "[Mon Aug 03 07:16:27 2026] Posed tells the group, 'hello'",
            "Youngman",
        );
        assert!(matches!(
            event,
            Some(LogEvent::LinkedItems {
                speaker,
                channel: ChatChannel::Group,
                message,
                item_names,
                ..
            }) if speaker == "Posed" && message == "'hello'" && item_names.is_empty()
        ));
    }

    #[test]
    fn parses_player_removal_as_a_full_group_clear() {
        let event = parse_log_event(
            "[Mon Aug 03 07:35:16 2026] You have been removed from the group.",
            "Youngman",
        );
        assert!(matches!(event, Some(LogEvent::GroupCleared { .. })));
    }

    #[test]
    fn parses_wts_and_wtb_auction_lines() {
        let wts = parse_log_event(
            "[Mon Aug 03 07:16:27 2026] Vinkledoo auctions, 'WTS This Item 1300 / That Item'",
            "Youngman",
        );
        assert!(
            matches!(wts, Some(LogEvent::MerchantListing { speaker, action: MerchantAction::Wts, message, .. }) if speaker == "Vinkledoo" && message == "WTS This Item 1300 / That Item")
        );

        let wtb = parse_log_event(
            "[Mon Aug 03 07:16:28 2026] You auction, \"WTB A Black Crown\"",
            "Youngman",
        );
        assert!(
            matches!(wtb, Some(LogEvent::MerchantListing { speaker, action: MerchantAction::Wtb, .. }) if speaker == "Youngman")
        );
    }

    #[test]
    fn parses_incoming_tells_with_optional_bracketed_name() {
        let event = parse_log_event(
            "[Mon Aug 03 07:16:29 2026] [Posed] tells you, \"I will buy that\"",
            "Youngman",
        );
        assert!(
            matches!(event, Some(LogEvent::DirectTell { speaker, message, .. }) if speaker == "Posed" && message == "I will buy that")
        );
    }

    #[test]
    fn parses_multiple_group_item_links_and_resolves_you() {
        let first = format!("\u{12}{}      A Blue Crown \u{12}", "0".repeat(45));
        let second = format!("\u{12}{}      Tears of Prexus \u{12}", "A".repeat(45));
        let line =
            format!("[Mon Aug 03 07:16:30 2026] You tell the group, 'Look: {first} / {second}'");
        let event = parse_log_event(&line, "Youngman");
        assert!(matches!(
            event,
            Some(LogEvent::LinkedItems {
                speaker,
                channel: ChatChannel::Group,
                item_names,
                ..
            }) if speaker == "Youngman"
                && item_names == vec!["A Blue Crown", "Tears of Prexus"]
        ));
    }

    #[test]
    fn parses_named_guild_item_link() {
        let link = format!("\u{12}{}      White Dragon Scale \u{12}", "1".repeat(45));
        let line = format!("[Mon Aug 03 07:16:31 2026] Posed tells the guild, '{link}'");
        let event = parse_log_event(&line, "Youngman");
        assert!(matches!(
            event,
            Some(LogEvent::LinkedItems {
                speaker,
                channel: ChatChannel::Guild,
                item_names,
                ..
            }) if speaker == "Posed" && item_names == vec!["White Dragon Scale"]
        ));
    }
}
