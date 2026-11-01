use crate::infrastructure::{database::Database, screen_capture};
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock, RwLock,
    },
    thread,
    time::Duration,
};
use tauri::Emitter;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GaugeRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturePreview {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub data_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GaugeReading {
    pub character: String,
    pub percent: f64,
    pub confidence: f64,
    pub observed_at: String,
}

#[derive(Debug, Clone, Copy)]
struct Estimate {
    percent: f64,
    confidence: f64,
    hue: usize,
}

struct TrackingConfiguration {
    character: String,
    region: GaugeRegion,
    hue: usize,
    scale: f64,
    target: Option<screen_capture::CaptureWindow>,
}

fn apply_scale(mut estimate: Estimate, scale: f64) -> Estimate {
    estimate.percent = (estimate.percent * scale).clamp(0.0, 100.0);
    estimate
}

#[derive(Debug, Clone)]
struct VerificationCapture {
    character: String,
    preview: CapturePreview,
}

fn latest_verification() -> &'static RwLock<Option<VerificationCapture>> {
    static VALUE: OnceLock<RwLock<Option<VerificationCapture>>> = OnceLock::new();
    VALUE.get_or_init(|| RwLock::new(None))
}

fn remember_verification(
    capture: &screen_capture::CapturedWindow,
    region: GaugeRegion,
    character: &str,
    expected_hue: usize,
    scale: f64,
) -> Result<(), String> {
    let (left, top, width, height) = selected_bounds(capture, region)?;
    let crop_left = left;
    let crop_top = top;
    let crop_right = left + width;
    let crop_bottom = top + height;
    let crop_width = crop_right - crop_left;
    let crop_height = crop_bottom - crop_top;
    let mut pixels = Vec::with_capacity(crop_width * crop_height * 4);
    for y in crop_top..crop_bottom {
        let start = (y * capture.width as usize + crop_left) * 4;
        pixels.extend_from_slice(&capture.pixels[start..start + crop_width * 4]);
    }
    let cropped = screen_capture::CapturedWindow {
        title: capture.title.clone(),
        width: crop_width as u32,
        height: crop_height as u32,
        pixels,
    };
    let diagnostic = estimate(capture, region, Some(expected_hue)).ok();
    *latest_verification()
        .write()
        .map_err(|_| "XP verification image lock was poisoned.".to_owned())? =
        Some(VerificationCapture {
            character: character.to_owned(),
            preview: CapturePreview {
                title: capture.title.clone(),
                width: cropped.width,
                height: cropped.height,
                data_url: cropped.bmp_data_url(),
                raw_percent: diagnostic.map(|value| value.percent),
                adjusted_percent: diagnostic.map(|value| apply_scale(value, scale).percent),
                confidence: diagnostic.map(|value| value.confidence),
            },
        });
    Ok(())
}

fn load_capture_window(
    connection: &rusqlite::Connection,
) -> Result<Option<screen_capture::CaptureWindow>, String> {
    setting(connection, "xp_visual_capture_window")?
        .map(|raw| {
            serde_json::from_str(&raw)
                .map_err(|error| format!("Invalid saved XP capture window: {error}"))
        })
        .transpose()
}

pub fn capture_preview(database: &Database, delay_ms: u64) -> Result<CapturePreview, String> {
    thread::sleep(Duration::from_millis(delay_ms.min(10_000)));
    let target = {
        let connection = database.connect().map_err(|error| error.to_string())?;
        load_capture_window(&connection)?
    };
    let capture = screen_capture::capture(true, target.as_ref())?;
    Ok(CapturePreview {
        title: capture.title.clone(),
        width: capture.width,
        height: capture.height,
        data_url: capture.bmp_data_url(),
        raw_percent: None,
        adjusted_percent: None,
        confidence: None,
    })
}

fn selected_bounds(
    capture: &screen_capture::CapturedWindow,
    region: GaugeRegion,
) -> Result<(usize, usize, usize, usize), String> {
    if !region.x.is_finite()
        || !region.y.is_finite()
        || !region.width.is_finite()
        || !region.height.is_finite()
    {
        return Err("The gauge selection is invalid.".to_owned());
    }
    let x = (region.x.clamp(0.0, 1.0) * capture.width as f64).round() as usize;
    let y = (region.y.clamp(0.0, 1.0) * capture.height as f64).round() as usize;
    let width = (region.width.clamp(0.0, 1.0) * capture.width as f64).round() as usize;
    let height = (region.height.clamp(0.0, 1.0) * capture.height as f64).round() as usize;
    let width = width.min(capture.width as usize - x.min(capture.width as usize));
    let height = height.min(capture.height as usize - y.min(capture.height as usize));
    if width < 20 || height < 3 {
        return Err(
            "Select the inside of the XP bar, at least 20 pixels wide and 3 pixels tall."
                .to_owned(),
        );
    }
    Ok((x, y, width, height))
}

