use super::combat_metrics::{
    insert_or_get_damage_encounter, record_inferred_dot_tick, record_inferred_proc_damage,
};
use crate::{
    domain::log_events::{parse_envelope, LogEvent},
    infrastructure::{
        proc_catalog,
        spell_catalog::{CombatSpellProfile, SpellCatalog},
    },
};
use chrono::{Duration, Local, NaiveDateTime};
use regex::Regex;
use rusqlite::{params, OptionalExtension};
use std::time::{Duration as StdDuration, Instant};

const GLOW_WINDOW_SECONDS: i64 = 3;
const CAST_FALLBACK_WINDOW_SECONDS: i64 = 15;
const CAST_RESOLUTION_TOLERANCE_MILLISECONDS: i64 = 2_000;
const UNIDENTIFIED_PLAYER: &str = "Unidentified player";

struct CompiledProfile {
    profile: CombatSpellProfile,
    landing: Regex,
}

#[derive(Clone)]
struct RecentGlow {
    happened_at: NaiveDateTime,
    owner_name: String,
    item_name: String,
}
#[derive(Clone)]
struct RecentCast {
    happened_at: NaiveDateTime,
    expected_at: Option<NaiveDateTime>,
    spell_name: String,
    explicit_proc_caster: Option<String>,
}
#[derive(Clone)]
struct RecentNonMelee {
    source: String,
    happened_at: NaiveDateTime,
    target_name: String,
    amount: u64,
}

#[derive(Clone)]
struct PendingProc {
    source: String,
    source_offset: i64,
    happened_at: NaiveDateTime,
    target_name: String,
    spell_name: String,
    source_name: Option<String>,
    direct_damage: Option<u64>,
    preceding_non_melee: Option<RecentNonMelee>,
    damage_per_tick: Option<u64>,
    tick_interval_seconds: u32,
    tick_count: Option<u32>,
    candidate_caster: Option<String>,
    allow_observed_caster: bool,
}

pub struct DotTracker {
    catalog: SpellCatalog,
    profiles: Vec<CompiledProfile>,
    loaded_at: Option<Instant>,
    recent_glow: Option<RecentGlow>,
    recent_cast: Option<RecentCast>,
    recent_non_melee: Option<RecentNonMelee>,
    pending_proc: Option<PendingProc>,
}

impl DotTracker {
    pub fn new(catalog: SpellCatalog) -> Self {
        Self {
            catalog,
            profiles: Vec::new(),
            loaded_at: None,
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        }
    }

    fn refresh_profiles(&mut self) {
        if self
            .loaded_at
            .is_some_and(|loaded| loaded.elapsed() < StdDuration::from_secs(30))
        {
            return;
        }
        if let Ok(profiles) = self.catalog.combat_profiles() {
            self.profiles = profiles.into_iter().filter_map(compile_profile).collect();
            self.loaded_at = Some(Instant::now());
        }
    }

    pub fn process_line(
        &mut self,
        connection: &rusqlite::Connection,
        source: &str,
        source_offset: i64,
        raw: &str,
        character: &str,
        event: Option<&LogEvent>,
    ) -> Result<usize, String> {
        let Some((happened_at, body)) = parse_envelope(raw) else {
            return Ok(0);
        };
        self.refresh_profiles();
        let preceding_non_melee = self.recent_non_melee.take();
        // Named proc-caster clues are intentionally single-use and adjacent-only. Carry the
        // clue into this line, then clear it before processing anything else so an unrelated
        // intervening log line cannot cause a later landing to be misattributed.
        let explicit_proc_clue = self
            .recent_cast
            .as_ref()
            .filter(|cast| cast.explicit_proc_caster.is_some())
            .cloned();
        if explicit_proc_clue.is_some() {
            self.recent_cast = None;
        }
        let mut changed = 0;
        if let Some(pending) = self.pending_proc.take() {
            if pending.source == source {
                let combat_actor = match event {
                    Some(LogEvent::CombatAttempt {
                        attacker_name,
                        mob_name,
                        ..
                    }) if mob_name.eq_ignore_ascii_case(&pending.target_name) => {
                        Some(attacker_name.as_str())
                    }
                    Some(LogEvent::Damage {
                        attacker_name,
                        mob_name,
                        damage_type,
                        ..
                    }) if damage_type.as_str() == "melee"
                        && mob_name.eq_ignore_ascii_case(&pending.target_name) =>
                    {
                        Some(attacker_name.as_str())
                    }
                    Some(LogEvent::ObservedMelee {
                        subject_name,
                        target_name,
                        ..
                    }) if target_name.eq_ignore_ascii_case(&pending.target_name) => {
                        Some(subject_name.as_str())
                    }
                    _ => None,
                };
                if let Some(combat_actor) = combat_actor {
                    let logged_proc_damage_confirms_local = pending.allow_observed_caster
                        && pending
                            .preceding_non_melee
                            .as_ref()
                            .is_some_and(|damage| pending.direct_damage == Some(damage.amount));
                    let caster = if logged_proc_damage_confirms_local
                        || combat_actor.eq_ignore_ascii_case(character)
                    {
                        character
                    } else if pending.allow_observed_caster
                        || pending
                            .candidate_caster
                            .as_ref()
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(combat_actor))
                    {
                        combat_actor
                    } else {
                        UNIDENTIFIED_PLAYER
                    };
                    changed += record_proc_occurrence(
                        connection,
                        &pending,
                        source_offset,
                        character,
                        caster,
                    )?;
                }
            }
        }
        if let Some(LogEvent::Damage {
            mob_name,
            attack,
            damage_type,
            ..
        }) = event
        {
            if damage_type.as_str() == "spell"
                && self
                    .profiles
                    .iter()
                    .any(|profile| profile.profile.spell_name.eq_ignore_ascii_case(attack))
            {
                changed += disable_duplicate_inference(connection, source, mob_name, attack)?;
            }
        }
        changed += self.flush_due(connection, happened_at)?;

        if let Some(LogEvent::ItemGlow {
            owner_name,
            item_name,
            ..
        }) = event
        {
            self.recent_glow = Some(RecentGlow {
                happened_at,
                owner_name: owner_name.clone().unwrap_or_else(|| character.to_owned()),
                item_name: item_name.clone(),
            });
        }
        if let Some(LogEvent::SpellCastStarted { spell_name, .. }) = event {
            let expected_at = self
                .profiles
                .iter()
                .find(|profile| profile.profile.spell_name.eq_ignore_ascii_case(spell_name))
                .and_then(|profile| profile.profile.casting_time_seconds)
                .map(|seconds| {
                    happened_at + Duration::milliseconds((seconds * 1_000.0).round() as i64)
                });
            self.recent_cast = Some(RecentCast {
                happened_at,
                expected_at,
                spell_name: spell_name.clone(),
                explicit_proc_caster: None,
            });
        }
        if let Some((caster_name, spell_name)) = explicit_proc_caster_clue(body) {
            self.recent_cast = Some(RecentCast {
                happened_at,
                expected_at: Some(happened_at),
                spell_name,
                explicit_proc_caster: Some(caster_name),
            });
        }
        if matches!(event, Some(LogEvent::SpellInterrupted { .. })) {
            self.recent_cast = None;
            self.recent_glow = None;
        }
        if let Some(LogEvent::SpellResisted { spell_name, .. }) = event {
            if self
                .recent_cast
                .as_ref()
                .is_some_and(|cast| cast_resolution_matches(cast, spell_name, happened_at))
            {
                self.recent_cast = None;
            }
        }

