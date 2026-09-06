use super::runtime::record_inferred_dot_tick;
use crate::{
    domain::log_events::{parse_envelope, LogEvent},
    infrastructure::spell_catalog::{DotSpellProfile, SpellCatalog},
};
use chrono::{Duration, Local, NaiveDateTime};
use regex::Regex;
use rusqlite::{params, OptionalExtension};
use std::time::{Duration as StdDuration, Instant};

const GLOW_WINDOW_SECONDS: i64 = 3;
const CAST_WINDOW_SECONDS: i64 = 15;
const PROC_WINDOW_SECONDS: i64 = 3;

struct CompiledProfile {
    profile: DotSpellProfile,
    landing: Regex,
}

#[derive(Clone)]
struct RecentGlow {
    happened_at: NaiveDateTime,
    owner_name: String,
}
#[derive(Clone)]
struct RecentCast {
    happened_at: NaiveDateTime,
    spell_name: String,
}
#[derive(Clone)]
struct RecentMelee {
    happened_at: NaiveDateTime,
    target_name: String,
}

pub struct DotTracker {
    catalog: SpellCatalog,
    profiles: Vec<CompiledProfile>,
    loaded_at: Option<Instant>,
    recent_glow: Option<RecentGlow>,
    recent_cast: Option<RecentCast>,
    recent_melee: Option<RecentMelee>,
}

impl DotTracker {
    pub fn new(catalog: SpellCatalog) -> Self {
        Self {
            catalog,
            profiles: Vec::new(),
            loaded_at: None,
            recent_glow: None,
            recent_cast: None,
            recent_melee: None,
        }
    }