fn pixel_hue(red: u8, green: u8, blue: u8) -> Option<usize> {
    let maximum = red.max(green).max(blue) as i32;
    let minimum = red.min(green).min(blue) as i32;
    let delta = maximum - minimum;
    if maximum < 55 || delta < 28 {
        return None;
    }
    let hue = if maximum == red as i32 {
        60.0 * ((green as i32 - blue as i32) as f64 / delta as f64)
    } else if maximum == green as i32 {
        60.0 * (2.0 + (blue as i32 - red as i32) as f64 / delta as f64)
    } else {
        60.0 * (4.0 + (red as i32 - green as i32) as f64 / delta as f64)
    };
    Some(((hue.rem_euclid(360.0) / 15.0).round() as usize) % 24)
}

fn estimate(
    capture: &screen_capture::CapturedWindow,
    region: GaugeRegion,
    expected_hue: Option<usize>,
) -> Result<Estimate, String> {
    let (left, top, width, height) = selected_bounds(capture, region)?;
    let mut histogram = [0usize; 24];
    for y in top..top + height {
        for x in left..left + width {
            let offset = (y * capture.width as usize + x) * 4;
            if let Some(hue) = pixel_hue(
                capture.pixels[offset + 2],
                capture.pixels[offset + 1],
                capture.pixels[offset],
            ) {
                histogram[hue] += 1;
            }
        }
    }
    let hue = expected_hue.unwrap_or_else(|| {
        histogram
            .iter()
            .enumerate()
            .max_by_key(|(_, count)| *count)
            .map(|(index, _)| index)
            .unwrap_or(0)
    });
    let threshold = (height / 5).max(1);
    let mut active = vec![false; width];
    let mut matched = 0usize;
    for (column, is_active) in active.iter_mut().enumerate() {
        let mut count = 0;
        for y in top..top + height {
            let offset = (y * capture.width as usize + left + column) * 4;
            if let Some(value) = pixel_hue(
                capture.pixels[offset + 2],
                capture.pixels[offset + 1],
                capture.pixels[offset],
            ) {
                let distance = value.abs_diff(hue).min(24 - value.abs_diff(hue));
                if distance <= 2 {
                    count += 1;
                }
            }
        }
        *is_active = count >= threshold;
        matched += count;
    }
    let last = active
        .iter()
        .rposition(|value| *value)
        .ok_or_else(|| "No colored XP fill was detected inside the selected bar.".to_owned())?;
    let percent = (((last + 1) as f64 / width as f64) * 1000.0).round() / 10.0;
    let filled = last + 1;
    let continuity = active[..filled].iter().filter(|value| **value).count() as f64 / filled as f64;
    let density = matched as f64 / (width * height) as f64;
    let confidence = (continuity * 0.75 + density.min(0.5) * 0.5).clamp(0.0, 1.0);
    Ok(Estimate {
        percent: percent.clamp(0.0, 100.0),
        confidence,
        hue,
    })
}

fn setting(connection: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row("SELECT value FROM app_settings WHERE key=?", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| error.to_string())
}