        let matches = self
            .profiles
            .iter()
            .filter_map(|compiled| {
                let captures = compiled.landing.captures(body)?;
                Some((
                    compiled.profile.clone(),
                    captures.name("target")?.as_str().trim().to_owned(),
                ))
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            let glow_caster = self.recent_glow.as_ref().filter(|glow| {
                glow.owner_name.eq_ignore_ascii_case(character)
                    && (0..=GLOW_WINDOW_SECONDS).contains(
                        &happened_at
                            .signed_duration_since(glow.happened_at)
                            .num_seconds(),
                    )
            });
            let candidate_names = matches
                .iter()
                .map(|(profile, _)| profile.spell_name.clone())
                .collect::<Vec<_>>();
            let direct_cast_name = self.recent_cast.as_ref().and_then(|cast| {
                if cast.explicit_proc_caster.is_some() {
                    return None;
                }
                matches
                    .iter()
                    .find(|(profile, _)| {
                        cast_resolution_matches(cast, &profile.spell_name, happened_at)
                    })
                    .map(|(profile, _)| profile.spell_name.clone())
            });
            let clicked = glow_caster.and_then(|glow| {
                proc_catalog::resolve_item_spell(connection, &glow.item_name, &candidate_names)
                    .ok()
                    .flatten()
            });
            let equipped = if preceding_non_melee.is_some() {
                proc_catalog::resolve_source(connection, character, &candidate_names)
                    .ok()
                    .flatten()
            } else {
                None
            };
            let selected_name = direct_cast_name
                .as_deref()
                .or_else(|| clicked.as_ref().map(|value| value.spell_name.as_str()))
                .or_else(|| equipped.as_ref().map(|value| value.spell_name.as_str()));
            let selected = selected_name
                .and_then(|name| {
                    matches
                        .iter()
                        .find(|(profile, _)| profile.spell_name.eq_ignore_ascii_case(name))
                })
                .cloned()
                .or_else(|| (matches.len() == 1).then(|| matches[0].clone()))
                .or_else(|| unidentified_root_proc(body));

            if let Some((profile, target)) = selected {
                let direct_cast = direct_cast_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(&profile.spell_name));
                let has_glow = glow_caster.is_some();
                let explicit_proc_caster = explicit_proc_clue.as_ref().and_then(|clue| {
                    let age = happened_at
                        .signed_duration_since(clue.happened_at)
                        .num_seconds();
                    (clue.spell_name.eq_ignore_ascii_case(&profile.spell_name) && age == 0)
                        .then(|| clue.explicit_proc_caster.clone())
                        .flatten()
                });
                if has_glow || direct_cast {
                    let (attribution_method, source_kind, source_name) =
                        if let Some(glow) = glow_caster {
                            ("item_glow", "item_click", Some(glow.item_name.as_str()))
                        } else {
                            ("direct_cast", "direct", None)
                        };
                    changed += record_confirmed_landing(
                        connection,
                        source,
                        source_offset,
                        character,
                        happened_at,
                        &target,
                        character,
                        attribution_method,
                        &profile.spell_name,
                        source_kind,
                        source_name,
                        profile.damage_per_tick,
                        profile.tick_interval_seconds,
                        profile.tick_count,
                    )?;
                } else if let Some(caster) = explicit_proc_caster {
                    let source_name = proc_catalog::resolve_source(
                        connection,
                        character,
                        std::slice::from_ref(&profile.spell_name),
                    )
                    .ok()
                    .flatten()
                    .map(|value| value.item_name)
                    .or(profile.observed_source_name.clone());
                    self.pending_proc = Some(PendingProc {
                        source: source.to_owned(),
                        source_offset,
                        happened_at,
                        target_name: target,
                        spell_name: profile.spell_name,
                        source_name,
                        direct_damage: profile.direct_damage,
                        preceding_non_melee: None,
                        damage_per_tick: profile.damage_per_tick,
                        tick_interval_seconds: profile.tick_interval_seconds,
                        tick_count: profile.tick_count,
                        candidate_caster: Some(caster),
                        allow_observed_caster: false,
                    });
                    self.recent_cast = None;
                } else {
                    let matched_damage = preceding_non_melee.clone().filter(|damage| {
                        damage.source == source
                            && damage.target_name.eq_ignore_ascii_case(&target)
                            && happened_at >= damage.happened_at
                    });
                    if profile.observed_source_kind.as_deref() == Some("item_click_only") {
                        changed += record_unattributed_landing(
                            connection,
                            source,
                            source_offset,
                            character,
                            happened_at,
                            &target,
                            &profile.spell_name,
                            profile.observed_source_name.as_deref(),
                        )?;
                    } else {
                        let resolved_source = equipped
                            .as_ref()
                            .or(clicked.as_ref())
                            .map(|value| value.item_name.clone())
                            .or_else(|| {
                                proc_catalog::resolve_source(
                                    connection,
                                    character,
                                    std::slice::from_ref(&profile.spell_name),
                                )
                                .ok()
                                .flatten()
                                .map(|value| value.item_name)
                            })
                            .or(profile.observed_source_name.clone());
                        self.pending_proc = Some(PendingProc {
                            source: source.to_owned(),
                            source_offset,
                            happened_at,
                            target_name: target,
                            spell_name: profile.spell_name,
                            source_name: resolved_source,
                            direct_damage: profile.direct_damage,
                            preceding_non_melee: matched_damage,
                            damage_per_tick: profile.damage_per_tick,
                            tick_interval_seconds: profile.tick_interval_seconds,
                            tick_count: profile.tick_count,
                            candidate_caster: None,
                            allow_observed_caster: profile.observed_source_kind.as_deref()
                                == Some("proc_only")
                                || is_root_entwinement_landing(body),
                        });
                    }
                }
                self.recent_glow = None;
                if direct_cast {
                    self.recent_cast = None;
                }
            } else {
                let same_target =
                    matches
                        .first()
                        .map(|(_, target)| target.clone())
                        .filter(|target| {
                            matches
                                .iter()
                                .all(|(_, other)| other.eq_ignore_ascii_case(target))
                        });
                let proc_candidate = candidate_names
                    .iter()
                    .any(|name| proc_catalog::is_known_proc(connection, name).unwrap_or(false));
                let includes_item_click_only = matches.iter().any(|(profile, _)| {
                    profile.observed_source_kind.as_deref() == Some("item_click_only")
                });
                if let Some(target) =
                    same_target.filter(|_| proc_candidate && !includes_item_click_only)
                {
                    let matched_damage = preceding_non_melee.filter(|damage| {
                        damage.source == source
                            && damage.target_name.eq_ignore_ascii_case(&target)
                            && happened_at >= damage.happened_at
                    });
                    self.pending_proc = Some(PendingProc {
                        source: source.to_owned(),
                        source_offset,
                        happened_at,
                        target_name: target,
                        spell_name: "Unidentified direct proc".to_owned(),
                        source_name: None,
                        direct_damage: None,
                        preceding_non_melee: matched_damage,
                        damage_per_tick: None,
                        tick_interval_seconds: 6,
                        tick_count: None,
                        candidate_caster: None,
                        allow_observed_caster: false,
                    });
                }
            }
        }

