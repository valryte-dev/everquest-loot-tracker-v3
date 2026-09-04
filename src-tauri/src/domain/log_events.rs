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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LevelChangeKind {
    Gained,
    Lost,
}

impl LevelChangeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gained => "gained",
            Self::Lost => "lost",
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
    LevelChanged {
        happened_at: NaiveDateTime,
        level: u16,
        direction: LevelChangeKind,
    },

    PlayerDeath {
        happened_at: NaiveDateTime,
        killer_name: String,
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
    TradeOffer {
        happened_at: NaiveDateTime,
        offerer: String,
        message: String,
        item_names: Vec<String>,
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

fn gained_level() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You have gained a level! Welcome to level (?<level>\d+)!$")
            .expect("valid gained level regex")
    })
}

fn lost_level() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You have lost a level! You are now level (?<level>\d+)!$")
            .expect("valid lost level regex")
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

fn outgoing_party_chat() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You tell your party,\s*(?<message>.+)$").expect("valid outgoing party regex")
    })
}

fn outgoing_guild_chat() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You say to your guild,\s*(?<message>.+)$")
            .expect("valid outgoing guild regex")
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

fn player_death() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^You have been slain by (?<killer>.+)!$").expect("valid player death regex")
    })
}

fn trade_offer() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^\[?(?<name>[A-Za-z][A-Za-z'_-]*)\]? has offered you(?:\s+(?<message>.+))?$")
            .expect("valid trade offer regex")
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
    for (pattern, direction) in [
        (gained_level(), LevelChangeKind::Gained),
        (lost_level(), LevelChangeKind::Lost),
    ] {
        if let Some(value) = pattern.captures(body) {
            return Some(LogEvent::LevelChanged {
                happened_at,
                level: value.name("level")?.as_str().parse().ok()?,
                direction,
            });
        }
    }
    if let Some(value) = player_death().captures(body) {
        return Some(LogEvent::PlayerDeath {
            happened_at,
            killer_name: value.name("killer")?.as_str().trim().to_owned(),
        });
    }
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
    if let Some(value) = trade_offer().captures(body) {
        let message = value
            .name("message")
            .map(|value| value.as_str())
            .unwrap_or_default()
            .trim();
        let mut item_names = extract_item_links(message);
        if item_names.is_empty() {
            let plain_message = unquote(message);
            let plain_message = plain_message.trim_end_matches(['.', '!']).trim();
            let plain = plain_message
                .strip_prefix("an ")
                .or_else(|| plain_message.strip_prefix("a "))
                .unwrap_or(plain_message)
                .trim();
            if !plain.is_empty() && !plain.eq_ignore_ascii_case("nothing") {
                item_names.push(plain.to_owned());
            }
        }
        return Some(LogEvent::TradeOffer {
            happened_at,
            offerer: value["name"].to_owned(),
            message: message.to_owned(),
            item_names,
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
    for (pattern, channel) in [
        (outgoing_party_chat(), ChatChannel::Group),
        (outgoing_guild_chat(), ChatChannel::Guild),
    ] {
        if let Some(value) = pattern.captures(body) {
            let message = value.name("message")?.as_str().to_owned();
            return Some(LogEvent::LinkedItems {
                happened_at,
                speaker: active_character.to_owned(),
                channel,
                item_names: extract_item_links(&message),
                message,
            });
        }
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
    const TITANIUM_LINK_METADATA_LEN: usize = 45;
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
        if bytes.len() <= TITANIUM_LINK_METADATA_LEN
            || !bytes[..TITANIUM_LINK_METADATA_LEN]
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            continue;
        }
        // Live Titanium links put the display name immediately after the
        // 45-character metadata block. Generated social links may add spaces.
        let name = payload[TITANIUM_LINK_METADATA_LEN..].trim();
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
    fn parses_level_gains_and_losses() {
        let gained = parse_log_event(
            "[Mon Jul 27 11:46:40 2026] You have gained a level! Welcome to level 54!",
            "Tornel",
        );
        assert!(matches!(
            gained,
            Some(LogEvent::LevelChanged {
                level: 54,
                direction: LevelChangeKind::Gained,
                ..
            })
        ));

        let lost = parse_log_event(
            "[Mon Jul 27 12:46:40 2026] You have lost a level! You are now level 53!",
            "Tornel",
        );
        assert!(matches!(
            lost,
            Some(LogEvent::LevelChanged {
                level: 53,
                direction: LevelChangeKind::Lost,
                ..
            })
        ));
    }

    #[test]
    fn parses_player_death_and_preserves_the_killer_name() {
        let event = parse_log_event(
            "[Tue Sep 01 12:31:21 2026] You have been slain by Overking Bathezid!",
            "Derpscleric",
        );
        assert!(matches!(
            event,
            Some(LogEvent::PlayerDeath { killer_name, .. })
                if killer_name == "Overking Bathezid"
        ));
    }

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
    fn parses_plain_and_clickable_trade_offers() {
        let article_prefixed_item = parse_log_event(
            "[Mon Jul 27 11:46:40 2026] Tornel has offered you a A Blue Throne.",
            "Youngman",
        );
        assert!(matches!(
            article_prefixed_item,
            Some(LogEvent::TradeOffer { offerer, item_names, .. })
                if offerer == "Tornel" && item_names == vec!["A Blue Throne"]
        ));

        let plain = parse_log_event(
            "[Mon Aug 03 07:16:29 2026] [Posed] has offered you a Blue Diamond.",
            "Youngman",
        );
        assert!(matches!(
            plain,
            Some(LogEvent::TradeOffer { offerer, item_names, .. })
                if offerer == "Posed" && item_names == vec!["Blue Diamond"]
        ));

        let first = format!("\u{12}{}A Blue Crown\u{12}", "0".repeat(45));
        let second = format!("\u{12}{}Blue Diamond\u{12}", "A".repeat(45));
        let linked = parse_log_event(
            &format!("[Mon Aug 03 07:16:30 2026] Posed has offered you {first} and {second}."),
            "Youngman",
        );
        assert!(matches!(
            linked,
            Some(LogEvent::TradeOffer { offerer, item_names, .. })
                if offerer == "Posed" && item_names == vec!["A Blue Crown", "Blue Diamond"]
        ));
    }

    #[test]
    fn parses_multiple_group_item_links_and_resolves_you() {
        let first = format!("\u{12}{}A Blue Crown\u{12}", "0".repeat(45));
        let second = format!("\u{12}{}      Tears of Prexus \u{12}", "A".repeat(45));
        let line =
            format!("[Mon Aug 03 07:16:30 2026] You tell your party, 'Look: {first} / {second}'");
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
        let link = format!("\u{12}{}White Dragon Scale\u{12}", "1".repeat(45));
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

    #[test]
    fn parses_outgoing_guild_item_link() {
        let link = format!("\u{12}{}White Dragon Scale\u{12}", "1".repeat(45));
        let line =
            format!("[Mon Aug 03 07:16:31 2026] You say to your guild, 'Check this: {link}'");
        let event = parse_log_event(&line, "Youngman");
        assert!(matches!(
            event,
            Some(LogEvent::LinkedItems {
                speaker,
                channel: ChatChannel::Guild,
                item_names,
                ..
            }) if speaker == "Youngman" && item_names == vec!["White Dragon Scale"]
        ));
    }

    #[test]
    fn extracts_live_and_social_button_link_layouts() {
        let live = format!("\u{12}{}A Black Crown\u{12}", "0".repeat(45));
        let padded = format!(
            "\u{12}{}      Water Sprinkler of Nem Ankh \u{12}",
            "F".repeat(45)
        );

        assert_eq!(
            extract_item_links(&format!("'{live} / {padded}'")),
            vec!["A Black Crown", "Water Sprinkler of Nem Ankh"]
        );
    }
}