fn save_setting(connection: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    connection.execute("INSERT INTO app_settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value]).map_err(|error| error.to_string())?;
    Ok(())
}

fn load_region(
    connection: &rusqlite::Connection,
    character: &str,
) -> Result<Option<(GaugeRegion, usize, f64)>, String> {
    let character_value: Option<(String, i64, f64)> = connection.query_row(
        "SELECT region_json,hue,scale FROM experience_calibrations WHERE character_name=? COLLATE NOCASE",
        [character], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(|error| error.to_string())?;
    if let Some((raw, hue, scale)) = character_value {
        let region = serde_json::from_str(&raw)
            .map_err(|error| format!("Invalid saved XP gauge region: {error}"))?;
        return Ok(Some((region, hue.clamp(0, 23) as usize, scale)));
    }
    let raw = setting(connection, "xp_visual_region")?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let region = serde_json::from_str(&raw)
        .map_err(|error| format!("Invalid saved XP gauge region: {error}"))?;
    let hue = setting(connection, "xp_visual_hue")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let scale = setting(connection, "xp_visual_scale")?
        .and_then(|value| value.parse().ok())
        .unwrap_or(1.0);
    Ok(Some((region, hue, scale)))
}

fn active_character(connection: &rusqlite::Connection) -> Result<String, String> {
    setting(connection, "active_character")?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "No active character has been detected yet.".to_owned())
}

fn confirmed_level_rollover(
    database: &Database,
    character: &str,
    hue: usize,
) -> Result<Option<Estimate>, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let previous: Option<(String, Option<i64>)> = connection
        .query_row(
            "SELECT happened_at,level
             FROM experience_observations
             WHERE character_name=? COLLATE NOCASE
             ORDER BY happened_at DESC,id DESC
             LIMIT 1",
            [character],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((previous_time, Some(previous_level))) = previous else {
        return Ok(None);
    };
    let gained_level: Option<i64> = connection
        .query_row(
            "SELECT level
             FROM activity_level_history
             WHERE character_name=? COLLATE NOCASE
               AND direction='gained'
               AND happened_at>?
               AND level>?
             ORDER BY happened_at DESC,id DESC
             LIMIT 1",
            params![character, previous_time, previous_level],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(gained_level.map(|_| Estimate {
        percent: 0.0,
        confidence: 0.95,
        hue,
    }))
}

fn reliable_estimate(
    database: &Database,
    character: &str,
    hue: usize,
    measured: Result<Estimate, String>,
) -> Result<Estimate, String> {
    match measured {
        Ok(value) if value.confidence >= 0.55 => Ok(value),
        Ok(_) => confirmed_level_rollover(database, character, hue)?.ok_or_else(|| {
            "The XP bar was visible, but this reading was not reliable enough to save.".to_owned()
        }),
        Err(error) => confirmed_level_rollover(database, character, hue)?.ok_or(error),
    }
}

fn save_observation(
    database: &Database,
    character: &str,
    estimate: Estimate,
) -> Result<GaugeReading, String> {
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    let observed_at = Local::now()
        .naive_local()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let previous: Option<(String, f64)> = connection.query_row(
        "SELECT happened_at,percent FROM experience_observations WHERE character_name=? COLLATE NOCASE ORDER BY happened_at DESC,id DESC LIMIT 1",
        [character], |row| Ok((row.get(0)?, row.get(1)?))).optional().map_err(|error| error.to_string())?;
    let previous_time = previous
        .as_ref()
        .map(|value| value.0.as_str())
        .unwrap_or("0000-00-00 00:00:00");
    let gains: i64 = if previous.is_some() {
        connection.query_row(
            "SELECT COUNT(*) FROM experience_gain_events WHERE character_name=? COLLATE NOCASE AND happened_at>? AND happened_at<=?",
            params![character, previous_time, observed_at], |row| row.get(0)).map_err(|error| error.to_string())?
    } else {
        0
    };
    let level: Option<i64> = connection.query_row(
        "SELECT COALESCE(
            (SELECT level_override FROM character_profiles WHERE character_name=? COLLATE NOCASE),
            (SELECT level FROM activity_level_history WHERE character_name=? COLLATE NOCASE ORDER BY happened_at DESC,id DESC LIMIT 1)
         )",
        params![character, character], |row| row.get(0)).optional().map_err(|error| error.to_string())?.flatten();
    connection.execute(
        "INSERT INTO experience_observations(happened_at,character_name,level,percent,confidence,gain_events_since_previous) VALUES(?,?,?,?,?,?)",
        params![observed_at, character, level, estimate.percent, estimate.confidence, gains]).map_err(|error| error.to_string())?;
    Ok(GaugeReading {
        character: character.to_owned(),
        percent: estimate.percent,
        confidence: estimate.confidence,
        observed_at,
    })
}

pub fn calibrate(
    database: &Database,
    region: GaugeRegion,
    displayed_percent: Option<f64>,
    delay_ms: u64,
) -> Result<GaugeReading, String> {
    thread::sleep(Duration::from_millis(delay_ms.min(10_000)));
    let target = {
        let connection = database.connect().map_err(|error| error.to_string())?;
        load_capture_window(&connection)?
    };
    let capture = screen_capture::capture(true, target.as_ref())?;
    let initial = estimate(&capture, region, None)?;
    let scale = displayed_percent
        .filter(|value| *value > 0.0 && *value <= 100.0)
        .map(|displayed| (displayed / initial.percent.max(0.1)).clamp(0.25, 4.0))
        .unwrap_or(1.0);
    let estimate = apply_scale(initial, scale);
    if estimate.confidence < 0.55 {
        return Err("The selected region did not produce a reliable XP reading. Select only the colored interior of the bar.".to_owned());
    }
    let character = {
        let connection = database.connect().map_err(|error| error.to_string())?;
        active_character(&connection)?
    };
    {
        let _guard = database.writer_guard();
        let connection = database.connect().map_err(|error| error.to_string())?;
        let region_json = serde_json::to_string(&region).map_err(|error| error.to_string())?;
        save_setting(&connection, "xp_visual_region", &region_json)?;
        save_setting(&connection, "xp_visual_hue", &estimate.hue.to_string())?;
        save_setting(&connection, "xp_visual_scale", &scale.to_string())?;
        save_setting(&connection, "xp_visual_enabled", "true")?;
        connection.execute(
            "INSERT INTO experience_calibrations(character_name,region_json,hue,scale,updated_at) VALUES(?,?,?,?,CURRENT_TIMESTAMP)
             ON CONFLICT(character_name) DO UPDATE SET region_json=excluded.region_json,hue=excluded.hue,scale=excluded.scale,updated_at=CURRENT_TIMESTAMP",
            params![character,region_json,estimate.hue as i64,scale]).map_err(|error| error.to_string())?;
    }
    remember_verification(&capture, region, &character, estimate.hue, scale)?;
    save_observation(database, &character, estimate)
}

pub fn read_now(database: &Database, delay_ms: u64) -> Result<GaugeReading, String> {
    thread::sleep(Duration::from_millis(delay_ms.min(10_000)));
    let connection = database.connect().map_err(|error| error.to_string())?;
    let character = active_character(&connection)?;
    let (region, hue, scale) = load_region(&connection, &character)?
        .ok_or_else(|| "Calibrate the XP gauge first.".to_owned())?;
    let target = load_capture_window(&connection)?;
    drop(connection);
    let capture = screen_capture::capture(true, target.as_ref())?;
    let estimate = reliable_estimate(
        database,
        &character,
        hue,
        estimate(&capture, region, Some(hue)).map(|value| apply_scale(value, scale)),
    )?;
    remember_verification(&capture, region, &character, hue, scale)?;
    save_observation(database, &character, estimate)
}

pub fn set_enabled(database: &Database, enabled: bool) -> Result<(), String> {
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    save_setting(
        &connection,
        "xp_visual_enabled",
        if enabled { "true" } else { "false" },
    )
}

pub fn set_capture_window(database: &Database, window_id: Option<&str>) -> Result<(), String> {
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    let Some(window_id) = window_id.filter(|value| !value.trim().is_empty()) else {
        connection
            .execute(
                "DELETE FROM app_settings WHERE key='xp_visual_capture_window'",
                [],
            )
            .map_err(|error| error.to_string())?;
        return Ok(());
    };
    let window = screen_capture::list_windows()
        .into_iter()
        .find(|window| window.id == window_id)
        .ok_or_else(|| {
            "That window is no longer available. Refresh the list and try again.".to_owned()
        })?;
    let value = serde_json::to_string(&window).map_err(|error| error.to_string())?;
    save_setting(&connection, "xp_visual_capture_window", &value)
}

pub fn set_zone(database: &Database, zone_name: &str) -> Result<(), String> {
    let zone_name = zone_name.trim();
    if zone_name.is_empty() {
        return Err("Enter a zone name.".to_owned());
    }
    let _guard = database.writer_guard();
    let connection = database.connect().map_err(|error| error.to_string())?;
    let character = active_character(&connection)?;
    connection
        .execute(
            "INSERT INTO character_zone_history(character_name,zone_name,evidence)
         VALUES(?,?,'manual')",
            params![character, zone_name],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn snapshot(database: &Database) -> Result<Value, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let enabled = setting(&connection, "xp_visual_enabled")?.as_deref() == Some("true");
    let active = setting(&connection, "active_character")?.unwrap_or_default();
    let calibrated = load_region(&connection, &active)?.is_some();
    let selected_capture_window = load_capture_window(&connection)?;
    let capture_windows = screen_capture::list_windows();
    let verification = latest_verification().read().ok().and_then(|value| {
        value
            .as_ref()
            .filter(|capture| capture.character.eq_ignore_ascii_case(&active))
            .map(|capture| capture.preview.clone())
    });
    let mut observations = Vec::new();
    let mut statement = connection.prepare("SELECT id,happened_at,character_name,level,percent,confidence,gain_events_since_previous FROM experience_observations ORDER BY happened_at DESC,id DESC LIMIT 1000").map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| Ok(json!({"id":row.get::<_,i64>(0)?,"happenedAt":row.get::<_,String>(1)?,"character":row.get::<_,String>(2)?,"level":row.get::<_,Option<i64>>(3)?,"percent":row.get::<_,f64>(4)?,"confidence":row.get::<_,f64>(5)?,"gainEvents":row.get::<_,i64>(6)?}))).map_err(|error| error.to_string())?;
    for row in rows {
        observations.push(row.map_err(|error| error.to_string())?);
    }
    let current_zone: Option<String> = connection
        .query_row(
            "SELECT zone_name FROM character_zone_history
         WHERE character_name=? COLLATE NOCASE ORDER BY happened_at DESC,id DESC LIMIT 1",
            [&active],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut current_group = connection
        .prepare(
            "SELECT m.name
             FROM current_group g
             JOIN known_members m ON m.id=g.member_id
             ORDER BY m.name COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if !active.is_empty()
        && !current_group
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&active))
    {
        current_group.push(active.clone());
        current_group.sort_by_key(|name| name.to_ascii_lowercase());
    }
    let mut gains = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT e.id,e.happened_at,e.character_name,c.mob_name,c.mob_evidence,
                c.zone_name,c.zone_evidence,
                COALESCE((
                    SELECT json_group_array(member_name) FROM (
                        SELECT member_name FROM experience_gain_group_members
                        WHERE experience_gain_event_id=e.id ORDER BY member_name COLLATE NOCASE
                    )
                ),'[]')
         FROM experience_gain_events e
         LEFT JOIN experience_gain_contexts c ON c.experience_gain_event_id=e.id
         ORDER BY e.happened_at DESC,e.id DESC LIMIT 2000",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| {
        let members: String = row.get(7)?;
        Ok(json!({
            "id":row.get::<_,i64>(0)?,
            "happenedAt":row.get::<_,String>(1)?,
            "character":row.get::<_,String>(2)?,
            "mobName":row.get::<_,Option<String>>(3)?,
            "mobEvidence":row.get::<_,Option<String>>(4)?,
            "zoneName":row.get::<_,Option<String>>(5)?,
            "zoneEvidence":row.get::<_,Option<String>>(6)?,
            "groupMembers":serde_json::from_str::<Value>(&members).unwrap_or_else(|_| json!([])),
        }))
    }).map_err(|error| error.to_string())?;
    for row in rows {
        gains.push(row.map_err(|error| error.to_string())?);
    }
    let mut level_events = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT id,happened_at,character_name,level,direction,source_file
         FROM activity_level_history
         ORDER BY happened_at DESC,id DESC LIMIT 2000",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(json!({
                "id":row.get::<_,i64>(0)?,
                "happenedAt":row.get::<_,String>(1)?,
                "character":row.get::<_,String>(2)?,
                "level":row.get::<_,i64>(3)?,
                "direction":row.get::<_,String>(4)?,
                "sourceFile":row.get::<_,String>(5)?,
            }))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        level_events.push(row.map_err(|error| error.to_string())?);
    }
    Ok(
        json!({"enabled":enabled,"calibrated":calibrated,"activeCharacter":active,"currentZone":current_zone,"currentGroup":current_group,"verification":verification,"selectedCaptureWindow":selected_capture_window,"captureWindows":capture_windows,"observations":observations,"gainEvents":gains,"levelEvents":level_events}),
    )
}