        if let Some(LogEvent::Damage {
            mob_name,
            attack,
            amount,
            damage_type,
            ..
        }) = event
        {
            if damage_type.as_str() == "spell" && attack.eq_ignore_ascii_case("non-melee") {
                self.recent_non_melee = Some(RecentNonMelee {
                    source: source.to_owned(),
                    happened_at,
                    target_name: mob_name.clone(),
                    amount: *amount,
                });
            }
        }

        match event {
            Some(LogEvent::MobSlain { mob_name, .. }) => {
                changed += finish_target(connection, source, mob_name, happened_at, "slain")?;
            }
            Some(LogEvent::PlayerDeath { .. }) => {
                changed += finish_character(connection, source, character, happened_at)?;
            }
            _ => {}
        }
        Ok(changed)
    }

    pub fn flush_live(&mut self, connection: &rusqlite::Connection) -> Result<usize, String> {
        self.refresh_profiles();
        self.flush_due(connection, Local::now().naive_local())
    }
    pub(super) fn flush_through(
        &mut self,
        connection: &rusqlite::Connection,
        through: NaiveDateTime,
    ) -> Result<usize, String> {
        self.refresh_profiles();
        self.flush_due(connection, through)
    }

    fn flush_due(
        &self,
        connection: &rusqlite::Connection,
        now: NaiveDateTime,
    ) -> Result<usize, String> {
        let mut statement = connection
            .prepare(
                "SELECT id,encounter_id,spell_name,caster_name,landed_at,damage_per_tick,
                    tick_interval_seconds,total_ticks,ticks_applied
             FROM dot_applications
             WHERE status='active' AND inference_enabled=1 AND landed_at<=?",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([now.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, u64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        drop(statement);
        let mut changed = 0;
        for (id, encounter_id, spell, caster, landed, damage, interval, total, applied) in rows {
            let landed = NaiveDateTime::parse_from_str(&landed, "%Y-%m-%d %H:%M:%S")
                .map_err(|error| error.to_string())?;
            let due = (now.signed_duration_since(landed).num_seconds() / interval).clamp(0, total);
            for tick in (applied + 1)..=due {
                let tick_at = landed + Duration::seconds(interval * tick);
                changed += record_inferred_dot_tick(
                    connection,
                    encounter_id,
                    id,
                    tick,
                    tick_at,
                    &caster,
                    &spell,
                    damage,
                )?;
            }
            if due > applied {
                let status = if due >= total { "expired" } else { "active" };
                connection.execute(
                    "UPDATE dot_applications SET ticks_applied=?,status=?,ended_at=CASE WHEN ?='expired' THEN expires_at ELSE ended_at END WHERE id=?",
                    params![due,status,status,id],
                ).map_err(|error| error.to_string())?;
                changed += 1;
            }
        }
        Ok(changed)
    }
}

fn cast_resolution_matches(
    cast: &RecentCast,
    spell_name: &str,
    happened_at: NaiveDateTime,
) -> bool {
    if !cast.spell_name.eq_ignore_ascii_case(spell_name) || happened_at < cast.happened_at {
        return false;
    }
    match cast.expected_at {
        Some(expected_at) => {
            happened_at
                .signed_duration_since(expected_at)
                .num_milliseconds()
                .abs()
                <= CAST_RESOLUTION_TOLERANCE_MILLISECONDS
        }
        None => (0..=CAST_FALLBACK_WINDOW_SECONDS).contains(
            &happened_at
                .signed_duration_since(cast.happened_at)
                .num_seconds(),
        ),
    }
}

fn explicit_proc_caster_clue(body: &str) -> Option<(String, String)> {
    static ESSENCE_TAP_CLUE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let captures = ESSENCE_TAP_CLUE
        .get_or_init(|| {
            Regex::new(
                r#"(?i)^(?P<caster>[A-Za-z][A-Za-z'\x60-]*) says ['"]\s*Ahhh,\s*I feel much better now\\?\.{3}\s*['"]$"#,
            )
            .expect("Essence Tap caster clue must compile")
        })
        .captures(body.trim())?;
    Some((
        captures.name("caster")?.as_str().to_owned(),
        "Essence Tap".to_owned(),
    ))
}

fn is_root_entwinement_landing(body: &str) -> bool {
    root_entwinement_target(body).is_some()
}

fn root_entwinement_target(body: &str) -> Option<String> {
    static ROOT_LANDING: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    ROOT_LANDING
        .get_or_init(|| {
            Regex::new(r"(?i)^(?P<target>.+)'s feet become entwined\.$")
                .expect("root landing must compile")
        })
        .captures(body.trim())?
        .name("target")
        .map(|value| value.as_str().trim().to_owned())
}

fn unidentified_root_proc(body: &str) -> Option<(CombatSpellProfile, String)> {
    Some((
        CombatSpellProfile {
            spell_name: "Unidentified root proc".to_owned(),
            cast_on_other: "Someone's feet become entwined.".to_owned(),
            damage_kind: "non_damage".to_owned(),
            direct_damage: None,
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: None,
            observed_source_kind: Some("proc_only".to_owned()),
            observed_source_name: None,
        },
        root_entwinement_target(body)?,
    ))
}

fn compile_profile(profile: CombatSpellProfile) -> Option<CompiledProfile> {
    let escaped = regex::escape(profile.cast_on_other.trim());
    let someone = Regex::new("(?i)someone").expect("valid placeholder regex");
    if !someone.is_match(&escaped) {
        return None;
    }
    let source = someone
        .replace(&escaped, "(?P<target>.+?)")
        .replace(" 's", "'s")
        .replace("'s", r"\s*'s");
    Regex::new(&format!("(?i)^{source}$"))
        .ok()
        .map(|landing| CompiledProfile { profile, landing })
}