    fn refresh_profiles(&mut self) {
        if self
            .loaded_at
            .is_some_and(|loaded| loaded.elapsed() < StdDuration::from_secs(30))
        {
            return;
        }
        if let Ok(profiles) = self.catalog.dot_profiles() {
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
        let mut changed = 0;
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

        if let Some(LogEvent::ItemGlow { owner_name, .. }) = event {
            self.recent_glow = Some(RecentGlow {
                happened_at,
                owner_name: owner_name.clone().unwrap_or_else(|| character.to_owned()),
            });
        }
        if let Some(LogEvent::SpellCastStarted { spell_name, .. }) = event {
            self.recent_cast = Some(RecentCast {
                happened_at,
                spell_name: spell_name.clone(),
            });
        }
        match event {
            Some(LogEvent::Damage {
                attacker_name,
                mob_name,
                damage_type,
                ..
            }) if attacker_name.eq_ignore_ascii_case(character)
                && damage_type.as_str() == "melee" =>
            {
                self.recent_melee = Some(RecentMelee {
                    happened_at,
                    target_name: mob_name.clone(),
                });
            }
            Some(LogEvent::ObservedMelee {
                subject_name,
                target_name,
                ..
            }) if subject_name.eq_ignore_ascii_case(character) => {
                self.recent_melee = Some(RecentMelee {
                    happened_at,
                    target_name: target_name.clone(),
                });
            }
            _ => {}
        }

        let matches = self
            .profiles
            .iter()
            .filter_map(|compiled| {
                let captures = compiled.landing.captures(body)?;
                Some((
                    &compiled.profile,
                    captures.name("target")?.as_str().trim().to_owned(),
                ))
            })
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            let (profile, target) = &matches[0];
            let glow_caster = self.recent_glow.as_ref().filter(|glow| {
                (0..=GLOW_WINDOW_SECONDS).contains(
                    &happened_at
                        .signed_duration_since(glow.happened_at)
                        .num_seconds(),
                )
            });
            let direct_cast = self.recent_cast.as_ref().is_some_and(|cast| {
                cast.spell_name.eq_ignore_ascii_case(&profile.spell_name)
                    && (0..=CAST_WINDOW_SECONDS).contains(
                        &happened_at
                            .signed_duration_since(cast.happened_at)
                            .num_seconds(),
                    )
            });
            let weapon_proc = self.recent_melee.as_ref().is_some_and(|melee| {
                melee.target_name.eq_ignore_ascii_case(target)
                    && (0..=PROC_WINDOW_SECONDS).contains(
                        &happened_at
                            .signed_duration_since(melee.happened_at)
                            .num_seconds(),
                    )
            });
            let (caster, attribution_method) = if let Some(glow) = glow_caster {
                (glow.owner_name.as_str(), "item_glow")
            } else if direct_cast {
                (character, "direct_cast")
            } else if weapon_proc {
                (character, "proc")
            } else {
                ("Unknown", "unknown")
            };
            changed += land_dot(
                connection,
                source,
                source_offset,
                character,
                happened_at,
                target,
                caster,
                attribution_method,
                profile,
            )?;
            self.recent_glow = None;
            if direct_cast {
                self.recent_cast = None;
            }
            if weapon_proc {
                self.recent_melee = None;
            }
        }

        match event {
            Some(LogEvent::CombatAttempt {
                attacker_name,
                mob_name,
                ..
            }) => {
                changed +=
                    attribute_recent(connection, source, happened_at, mob_name, attacker_name)?;
            }
            Some(LogEvent::Damage {
                attacker_name,
                mob_name,
                ..
            }) => {
                changed +=
                    attribute_recent(connection, source, happened_at, mob_name, attacker_name)?;
            }
            Some(LogEvent::ObservedMelee {
                subject_name,
                target_name,
                ..
            }) => {
                changed +=
                    attribute_recent(connection, source, happened_at, target_name, subject_name)?;
            }
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

fn compile_profile(profile: DotSpellProfile) -> Option<CompiledProfile> {
    let escaped = regex::escape(profile.cast_on_other.trim());
    let someone = Regex::new("(?i)someone").expect("valid placeholder regex");
    if !someone.is_match(&escaped) {
        return None;
    }
    let source = someone.replace(&escaped, "(?P<target>.+?)");
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
    profile: &DotSpellProfile,
) -> Result<usize, String> {
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
        params![happened_at.to_string(),encounter_id,target,profile.spell_name],
    ).map_err(|error| error.to_string())?;
    let expires_at = happened_at
        + Duration::seconds(
            i64::from(profile.tick_interval_seconds) * i64::from(profile.tick_count),
        );
    connection
        .execute(
            "INSERT OR IGNORE INTO dot_applications(
            encounter_id,spell_name,target_name,caster_name,attribution_method,landed_at,expires_at,
            damage_per_tick,tick_interval_seconds,total_ticks,source_file,source_offset
         ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                profile.spell_name,
                target,
                caster,
                attribution_method,
                happened_at.to_string(),
                expires_at.to_string(),
                profile.damage_per_tick,
                profile.tick_interval_seconds,
                profile.tick_count,
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())
}

fn ensure_encounter(
    connection: &rusqlite::Connection,
    source: &str,
    offset: i64,
    character: &str,
    at: NaiveDateTime,
    mob: &str,
) -> Result<i64, String> {
    if let Some(id) = connection.query_row(
        "SELECT id FROM damage_encounters WHERE source_file=? AND character_name=? COLLATE NOCASE AND mob_name=? COLLATE NOCASE AND outcome='active' ORDER BY last_source_offset DESC LIMIT 1",
        params![source,character,mob], |row| row.get(0),
    ).optional().map_err(|error| error.to_string())? { return Ok(id); }
    connection.execute(
        "INSERT INTO damage_encounters(character_name,mob_name,started_at,last_damage_at,source_file,first_source_offset,last_source_offset) VALUES(?,?,?,?,?,?,?)",
        params![character,mob,at.to_string(),at.to_string(),source,offset,offset],
    ).map_err(|error| error.to_string())?;
    Ok(connection.last_insert_rowid())
}

fn attribute_recent(
    connection: &rusqlite::Connection,
    source: &str,
    at: NaiveDateTime,
    mob: &str,
    caster: &str,
) -> Result<usize, String> {
    connection.execute(
        "UPDATE dot_applications SET caster_name=?,attribution_method='next_attack'
         WHERE id=(SELECT id FROM dot_applications WHERE source_file=? AND target_name=? COLLATE NOCASE
           AND caster_name='Unknown' COLLATE NOCASE AND status='active'
           AND landed_at<=? AND landed_at>=datetime(?,'-5 seconds') ORDER BY landed_at DESC,id DESC LIMIT 1)",
        params![caster,source,mob,at.to_string(),at.to_string()],
    ).map_err(|error| error.to_string())
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

    fn dawncall_profile() -> DotSpellProfile {
        DotSpellProfile {
            spell_name: "Dawncall".into(),
            cast_on_other: "Someone staggers as the light of dawn washes over it.".into(),
            damage_per_tick: 125,
            tick_count: 6,
            tick_interval_seconds: 6,
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
    fn recent_melee_on_the_same_target_classifies_an_unannounced_landing_as_a_proc() {
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
            recent_melee: None,
        };
        let connection = database.connect().unwrap();
        let attack =
            "[Sun Sep 06 10:53:03 2026] You crush Hexbone skeleton for 79 points of damage.";
        let attack_event = crate::domain::log_events::parse_log_event(attack, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                1,
                attack,
                "Asquatii",
                attack_event.as_ref(),
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
        assert_eq!(attribution, ("Asquatii".into(), "proc".into()));

        let other_attack =
            "[Sun Sep 06 10:54:00 2026] You crush a snow cougar for 80 points of damage.";
        let other_event = crate::domain::log_events::parse_log_event(other_attack, "Asquatii");
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                3,
                other_attack,
                "Asquatii",
                other_event.as_ref(),
            )
            .unwrap();
        tracker
            .process_line(
                &connection,
                "eqlog_Asquatii.txt",
                4,
                "[Sun Sep 06 10:54:01 2026] A frost giant staggers as the light of dawn washes over it.",
                "Asquatii",
                None,
            )
            .unwrap();
        let unrelated: (String, String) = connection
            .query_row(
                "SELECT caster_name,attribution_method FROM dot_applications WHERE target_name='A frost giant'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unrelated, ("Unknown".into(), "unknown".into()));
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
            recent_melee: None,
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
            recent_melee: None,
        };
        let connection = database.connect().unwrap();
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