pub fn start(database: Database, app: tauri::AppHandle, revision: Arc<AtomicU64>) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        let configuration = (|| -> Result<Option<TrackingConfiguration>, String> {
            let connection = database.connect().map_err(|error| error.to_string())?;
            if setting(&connection, "xp_visual_enabled")?.as_deref() != Some("true") {
                return Ok(None);
            }
            let character = active_character(&connection)?;
            let target = load_capture_window(&connection)?;
            Ok(
                load_region(&connection, &character)?.map(|(region, hue, scale)| {
                    TrackingConfiguration {
                        character,
                        region,
                        hue,
                        scale,
                        target,
                    }
                }),
            )
        })();
        let Ok(Some(configuration)) = configuration else {
            continue;
        };
        let Ok(capture) = screen_capture::capture(true, configuration.target.as_ref()) else {
            continue;
        };
        let Ok(value) = reliable_estimate(
            &database,
            &configuration.character,
            configuration.hue,
            estimate(&capture, configuration.region, Some(configuration.hue))
                .map(|estimate| apply_scale(estimate, configuration.scale)),
        ) else {
            continue;
        };
        let _ = remember_verification(
            &capture,
            configuration.region,
            &configuration.character,
            configuration.hue,
            configuration.scale,
        );
        let should_save = database.connect().ok().and_then(|connection| connection.query_row(
            "SELECT percent,happened_at FROM experience_observations WHERE character_name=? COLLATE NOCASE ORDER BY happened_at DESC,id DESC LIMIT 1",
            [&configuration.character], |row| Ok((row.get::<_,f64>(0)?,row.get::<_,String>(1)?))).optional().ok().flatten()).map(|(percent, happened_at)| {
                (percent-value.percent).abs() >= 0.1 || chrono::NaiveDateTime::parse_from_str(&happened_at, "%Y-%m-%d %H:%M:%S").map(|time| Local::now().naive_local()-time >= chrono::Duration::seconds(60)).unwrap_or(true)
            }).unwrap_or(true);
        if should_save && save_observation(&database, &configuration.character, value).is_ok() {
            revision.fetch_add(1, Ordering::Relaxed);
            let _ = app.emit("experience-changed", ());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_a_half_filled_orange_bar() {
        let mut pixels = vec![0u8; 100 * 10 * 4];
        for y in 0..10 {
            for x in 0..50 {
                let offset = (y * 100 + x) * 4;
                pixels[offset] = 0;
                pixels[offset + 1] = 150;
                pixels[offset + 2] = 220;
                pixels[offset + 3] = 255;
            }
        }
        let capture = screen_capture::CapturedWindow {
            title: "test".into(),
            width: 100,
            height: 10,
            pixels,
        };
        let reading = estimate(
            &capture,
            GaugeRegion {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            None,
        )
        .unwrap();
        assert!((reading.percent - 50.0).abs() < 0.1);
        assert!(reading.confidence > 0.8);
    }

    #[test]
    fn applies_a_saved_numeric_anchor_without_changing_the_region() {
        let estimate = Estimate {
            percent: 60.7,
            confidence: 0.9,
            hue: 4,
        };
        let corrected = apply_scale(estimate, 77.0 / 60.7);
        assert!((corrected.percent - 77.0).abs() < 0.01);
        assert_eq!(corrected.hue, 4);
    }

    #[test]
    fn accepts_an_empty_bar_after_a_confirmed_level_gain() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO experience_observations(
                happened_at,character_name,level,percent,confidence
             ) VALUES('2026-10-24 12:00:00','Tester',42,99.9,0.9)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO activity_level_history(
                happened_at,character_name,level,direction,raw_line,source_file,source_offset
             ) VALUES(
                '2026-10-24 12:00:05','Tester',43,'gained',
                'You have gained a level! Welcome to level 43!','test.log',1
             )",
                [],
            )
            .unwrap();
        drop(connection);

        let reading = reliable_estimate(
            &database,
            "Tester",
            4,
            Err("No colored XP fill was detected inside the selected bar.".to_owned()),
        )
        .unwrap();

        assert_eq!(reading.percent, 0.0);
        assert_eq!(reading.hue, 4);
        assert!(reading.confidence >= 0.9);
    }

    #[test]
    fn does_not_assume_zero_without_a_newer_level_gain() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO activity_level_history(
                happened_at,character_name,level,direction,raw_line,source_file,source_offset
             ) VALUES(
                '2026-10-24 11:59:55','Tester',43,'gained',
                'You have gained a level! Welcome to level 43!','test.log',1
             )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO experience_observations(
                happened_at,character_name,level,percent,confidence
             ) VALUES('2026-10-24 12:00:00','Tester',43,0.0,0.95)",
                [],
            )
            .unwrap();
        drop(connection);

        let result = reliable_estimate(
            &database,
            "Tester",
            4,
            Err("No colored XP fill was detected inside the selected bar.".to_owned()),
        );

        assert!(result.is_err());
    }

    #[test]
    fn snapshot_exposes_the_current_live_group_separately_from_xp_history() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_character','Utrido')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO known_members(name) VALUES('Utrido'),('Gnorbysiete')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO current_group(member_id)
                 SELECT id FROM known_members",
                [],
            )
            .unwrap();
        drop(connection);

        let value = snapshot(&database).unwrap();
        assert_eq!(value["currentGroup"], json!(["Gnorbysiete", "Utrido"]));
    }
}