#[allow(clippy::too_many_arguments)]
fn land_dot(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    character: &str,
    happened_at: NaiveDateTime,
    target: &str,
    caster: &str,
    attribution_method: &str,
    spell_name: &str,
    damage_per_tick: u64,
    tick_interval_seconds: u32,
    tick_count: u32,
) -> Result<(usize, i64), String> {
    let encounter_id = ensure_encounter(
        connection,
        source,
        source_offset,
        character,
        happened_at,
        target,
    )?;
    connection.execute(
        "UPDATE dot_applications SET status='refreshed',ended_at=?
         WHERE encounter_id=? AND target_name=? COLLATE NOCASE AND spell_name=? COLLATE NOCASE AND status='active'",
        params![happened_at.to_string(),encounter_id,target,spell_name],
    ).map_err(|error| error.to_string())?;
    let expires_at =
        happened_at + Duration::seconds(i64::from(tick_interval_seconds) * i64::from(tick_count));
    let inserted = connection
        .execute(
            "INSERT OR IGNORE INTO dot_applications(
            encounter_id,spell_name,target_name,caster_name,attribution_method,landed_at,expires_at,
            damage_per_tick,tick_interval_seconds,total_ticks,source_file,source_offset
         ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                spell_name,
                target,
                caster,
                attribution_method,
                happened_at.to_string(),
                expires_at.to_string(),
                damage_per_tick,
                tick_interval_seconds,
                tick_count,
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())?;
    let application_id = connection
        .query_row(
            "SELECT id FROM dot_applications
             WHERE source_file=? AND source_offset=? AND spell_name=? COLLATE NOCASE",
            params![source, source_offset, spell_name],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?;
    Ok((inserted, application_id))
}

fn ensure_encounter(
    connection: &rusqlite::Connection,
    source: &str,
    offset: i64,
    character: &str,
    at: NaiveDateTime,
    mob: &str,
) -> Result<i64, String> {
    // A backlog can replay a landing after its encounter has already closed.
    // The original file offset is the durable idempotency key, so reuse that
    // encounter regardless of lifecycle state before attempting an insert.
    if let Some(id) = connection
        .query_row(
            "SELECT id FROM damage_encounters
             WHERE source_file=? AND first_source_offset=?",
            params![source, offset],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    {
        return Ok(id);
    }
    if let Some(id) = connection.query_row(
        "SELECT id FROM damage_encounters WHERE source_file=? AND character_name=? COLLATE NOCASE AND mob_name=? COLLATE NOCASE AND outcome='active' ORDER BY last_source_offset DESC LIMIT 1",
        params![source,character,mob], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())? { return Ok(id); }
    insert_or_get_damage_encounter(connection, source, offset, character, at, mob)
}

#[allow(clippy::too_many_arguments)]
fn record_unattributed_landing(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    character: &str,
    happened_at: NaiveDateTime,
    target: &str,
    spell_name: &str,
    source_name: Option<&str>,
) -> Result<usize, String> {
    let encounter_id = ensure_encounter(
        connection,
        source,
        source_offset,
        character,
        happened_at,
        target,
    )?;
    record_spell_activity(
        connection,
        encounter_id,
        source,
        source_offset,
        happened_at,
        target,
        "Unattributed",
        spell_name,
        "unknown",
        source_name,
    )
}

#[allow(clippy::too_many_arguments)]
fn record_confirmed_landing(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    character: &str,
    happened_at: NaiveDateTime,
    target: &str,
    caster: &str,
    attribution_method: &str,
    spell_name: &str,
    source_kind: &str,
    source_name: Option<&str>,
    damage_per_tick: Option<u64>,
    tick_interval_seconds: u32,
    tick_count: Option<u32>,
) -> Result<usize, String> {
    let mut changed = 0;
    let dot_application_id =
        if let (Some(damage_per_tick), Some(tick_count)) = (damage_per_tick, tick_count) {
            let (inserted, application_id) = land_dot(
                connection,
                source,
                source_offset,
                character,
                happened_at,
                target,
                caster,
                attribution_method,
                spell_name,
                damage_per_tick,
                tick_interval_seconds,
                tick_count,
            )?;
            changed += inserted;
            Some(application_id)
        } else {
            None
        };
    let encounter_id = if let Some(application_id) = dot_application_id {
        connection
            .query_row(
                "SELECT encounter_id FROM dot_applications WHERE id=?",
                [application_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?
    } else {
        ensure_encounter(
            connection,
            source,
            source_offset,
            character,
            happened_at,
            target,
        )?
    };
    changed += record_spell_activity(
        connection,
        encounter_id,
        source,
        source_offset,
        happened_at,
        target,
        caster,
        spell_name,
        source_kind,
        source_name,
    )?;
    Ok(changed)
}
#[allow(clippy::too_many_arguments)]
fn record_spell_activity(
    connection: &rusqlite::Connection,
    encounter_id: i64,
    source: &str,
    source_offset: i64,
    happened_at: NaiveDateTime,
    target: &str,
    caster: &str,
    spell_name: &str,
    source_kind: &str,
    source_name: Option<&str>,
) -> Result<usize, String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO combat_spell_activity(
                encounter_id,spell_name,target_name,caster_name,source_kind,source_name,
                happened_at,source_file,landing_source_offset
             ) VALUES(?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                spell_name,
                target,
                caster,
                source_kind,
                source_name,
                happened_at.to_string(),
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())
}

fn record_proc_occurrence(
    connection: &rusqlite::Connection,
    pending: &PendingProc,
    attribution_source_offset: i64,
    character: &str,
    caster: &str,
) -> Result<usize, String> {
    let already_recorded = connection
        .query_row(
            "SELECT 1 FROM proc_occurrences
             WHERE source_file=? AND landing_source_offset=? AND spell_name=? COLLATE NOCASE",
            params![pending.source, pending.source_offset, pending.spell_name],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if already_recorded {
        return Ok(0);
    }

    let dot_application_id = if let (Some(damage_per_tick), Some(tick_count)) =
        (pending.damage_per_tick, pending.tick_count)
    {
        let (_, application_id) = land_dot(
            connection,
            &pending.source,
            pending.source_offset,
            character,
            pending.happened_at,
            &pending.target_name,
            caster,
            "proc",
            &pending.spell_name,
            damage_per_tick,
            pending.tick_interval_seconds,
            tick_count,
        )?;
        connection
            .execute(
                "UPDATE dot_applications SET caster_name=?,attribution_method='proc'
                 WHERE id=?",
                params![caster, application_id],
            )
            .map_err(|error| error.to_string())?;
        Some(application_id)
    } else {
        None
    };
    let encounter_id = if let Some(application_id) = dot_application_id {
        connection
            .query_row(
                "SELECT encounter_id FROM dot_applications WHERE id=?",
                [application_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?
    } else {
        ensure_encounter(
            connection,
            &pending.source,
            pending.source_offset,
            character,
            pending.happened_at,
            &pending.target_name,
        )?
    };
    let direct_damage = pending
        .preceding_non_melee
        .as_ref()
        .map(|damage| damage.amount)
        .or(pending.direct_damage)
        .unwrap_or(0);
    let direct_damage_at = pending
        .preceding_non_melee
        .as_ref()
        .map(|damage| damage.happened_at)
        .unwrap_or(pending.happened_at);
    let inserted = connection
        .execute(
            "INSERT OR IGNORE INTO proc_occurrences(
                encounter_id,dot_application_id,spell_name,target_name,caster_name,happened_at,
                direct_damage,source_file,landing_source_offset,attribution_source_offset
             ) VALUES(?,?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                dot_application_id,
                pending.spell_name,
                pending.target_name,
                caster,
                pending.happened_at.to_string(),
                direct_damage as i64,
                pending.source,
                pending.source_offset,
                attribution_source_offset
            ],
        )
        .map_err(|error| error.to_string())?;
    if inserted == 0 {
        return Ok(0);
    }
    let occurrence_id = connection.last_insert_rowid();
    let mut changed = inserted;
    changed += record_spell_activity(
        connection,
        encounter_id,
        &pending.source,
        pending.source_offset,
        pending.happened_at,
        &pending.target_name,
        caster,
        &pending.spell_name,
        "proc",
        pending.source_name.as_deref(),
    )?;
    connection
        .execute(
            "UPDATE combat_spell_activity
             SET caster_name=?,source_kind='proc',source_name=?,attribution_source_offset=?
             WHERE source_file=? AND landing_source_offset=? AND spell_name=? COLLATE NOCASE",
            params![
                caster,
                pending.source_name,
                attribution_source_offset,
                pending.source,
                pending.source_offset,
                pending.spell_name
            ],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO damage_spell_summaries(
                encounter_id,caster_name,spell_name,proc_count,direct_proc_damage
             ) VALUES(?,?,?,1,?)
             ON CONFLICT(encounter_id,caster_name,spell_name) DO UPDATE SET
                proc_count=proc_count+1,
                direct_proc_damage=direct_proc_damage+excluded.direct_proc_damage",
            params![
                encounter_id,
                caster,
                pending.spell_name,
                direct_damage as i64
            ],
        )
        .map_err(|error| error.to_string())?;
    if direct_damage > 0 {
        changed += record_inferred_proc_damage(
            connection,
            encounter_id,
            occurrence_id,
            direct_damage_at,
            caster,
            &pending.spell_name,
            direct_damage,
        )?;
        let damage_event_id = connection
            .query_row(
                "SELECT id FROM damage_events WHERE source_file=? AND source_offset=1",
                [format!("proc://{occurrence_id}")],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE proc_occurrences SET damage_event_id=? WHERE id=?",
                params![damage_event_id, occurrence_id],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(changed)
}
fn disable_duplicate_inference(
    connection: &rusqlite::Connection,
    source: &str,
    mob: &str,
    spell: &str,
) -> Result<usize, String> {
    connection.execute(
        "UPDATE dot_applications SET inference_enabled=0 WHERE id=(SELECT id FROM dot_applications
         WHERE source_file=? AND target_name=? COLLATE NOCASE AND spell_name=? COLLATE NOCASE AND status='active'
         ORDER BY landed_at DESC,id DESC LIMIT 1)",
        params![source,mob,spell],
    ).map_err(|error| error.to_string())
}

fn finish_target(
    connection: &rusqlite::Connection,
    source: &str,
    mob: &str,
    at: NaiveDateTime,
    status: &str,
) -> Result<usize, String> {
    connection.execute(
        "UPDATE dot_applications SET status=?,ended_at=? WHERE source_file=? AND target_name=? COLLATE NOCASE AND status='active'",
        params![status,at.to_string(),source,mob],
    ).map_err(|error| error.to_string())
}

fn finish_character(
    connection: &rusqlite::Connection,
    source: &str,
    character: &str,
    at: NaiveDateTime,
) -> Result<usize, String> {
    connection.execute(
        "UPDATE dot_applications SET status='playerDeath',ended_at=? WHERE status='active' AND source_file=?
         AND encounter_id IN (SELECT id FROM damage_encounters WHERE character_name=? COLLATE NOCASE)",
        params![at.to_string(),source,character],
    ).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dawncall_profile() -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: "Dawncall".into(),
            cast_on_other: "Someone staggers as the light of dawn washes over it.".into(),
            damage_kind: "dot".into(),
            direct_damage: None,
            damage_per_tick: Some(125),
            tick_count: Some(6),
            tick_interval_seconds: 6,
            casting_time_seconds: Some(3.0),
            observed_source_kind: None,
            observed_source_name: None,
        }
    }

    fn essence_tap_profile() -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: "Essence Tap".into(),
            cast_on_other: "Someone staggers.".into(),
            damage_kind: "direct".into(),
            direct_damage: Some(20),
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: Some(3.0),
            observed_source_kind: None,
            observed_source_name: None,
        }
    }

    fn lifetap_profile() -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: "Lifetap".into(),
            cast_on_other: "Someone staggers.".into(),
            damage_kind: "direct".into(),
            direct_damage: Some(5),
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: Some(3.0),
            observed_source_kind: None,
            observed_source_name: None,
        }
    }

    fn hundred_blows_profile() -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: "One Hundred Blows".into(),
            cast_on_other: "Someone begins to spin from one hundred blows.".into(),
            damage_kind: "direct".into(),
            direct_damage: Some(1),
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: None,
            observed_source_kind: None,
            observed_source_name: None,
        }
    }

    fn root_profile(name: &str, damage: u64) -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: name.into(),
            cast_on_other: "Someone 's feet become entwined.".into(),
            damage_kind: "direct".into(),
            direct_damage: Some(damage),
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: Some(2.5),
            observed_source_kind: Some("proc_only".into()),
            observed_source_name: None,
        }
    }
    fn curse_of_spirits_click_profile() -> CombatSpellProfile {
        CombatSpellProfile {
            spell_name: "Curse of the Spirits".into(),
            cast_on_other: "Someone is consumed by the raging spirits of the land.".into(),
            damage_kind: "dot".into(),
            direct_damage: None,
            damage_per_tick: Some(11),
            tick_count: Some(15),
            tick_interval_seconds: 6,
            casting_time_seconds: Some(0.0),
            observed_source_kind: Some("item_click_only".into()),
            observed_source_name: Some("Spear of Fate".into()),
        }
    }

    #[test]
    fn cast_on_other_message_becomes_a_target_matcher() {
        let compiled = compile_profile(dawncall_profile()).unwrap();
        let captures = compiled
            .landing
            .captures("Hexbone skeleton staggers as the light of dawn washes over it.")
            .unwrap();
        assert_eq!(
            captures.name("target").unwrap().as_str(),
            "Hexbone skeleton"
        );
    }

    #[test]
    fn glow_is_only_a_clue_and_never_creates_dot_damage_by_itself() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let glow = "[Sun Sep 06 10:53:00 2026] Your Test Weapon begins to glow.";
        let glow_event = crate::domain::log_events::parse_log_event(glow, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                1,
                glow,
                "Asquatii",
                glow_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                2,
                "[Sun Sep 06 10:53:01 2026] Asquatii is surrounded by a protective light.",
                "Asquatii",
                None,
            )
            .unwrap();

        let application_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM dot_applications", [], |row| {
                row.get(0)
            })
            .unwrap();
        let damage_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!((application_count, damage_count), (0, 0));
    }

    #[test]
    fn observed_item_click_only_landing_stays_unattributed_before_an_attack() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(curse_of_spirits_click_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (offset, line) in [
            (1, "[Mon Sep 07 14:28:25 2026] Hexbone skeleton is consumed by the raging spirits of the land."),
            (2, "[Mon Sep 07 14:28:25 2026] Anotherplayer crushes Hexbone skeleton for 30 points of damage."),
        ] {
            let event = crate::domain::log_events::parse_log_event(line, "Asquatii");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Asquatii.txt",
                    offset,
                    line,
                    "Asquatii",
                    event.as_ref(),
                )
                .unwrap();
            if offset == 1 {
                let premature_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(premature_count, 0);
            }
        }
        let activity: (String, String, Option<String>) = connection
            .query_row(
                "SELECT caster_name,source_kind,source_name FROM combat_spell_activity",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            activity,
            (
                "Unattributed".into(),
                "unknown".into(),
                Some("Spear of Fate".into())
            )
        );
        let proc_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        let dot_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM dot_applications", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((proc_count, dot_count), (0, 0));
    }

    #[test]
    fn immediately_following_same_target_attack_attributes_a_proc() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();

        let landing =
            "[Sun Sep 06 10:53:03 2026] Hexbone skeleton staggers as the light of dawn washes over it.";
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                1,
                landing,
                "Asquatii",
                None,
            )
            .unwrap();
        let riposte = "[Sun Sep 06 10:53:04 2026] Asquatii tries to crush Hexbone skeleton, but Hexbone skeleton ripostes!";
        let riposte_event = crate::domain::log_events::parse_log_event(riposte, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                2,
                riposte,
                "Asquatii",
                riposte_event.as_ref(),
            )
            .unwrap();

        let attribution: (String, String) = connection
            .query_row(
                "SELECT caster_name,attribution_method FROM dot_applications WHERE target_name='Hexbone skeleton'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(attribution, ("Asquatii".into(), "proc".into()));
    }

    #[test]
    fn blocked_attack_confirms_local_proc_after_non_melee_damage() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(hundred_blows_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let lines = [
            "[Mon Sep 07 12:47:42 2026] a helot spectre was hit by non-melee for 120 points of damage.",
            "[Mon Sep 07 12:47:42 2026] A helot spectre begins to spin from one hundred blows.",
            "[Mon Sep 07 12:47:42 2026] You try to crush a helot spectre, but a helot spectre blocks!",
            "[Mon Sep 07 12:47:42 2026] You crush a helot spectre for 89 points of damage.",
        ];
        for (index, line) in lines.iter().enumerate() {
            let event = crate::domain::log_events::parse_log_event(line, "Valmez");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Valmez_P1999Green.txt",
                    index as i64 + 1,
                    line,
                    "Valmez",
                    event.as_ref(),
                )
                .unwrap();
        }

        let occurrence: (String, String, i64) = connection
            .query_row(
                "SELECT caster_name,spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            occurrence,
            ("Valmez".into(), "One Hundred Blows".into(), 120)
        );
        let fighter_damage: i64 = connection
            .query_row(
                "SELECT total_damage FROM damage_participant_summaries WHERE attacker_name='Valmez'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let encounter_damage: i64 = connection
            .query_row("SELECT total_damage FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((fighter_damage, encounter_damage), (120, 120));
    }
    #[test]
    fn missed_attack_confirms_local_proc_for_the_active_fighter() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(hundred_blows_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (index, line) in [
            "[Sun Sep 06 11:51:43 2026] a helot spectre was hit by non-melee for 120 points of damage.",
            "[Sun Sep 06 11:51:43 2026] A helot spectre begins to spin from one hundred blows.",
            "[Sun Sep 06 11:51:43 2026] You try to crush a helot spectre, but miss!",
        ]
        .iter()
        .enumerate()
        {
            let event = crate::domain::log_events::parse_log_event(line, "Valmez");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Valmez_P1999Green.txt",
                    index as i64 + 1,
                    line,
                    "Valmez",
                    event.as_ref(),
                )
                .unwrap();
        }

        let occurrence: (String, String, i64) = connection
            .query_row(
                "SELECT caster_name,spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            occurrence,
            ("Valmez".into(), "One Hundred Blows".into(), 120)
        );
        let fighter: (i64, i64) = connection
            .query_row(
                "SELECT total_damage,hit_count FROM damage_participant_summaries WHERE attacker_name='Valmez'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(fighter, (120, 1));
    }

    #[test]
    fn hundred_blows_attributes_local_and_named_dodged_proc_sequences() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut profile = hundred_blows_profile();
        profile.direct_damage = Some(120);
        profile.observed_source_kind = Some("proc_only".into());
        profile.observed_source_name = Some("Tranquil Staff".into());
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(profile).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let lines = [
            "[Sat Sep 12 15:23:26 2026] hexbone skeleton was hit by non-melee for 120 points of damage.",
            "[Sat Sep 12 15:23:26 2026] Hexbone skeleton begins to spin from one hundred blows.",
            "[Sat Sep 12 15:23:26 2026] Sakkai crushes hexbone skeleton for 53 points of damage.",
            "[Sat Sep 12 15:23:35 2026] Hexbone skeleton begins to spin from one hundred blows.",
            "[Sat Sep 12 15:23:35 2026] Sakkai tries to crush hexbone skeleton, but hexbone skeleton dodges!",
        ];
        for (index, line) in lines.iter().enumerate() {
            let event = crate::domain::log_events::parse_log_event(line, "Valmezz");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Valmezz_P1999Green.txt",
                    index as i64 + 1,
                    line,
                    "Valmezz",
                    event.as_ref(),
                )
                .unwrap();
        }

        let occurrences = connection
            .prepare(
                "SELECT caster_name,direct_damage FROM proc_occurrences ORDER BY landing_source_offset",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            occurrences,
            vec![("Valmezz".into(), 120), ("Sakkai".into(), 120)]
        );
        let sakkai_damage: i64 = connection
            .query_row(
                "SELECT total_damage FROM damage_participant_summaries WHERE attacker_name='Sakkai'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sakkai_damage, 120);
    }

    #[test]
    fn divine_might_landing_uses_the_next_same_target_attacker() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let profile = CombatSpellProfile {
            spell_name: "Divine Might Effect".into(),
            cast_on_other: "Someone is struck by a surge of Divine Might.".into(),
            damage_kind: "direct".into(),
            direct_damage: Some(65),
            damage_per_tick: None,
            tick_count: None,
            tick_interval_seconds: 6,
            casting_time_seconds: None,
            observed_source_kind: Some("proc_only".into()),
            observed_source_name: Some("Divine Might".into()),
        };
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(profile).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (index, line) in [
            "[Sat Sep 12 15:44:59 2026] A helot skeleton is struck by a surge of Divine Might.",
            "[Sat Sep 12 15:44:59 2026] Balbazak pierces a helot skeleton for 109 points of damage.",
        ]
        .iter()
        .enumerate()
        {
            let event = crate::domain::log_events::parse_log_event(line, "Valmezz");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Valmezz_P1999Green.txt",
                    index as i64 + 1,
                    line,
                    "Valmezz",
                    event.as_ref(),
                )
                .unwrap();
        }
        let occurrence: (String, String, i64) = connection
            .query_row(
                "SELECT caster_name,spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            occurrence,
            ("Balbazak".into(), "Divine Might Effect".into(), 65)
        );
        let source_name: String = connection
            .query_row("SELECT source_name FROM combat_spell_activity", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(source_name, "Divine Might");
    }

    #[test]
    fn next_same_target_attack_confirms_observed_proc_without_guessing_the_caster() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();

        let preceding_attack =
            "[Sun Sep 06 10:54:00 2026] You crush a frost giant for 80 points of damage.";
        let preceding_event =
            crate::domain::log_events::parse_log_event(preceding_attack, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                10,
                preceding_attack,
                "Asquatii",
                preceding_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                11,
                "[Sun Sep 06 10:54:01 2026] A frost giant staggers as the light of dawn washes over it.",
                "Asquatii",
                None,
            )
            .unwrap();
        let unrelated_attack =
            "[Sun Sep 06 10:54:02 2026] Legiteral crushes a frost giant for 81 points of damage.";
        let unrelated_event =
            crate::domain::log_events::parse_log_event(unrelated_attack, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                12,
                unrelated_attack,
                "Asquatii",
                unrelated_event.as_ref(),
            )
            .unwrap();

        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                20,
                "[Sun Sep 06 10:55:00 2026] An ice goblin staggers as the light of dawn washes over it.",
                "Asquatii",
                None,
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                21,
                "[Sun Sep 06 10:55:01 2026] You say, 'intervening line'",
                "Asquatii",
                None,
            )
            .unwrap();
        let late_attack =
            "[Sun Sep 06 10:55:02 2026] Legiteral crushes An ice goblin for 81 points of damage.";
        let late_event = crate::domain::log_events::parse_log_event(late_attack, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                22,
                late_attack,
                "Asquatii",
                late_event.as_ref(),
            )
            .unwrap();

        let attributions = connection
            .prepare(
                "SELECT target_name,caster_name,attribution_method FROM dot_applications ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            attributions,
            vec![(
                "A frost giant".into(),
                "Unidentified player".into(),
                "proc".into()
            )]
        );
        let activity_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM combat_spell_activity", [], |row| {
                row.get(0)
            })
            .unwrap();
        let proc_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((activity_count, proc_count), (1, 1));
    }
    #[test]
    fn root_landing_uses_the_next_same_target_attacker_as_the_likely_proccer() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: ["Engulfing Roots", "Ensnaring Roots"]
                .into_iter()
                .map(|name| compile_profile(root_profile(name, 160)).unwrap())
                .collect(),
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let lines = [
            "[Fri Sep 11 22:53:46 2026] A sepulcher spirit's feet become entwined.",
            "[Fri Sep 11 22:53:46 2026] Tokuo slashes a sepulcher spirit for 174 points of damage.",
        ];
        for (offset, raw) in lines.iter().enumerate() {
            let event = crate::domain::log_events::parse_log_event(raw, "Derpscleric");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Derpscleric.txt",
                    offset as i64,
                    raw,
                    "Derpscleric",
                    event.as_ref(),
                )
                .unwrap();
        }
        let proc: (String, String, i64) = connection
            .query_row(
                "SELECT caster_name,spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(proc, ("Tokuo".into(), "Unidentified root proc".into(), 0));
    }

    #[test]
    fn active_root_cast_is_not_counted_as_a_proc() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: ["Engulfing Roots", "Ensnaring Roots"]
                .into_iter()
                .map(|name| compile_profile(root_profile(name, 160)).unwrap())
                .collect(),
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (offset, raw) in [
            "[Fri Sep 11 22:53:43 2026] You begin casting Engulfing Roots.",
            "[Fri Sep 11 22:53:46 2026] A sepulcher spirit's feet become entwined.",
        ]
        .iter()
        .enumerate()
        {
            let event = crate::domain::log_events::parse_log_event(raw, "Derpscleric");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Derpscleric.txt",
                    offset as i64,
                    raw,
                    "Derpscleric",
                    event.as_ref(),
                )
                .unwrap();
        }
        let proc_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        let source_kind: String = connection
            .query_row("SELECT source_kind FROM combat_spell_activity", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((proc_count, source_kind), (0, "direct".into()));
    }

    #[test]
    fn essence_tap_self_effect_and_following_attack_identify_the_named_proc_caster() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(essence_tap_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let lines = [
            "[Fri Sep 11 21:09:22 2026] Tokuo says 'Ahhh, I feel much better now\\...'",
            "[Fri Sep 11 21:09:22 2026] A bottomless devourer staggers.",
            "[Fri Sep 11 21:09:22 2026] Tokuo slashes a bottomless devourer for 174 points of damage.",
        ];
        for (offset, raw) in lines.iter().enumerate() {
            let event = crate::domain::log_events::parse_log_event(raw, "Derpscleric");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Derpscleric.txt",
                    offset as i64,
                    raw,
                    "Derpscleric",
                    event.as_ref(),
                )
                .unwrap();
        }
        let caster: String = connection
            .query_row("SELECT caster_name FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(caster, "Tokuo");
    }

    #[test]
    fn attack_confirmed_direct_proc_records_count_and_catalog_damage_once() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(essence_tap_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let non_melee =
            "[Mon Mar 23 16:00:01 2026] Yeldema was hit by non-melee for 80 points of damage.";
        let non_melee_event = crate::domain::log_events::parse_log_event(non_melee, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                1,
                non_melee,
                "Asquatii",
                non_melee_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                2,
                "[Mon Mar 23 16:00:01 2026] Yeldema staggers.",
                "Asquatii",
                None,
            )
            .unwrap();
        let attack = "[Mon Mar 23 16:00:01 2026] You crush Yeldema for 51 points of damage.";
        let attack_event = crate::domain::log_events::parse_log_event(attack, "Asquatii");
        crate::application::runtime::record_damage_event(
            &connection,
            "eqlog_Asquatii.txt",
            3,
            attack,
            "Asquatii",
            "Asquatii",
            NaiveDateTime::parse_from_str("2026-03-23 16:00:01", "%Y-%m-%d %H:%M:%S").unwrap(),
            "Yeldema",
            "crush",
            51,
            "melee",
        )
        .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                3,
                attack,
                "Asquatii",
                attack_event.as_ref(),
            )
            .unwrap();
        let occurrence: (String, String, i64) = connection
            .query_row(
                "SELECT caster_name,spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(occurrence, ("Asquatii".into(), "Essence Tap".into(), 80));
        let tracked: (String, String) = connection
            .query_row(
                "SELECT caster_name,source_kind FROM combat_spell_activity",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(tracked, ("Asquatii".into(), "proc".into()));
        let summary: (i64, i64, i64) = connection
            .query_row(
                "SELECT proc_count,direct_proc_damage,dot_damage
                 FROM damage_spell_summaries",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(summary, (1, 80, 0));
        let totals: (i64, i64) = connection
            .query_row(
                "SELECT total_damage,spell_damage FROM damage_encounters",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(totals, (131, 80));

        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                3,
                attack,
                "Asquatii",
                attack_event.as_ref(),
            )
            .unwrap();
        let occurrence_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM proc_occurrences", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(occurrence_count, 1);
    }

    #[test]
    fn equipped_weapon_resolves_a_colliding_proc_landing() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![
                compile_profile(essence_tap_profile()).unwrap(),
                compile_profile(lifetap_profile()).unwrap(),
            ],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO character_weapon_loadouts(
                    character_name,captured_at,primary_weapon_name,source_file
                 ) VALUES('Asquatii','2026-03-23 16:00:00','Essence Mace','inventory.txt')",
                [],
            )
            .unwrap();
        for (offset, line) in [
            (
                1,
                "[Mon Mar 23 16:00:01 2026] Yeldema was hit by non-melee for 80 points of damage.",
            ),
            (2, "[Mon Mar 23 16:00:01 2026] Yeldema staggers."),
            (
                3,
                "[Mon Mar 23 16:00:01 2026] You crush Yeldema for 51 points of damage.",
            ),
        ] {
            let event = crate::domain::log_events::parse_log_event(line, "Asquatii");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Asquatii.txt",
                    offset,
                    line,
                    "Asquatii",
                    event.as_ref(),
                )
                .unwrap();
        }
        let occurrence = connection
            .query_row(
                "SELECT spell_name,direct_damage FROM proc_occurrences",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .unwrap();
        assert_eq!(occurrence, ("Essence Tap".to_owned(), 80));
        let source_name = connection
            .query_row("SELECT source_name FROM combat_spell_activity", [], |row| {
                row.get::<_, Option<String>>(0)
            })
            .unwrap();
        assert_eq!(source_name.as_deref(), Some("Essence Mace"));
    }

    #[test]
    fn direct_cast_attributes_the_matching_landing_to_the_active_character() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let cast = "[Sun Sep 06 10:52:59 2026] You begin casting Dawncall.";
        let cast_event = crate::domain::log_events::parse_log_event(cast, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                1,
                cast,
                "Asquatii",
                cast_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                2,
                "[Sun Sep 06 10:53:04 2026] Hexbone skeleton staggers as the light of dawn washes over it.",
                "Asquatii",
                None,
            )
            .unwrap();
        let attribution: (String, String) = connection
            .query_row(
                "SELECT caster_name,attribution_method FROM dot_applications",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(attribution, ("Asquatii".into(), "direct_cast".into()));
        let activity: (String, String, Option<String>) = connection
            .query_row(
                "SELECT caster_name,source_kind,source_name FROM combat_spell_activity",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(activity, ("Asquatii".into(), "direct".into(), None));
    }

    #[test]
    fn backlog_replay_reuses_a_closed_encounter_at_the_same_source_offset() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let source = "eqlog_Asquatii.txt";
        let cast = "[Sun Sep 06 10:53:01 2026] You begin casting Dawncall.";
        let landing = "[Sun Sep 06 10:53:04 2026] Hexbone skeleton staggers as the light of dawn washes over it.";

        let cast_event = crate::domain::log_events::parse_log_event(cast, "Asquatii");
        tracker
            .process_line(
                &connection,
                source,
                1,
                cast,
                "Asquatii",
                cast_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(&connection, source, 2, landing, "Asquatii", None)
            .unwrap();
        connection
            .execute(
                "UPDATE damage_encounters SET outcome='slain',ended_at=last_damage_at",
                [],
            )
            .unwrap();

        let cast_event = crate::domain::log_events::parse_log_event(cast, "Asquatii");
        tracker
            .process_line(
                &connection,
                source,
                1,
                cast,
                "Asquatii",
                cast_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(&connection, source, 2, landing, "Asquatii", None)
            .unwrap();

        let encounter_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        let application_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM dot_applications", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!((encounter_count, application_count), (1, 1));
    }

    #[test]
    fn interruption_cancels_direct_cast_and_item_click_attribution() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (offset, line) in [
            (1, "[Fri May 09 10:12:37 2025] You begin casting Dawncall."),
            (2, "[Fri May 09 10:12:39 2025] Your spell is interrupted."),
            (3, "[Fri May 09 10:12:40 2025] A frost giant staggers as the light of dawn washes over it."),
            (4, "[Fri May 09 10:13:00 2025] Your Great Spear of Dawn begins to glow."),
            (5, "[Fri May 09 10:13:01 2025] Your spell is interrupted."),
            (6, "[Fri May 09 10:13:02 2025] An ice giant staggers as the light of dawn washes over it."),
        ] {
            let event = crate::domain::log_events::parse_log_event(line, "Asquatii");
            tracker
                .process_line(
                    &connection,
                    "eqlog_Asquatii.txt",
                    offset,
                    line,
                    "Asquatii",
                    event.as_ref(),
                )
                .unwrap();
        }
        let sources = connection
            .prepare("SELECT source_kind FROM combat_spell_activity ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(sources.is_empty());
    }

    #[test]
    fn item_click_landing_retains_the_glowing_item_name() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        for (offset, line) in [
            (1, "[Sun Sep 06 10:53:03 2026] Your Great Spear of Dawn begins to glow."),
            (2, "[Sun Sep 06 10:53:04 2026] Hexbone skeleton staggers as the light of dawn washes over it."),
        ] {
            let event = crate::domain::log_events::parse_log_event(line, "Asquatii");
            tracker.process_line(&connection, "eqlog_Asquatii.txt", offset, line, "Asquatii", event.as_ref()).unwrap();
        }
        let activity: (String, String, String) = connection
            .query_row(
                "SELECT caster_name,source_kind,source_name FROM combat_spell_activity",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            activity,
            (
                "Asquatii".into(),
                "item_click".into(),
                "Great Spear of Dawn".into()
            )
        );
    }

    #[test]
    fn landing_ticks_and_refresh_are_persisted_without_stacking() {
        use crate::infrastructure::database::Database;
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let catalog = SpellCatalog::open(directory.path().join("spells.db")).unwrap();
        let mut tracker = DotTracker {
            catalog,
            profiles: vec![compile_profile(dawncall_profile()).unwrap()],
            loaded_at: Some(Instant::now()),
            recent_glow: None,
            recent_cast: None,
            recent_non_melee: None,
            pending_proc: None,
        };
        let connection = database.connect().unwrap();
        let cast = "[Sun Sep 06 10:53:01 2026] You begin casting Dawncall.";
        let cast_event = crate::domain::log_events::parse_log_event(cast, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                5,
                cast,
                "Asquatii",
                cast_event.as_ref(),
            )
            .unwrap();
        tracker.process_line(&connection, "eqlog_Asquatii.txt", 10,
            "[Sun Sep 06 10:53:04 2026] Hexbone skeleton staggers as the light of dawn washes over it.",
            "Asquatii", None).unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                20,
                "[Sun Sep 06 10:53:10 2026] You say, 'tick'",
                "Asquatii",
                None,
            )
            .unwrap();
        let total: i64 = connection
            .query_row("SELECT total_damage FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(total, 125);
        let event: (String, String) = connection.query_row(
            "SELECT happened_at,damage_type FROM damage_events WHERE source_file LIKE 'dot://%'",
            [], |row| Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        assert_eq!(event, ("2026-09-06 10:53:10".into(), "spell".into()));
        let glow = "[Sun Sep 06 10:53:11 2026] Your Great Spear of Dawn begins to glow.";
        let glow_event = crate::domain::log_events::parse_log_event(glow, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                25,
                glow,
                "Asquatii",
                glow_event.as_ref(),
            )
            .unwrap();
        tracker.process_line(&connection, "eqlog_Asquatii.txt", 30,
            "[Sun Sep 06 10:53:12 2026] Hexbone skeleton staggers as the light of dawn washes over it.",
            "Asquatii", None).unwrap();
        let active: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM dot_applications WHERE status='active'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let refreshed: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM dot_applications WHERE status='refreshed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((active, refreshed), (1, 1));
    }
}
