use crate::{
    application::{
        combat_metrics::insert_or_get_damage_encounter, data, dot_tracking::DotTracker, services,
        system_tasks::TaskRegistry,
    },
    domain::log_events::{parse_log_event, ChatChannel, GroupChangeKind, LogEvent},
    domain::merchant::{parse_listing_items, CatalogItem},
    infrastructure::{database::Database, spell_catalog::SpellCatalog},
};
use chrono::{Duration as ChronoDuration, NaiveDateTime};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, SystemTime},
};
use tauri::Emitter;

pub fn start(
    database: Database,
    app_handle: tauri::AppHandle,
    revision: Arc<AtomicU64>,
    tasks: TaskRegistry,
    spell_catalog: SpellCatalog,
) {
    let backlog_database = database.clone();
    let backlog_app_handle = app_handle.clone();
    let backlog_revision = revision.clone();
    let upload_database_path = database.path().to_owned();
    let upload_app_handle = app_handle.clone();
    let upload_revision = revision.clone();
    let upload_tasks = tasks.clone();
    let summary_database = database.clone();
    let summary_app_handle = app_handle.clone();
    let summary_revision = revision.clone();
    let summary_tasks = tasks.clone();
    let watcher_tasks = tasks.clone();
    let (watcher_ready_tx, watcher_ready_rx) = mpsc::sync_channel(1);
    let (backlog_ready_tx, backlog_ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("eq-runtime-watcher".into())
        .spawn(move || {
            watch(
                database,
                app_handle,
                revision,
                watcher_tasks,
                spell_catalog,
                Some(watcher_ready_tx),
            )
        })
        .expect("runtime watcher thread must start");
    thread::Builder::new()
        .name("eq-log-backlog-scanner".into())
        .spawn(move || {
            scan_log_backlog(
                backlog_database,
                backlog_app_handle,
                backlog_revision,
                tasks,
                watcher_ready_rx,
                backlog_ready_tx,
            )
        })
        .expect("log backlog scanner thread must start");
    thread::Builder::new()
        .name("planner-upload-worker".into())
        .spawn(move || {
            planner_upload_worker(
                upload_database_path,
                upload_app_handle,
                upload_revision,
                upload_tasks,
            )
        })
        .expect("planner upload worker thread must start");
    thread::Builder::new()
        .name("damage-summary-backfill".into())
        .spawn(move || {
            damage_summary_backfill_worker(
                summary_database,
                summary_app_handle,
                summary_revision,
                summary_tasks,
                backlog_ready_rx,
            )
        })
        .expect("damage summary backfill thread must start");
}

fn damage_summary_backfill_worker(
    database: Database,
    app_handle: tauri::AppHandle,
    revision: Arc<AtomicU64>,
    tasks: TaskRegistry,
    startup_ready: mpsc::Receiver<()>,
) {
    let _ = startup_ready.recv();
    let writer_guard = database.writer_guard();
    let Ok(connection) = database.connect() else {
        return;
    };
    let _ = connection.execute(
        "UPDATE damage_encounters SET outcome='disengaged',ended_at=last_damage_at
         WHERE outcome='active' AND datetime(last_damage_at)<datetime('now','-120 seconds')",
        [],
    );
    let total = connection
        .query_row(
            "SELECT COALESCE(MAX(id),0) FROM damage_encounters",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);
    let mut cursor = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='damage_summary_backfill_cursor'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(0);
    drop(connection);
    drop(writer_guard);
    if cursor >= total {
        return;
    }
    tasks.start(
        "damage-summary-backfill",
        "Optimizing damage history",
        "Preparing encounter summaries",
        Some(total as u64),
    );
    let _ = app_handle.emit("system-task-changed", "damage-summary-backfill");
    let result = (|| -> Result<serde_json::Value, String> {
        while cursor < total {
            let _writer_guard = database.writer_guard();
            let mut connection = database.connect().map_err(|error| error.to_string())?;
            let next=connection.query_row(
                "SELECT COALESCE(MAX(id),?) FROM (SELECT id FROM damage_encounters WHERE id>? ORDER BY id LIMIT 250)",
                params![cursor,cursor],|row|row.get::<_,i64>(0),
            ).map_err(|error|error.to_string())?;
            if next <= cursor {
                break;
            }
            let transaction = connection
                .transaction()
                .map_err(|error| error.to_string())?;
            transaction.execute(
                "INSERT OR REPLACE INTO damage_participant_summaries(encounter_id,attacker_name,total_damage,hit_count,first_damage_at,last_damage_at)
                 SELECT encounter_id,COALESCE(NULLIF(attacker_name,''),'Unknown'),SUM(damage),COUNT(*),MIN(happened_at),MAX(happened_at)
                 FROM damage_events WHERE encounter_id>? AND encounter_id<=?
                 GROUP BY encounter_id,COALESCE(NULLIF(attacker_name,''),'Unknown') COLLATE NOCASE",
                params![cursor,next],
            ).map_err(|error|error.to_string())?;
            transaction.execute(
                "INSERT OR REPLACE INTO damage_target_summaries(encounter_id,target_name,total_damage,hit_count,max_hit,first_damage_at,last_damage_at)
                 SELECT encounter_id,target_name,SUM(damage),COUNT(*),MAX(damage),MIN(happened_at),MAX(happened_at)
                 FROM damage_received_events WHERE encounter_id>? AND encounter_id<=?
                 GROUP BY encounter_id,target_name COLLATE NOCASE",
                params![cursor,next],
            ).map_err(|error|error.to_string())?;
            transaction
                .execute(
                    "INSERT INTO app_settings(key,value) VALUES('damage_summary_backfill_cursor',?)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    [next.to_string()],
                )
                .map_err(|error| error.to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
            cursor = next;
            tasks.progress(
                "damage-summary-backfill",
                &format!("Optimized through encounter {cursor}"),
                Some(cursor as u64),
                Some(total as u64),
            );
            let _ = app_handle.emit("system-task-changed", "damage-summary-backfill");
            thread::yield_now();
        }
        Ok(json!({"completed":cursor,"total":total}))
    })();
    tasks.finish(
        "damage-summary-backfill",
        &result,
        "Damage history optimized",
    );
    revision.fetch_add(1, Ordering::Relaxed);
    let _ = app_handle.emit("system-task-changed", "damage-summary-backfill");
    let _ = app_handle.emit("data-changed", "damage.summary");
}
fn planner_upload_worker(
    database_path: PathBuf,
    app_handle: tauri::AppHandle,
    revision: Arc<AtomicU64>,
    tasks: TaskRegistry,
) {
    let Ok(database) = Database::open(database_path) else {
        return;
    };
    if let Ok(connection) = database.connect() {
        let _ = connection.execute(
            "UPDATE planner_upload_jobs SET status='pending', updated_at=CURRENT_TIMESTAMP WHERE status='running'",
            [],
        );
    }
    loop {
        let job = database.connect().ok().and_then(|connection| {
            connection
                .query_row(
                    "SELECT id,file_name,payload_text,attempts FROM planner_upload_jobs
                 WHERE status='pending' AND datetime(next_attempt_at)<=datetime('now')
                 ORDER BY id LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                        ))
                    },
                )
                .optional()
                .ok()
                .flatten()
        });
        let Some((id, file_name, text, attempts)) = job else {
            thread::sleep(Duration::from_secs(2));
            continue;
        };
        let claimed = database.connect().ok().and_then(|connection| {
            connection.execute(
                "UPDATE planner_upload_jobs SET status='running',updated_at=CURRENT_TIMESTAMP WHERE id=? AND status='pending'",
                [id],
            ).ok()
        }).unwrap_or(0) == 1;
        if !claimed {
            continue;
        }

        tasks.start(
            "planner-upload",
            "Uploading to P99 Planner",
            &file_name,
            Some(1),
        );
        let _ = app_handle.emit("system-task-changed", "planner-upload");
        let result = services::upload_file_payloads(
            &database,
            &json!({"files":[{"name":file_name,"text":text}]}),
        );
        if let Ok(connection) = database.connect() {
            match &result {
                Ok(value) => {
                    let url = value
                        .get("url")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let _=connection.execute(
                        "UPDATE planner_upload_jobs
                         SET status='completed',payload_text='',attempts=attempts+1,last_error='',review_url=?,updated_at=CURRENT_TIMESTAMP
                         WHERE id=?",
                        params![url,id],
                    );
                    let _=connection.execute(
                        "DELETE FROM planner_upload_jobs WHERE status='completed' AND datetime(updated_at)<datetime('now','-30 days')",
                        [],
                    );
                    log(&database,"info","planner","Uploaded export; the private review link is available on Import and System");
                }
                Err(error) => {
                    let delay = (5_i64.saturating_mul(1_i64 << attempts.min(6) as u32)).min(300);
                    let modifier = format!("+{delay} seconds");
                    let _=connection.execute(
                        "UPDATE planner_upload_jobs
                         SET status='pending',attempts=attempts+1,last_error=?,next_attempt_at=datetime('now',?),updated_at=CURRENT_TIMESTAMP
                         WHERE id=?",
                        params![error,modifier,id],
                    );
                    log(
                        &database,
                        "error",
                        "planner",
                        &format!("Inventory upload failed; retrying automatically: {error}"),
                    );
                }
            }
        }
        tasks.finish("planner-upload", &result, "Planner upload complete");
        revision.fetch_add(1, Ordering::Relaxed);
        let _ = app_handle.emit("system-task-changed", "planner-upload");
        let _ = app_handle.emit("data-changed", "planner.upload");
    }
}
fn scan_log_backlog(
    database: Database,
    app_handle: tauri::AppHandle,
    revision: Arc<AtomicU64>,
    tasks: TaskRegistry,
    startup_ready: mpsc::Receiver<()>,
    startup_complete: mpsc::SyncSender<()>,
) {
    let _ = startup_ready.recv();
    let mut startup_complete = Some(startup_complete);
    let mut configured_root: Option<PathBuf> = None;
    let mut last_scan = SystemTime::UNIX_EPOCH;
    loop {
        let configured = configured_log_directory(&database);
        let directory_changed = configured != configured_root;
        let scan_due = last_scan
            .elapsed()
            .map_or(true, |elapsed| elapsed >= Duration::from_secs(30));
        if configured.as_ref().is_some_and(|path| path.is_dir()) && (directory_changed || scan_due)
        {
            tasks.start(
                "log-backlog",
                "Checking log backlog",
                "Scanning all character log segments",
                None,
            );
            let _ = app_handle.emit("system-task-changed", "log-backlog");
            let result = scan_history(&database);
            match &result {
                Ok(summary) => {
                    let inserted = summary
                        .get("inserted")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0);
                    if inserted > 0 {
                        revision.fetch_add(1, Ordering::Relaxed);
                        let _ = app_handle.emit("data-changed", "log-backlog");
                    }
                }
                Err(error) => log(&database, "error", "history", error),
            }
            let inserted = result
                .as_ref()
                .ok()
                .and_then(|value| value.get("inserted"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            tasks.finish(
                "log-backlog",
                &result,
                &format!("Backlog current - {inserted} new events"),
            );
            let _ = app_handle.emit("system-task-changed", "log-backlog");
            last_scan = SystemTime::now();
        }
        if let Some(sender) = startup_complete.take() {
            let _ = sender.send(());
        }
        configured_root = configured;
        thread::sleep(Duration::from_secs(2));
    }
}

fn watch(
    database: Database,
    app_handle: tauri::AppHandle,
    revision: Arc<AtomicU64>,
    tasks: TaskRegistry,
    spell_catalog: SpellCatalog,
    mut startup_complete: Option<mpsc::SyncSender<()>>,
) {
    let (event_tx, event_rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |event| {
        let _ = event_tx.send(event);
    }) {
        Ok(watcher) => watcher,
        Err(error) => {
            log(&database, "error", "watcher", &error.to_string());
            return;
        }
    };
    let mut active_log: Option<PathBuf> = None;
    let mut offsets: HashMap<PathBuf, u64> = HashMap::new();
    let mut last_mob: HashMap<PathBuf, String> = HashMap::new();
    let mut export_signatures: HashMap<PathBuf, (u64, SystemTime)> = HashMap::new();
    let mut export_directory: Option<PathBuf> = None;
    let mut configured_root: Option<PathBuf> = None;
    let mut watched_logs: Option<PathBuf> = None;
    let mut watched_exports: Option<PathBuf> = None;
    let mut last_safety_poll = SystemTime::UNIX_EPOCH;
    let mut dot_tracker = DotTracker::new(spell_catalog);
    loop {
        let configured = configured_log_directory(&database);
        let mut force_poll = false;
        let needs_reconfigure = configured != configured_root
            || configured
                .as_ref()
                .is_some_and(|path| path.is_dir() && watched_logs.is_none())
            || configured.as_ref().is_some_and(|path| {
                services::output_directory(path).is_dir() && watched_exports.is_none()
            });
        if needs_reconfigure {
            configured_root = configured.clone();
            if let Some(path) = watched_logs.take() {
                let _ = watcher.unwatch(&path);
            }
            if let Some(path) = watched_exports.take() {
                let _ = watcher.unwatch(&path);
            }
            if let Some(path) = configured.filter(|path| path.is_dir()) {
                if watcher.watch(&path, RecursiveMode::NonRecursive).is_ok() {
                    watched_logs = Some(path.clone());
                }
                let output = services::output_directory(&path);
                if output.is_dir() && watcher.watch(&output, RecursiveMode::NonRecursive).is_ok() {
                    watched_exports = Some(output);
                }
            }
            force_poll = true;
        }
        let mut changed_log: Option<PathBuf> = None;
        let event_received = if force_poll {
            false
        } else {
            match event_rx.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(event)) => {
                    changed_log = event.paths.into_iter().find(|path| is_active_log(path));
                    true
                }
                Ok(Err(error)) => {
                    log(&database, "error", "watcher", &error.to_string());
                    false
                }
                Err(mpsc::RecvTimeoutError::Timeout) => false,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };
        let safety_due = last_safety_poll
            .elapsed()
            .map_or(true, |elapsed| elapsed >= Duration::from_secs(30));
        if !(force_poll || event_received || safety_due) {
            let tick_changed = {
                let _writer_guard = database.writer_guard();
                database
                    .connect()
                    .ok()
                    .and_then(|connection| dot_tracker.flush_live(&connection).ok())
                    .unwrap_or(0)
                    > 0
            };
            if tick_changed {
                revision.fetch_add(1, Ordering::Relaxed);
                let _ = app_handle.emit("data-changed", "damage.dot-tick");
            }
            continue;
        }
        if event_received {
            thread::sleep(Duration::from_millis(60));
            while let Ok(event) = event_rx.try_recv() {
                if let Ok(event) = event {
                    if let Some(path) = event.paths.into_iter().find(|path| is_active_log(path)) {
                        changed_log = Some(path);
                    }
                }
            }
        }
        if force_poll {
            tasks.start(
                "folder-reconcile",
                "Reconciling configured folders",
                "Checking logs, inventory, and spellbook outputs",
                None,
            );
            let _ = app_handle.emit("system-task-changed", "folder-reconcile");
        }
        let poll_result = poll_with_dots(
            &database,
            &mut active_log,
            &mut offsets,
            &mut last_mob,
            &mut export_signatures,
            &mut export_directory,
            changed_log.as_deref(),
            &mut dot_tracker,
        );
        let poll_changed = match &poll_result {
            Ok(changed) => *changed,
            Err(error) => {
                log(&database, "error", "watcher", error);
                false
            }
        };
        if force_poll {
            let task_result = poll_result
                .map(|changed| json!({"changed":changed}))
                .map_err(|error| error.to_string());
            tasks.finish(
                "folder-reconcile",
                &task_result,
                "Logs and output files are current",
            );
            let _ = app_handle.emit("system-task-changed", "folder-reconcile");
        }
        if let Some(sender) = startup_complete.take() {
            let _ = sender.send(());
        }
        last_safety_poll = SystemTime::now();
        if poll_changed {
            revision.fetch_add(1, Ordering::Relaxed);
            let _ = app_handle.emit("data-changed", "watcher");
        }
    }
}

fn configured_log_directory(database: &Database) -> Option<PathBuf> {
    database
        .connect()
        .ok()?
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
fn poll(
    database: &Database,
    active_log: &mut Option<PathBuf>,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    exports: &mut HashMap<PathBuf, (u64, SystemTime)>,
    watched_export_directory: &mut Option<PathBuf>,
    preferred_log: Option<&Path>,
) -> Result<bool, String> {
    poll_internal(
        database,
        active_log,
        offsets,
        last_mob,
        exports,
        watched_export_directory,
        preferred_log,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn poll_with_dots(
    database: &Database,
    active_log: &mut Option<PathBuf>,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    exports: &mut HashMap<PathBuf, (u64, SystemTime)>,
    watched_export_directory: &mut Option<PathBuf>,
    preferred_log: Option<&Path>,
    dot_tracker: &mut DotTracker,
) -> Result<bool, String> {
    poll_internal(
        database,
        active_log,
        offsets,
        last_mob,
        exports,
        watched_export_directory,
        preferred_log,
        Some(dot_tracker),
    )
}

#[allow(clippy::too_many_arguments)]
fn poll_internal(
    database: &Database,
    active_log: &mut Option<PathBuf>,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    exports: &mut HashMap<PathBuf, (u64, SystemTime)>,
    watched_export_directory: &mut Option<PathBuf>,
    preferred_log: Option<&Path>,
    dot_tracker: Option<&mut DotTracker>,
) -> Result<bool, String> {
    let mut changed = false;
    let connection = database.connect().map_err(|error| error.to_string())?;
    let directory: Option<String> = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='logs_directory'",
            [],
            |row| row.get(0),
        )
        .ok();
    drop(connection);
    let Some(directory) = directory else {
        return Ok(false);
    };
    let directory = PathBuf::from(directory);
    if !directory.is_dir() {
        return Ok(false);
    }

    let preferred = preferred_log
        .filter(|path| path.parent() == Some(directory.as_path()) && is_active_log(path))
        .map(Path::to_path_buf);
    let newest = if preferred.is_some() {
        preferred
    } else {
        let mut logs = fs::read_dir(&directory)
            .map_err(|error| error.to_string())?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| is_active_log(path))
            .collect::<Vec<_>>();
        logs.sort_by_key(|path| {
            fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });
        logs.pop()
    };
    if let Some(newest) = newest {
        let mut character_changed = false;
        if active_log.as_ref() != Some(&newest) {
            let _writer_guard = database.writer_guard();
            let character = character_from_log(&newest).unwrap_or_else(|| "Unknown".into());
            let connection = database.connect().map_err(|error| error.to_string())?;
            let previous: Option<String> = connection
                .query_row(
                    "SELECT value FROM app_settings WHERE key='active_character'",
                    [],
                    |r| r.get(0),
                )
                .ok();
            character_changed = previous
                .as_deref()
                .is_some_and(|value| !value.eq_ignore_ascii_case(&character));
            if character_changed {
                connection
                    .execute_batch(
                        "DELETE FROM current_group;
                         DELETE FROM app_settings
                         WHERE key IN ('damage_target_character','damage_target_encounter_id');",
                    )
                    .map_err(|e| e.to_string())?;
            }
            connection
                .execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [&character],
                )
                .map_err(|e| e.to_string())?;
            connection.execute("INSERT INTO app_settings(key,value) VALUES('active_character',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[&character]).map_err(|e|e.to_string())?;
            log_with(
                &connection,
                "info",
                "watcher",
                &format!("Active log: {} ({character})", newest.display()),
            );
            if !offsets.contains_key(&newest) {
                let size = fs::metadata(&newest).map(|m| m.len()).unwrap_or(0);
                let offset = initial_live_offset(database, &newest, size)?;
                offsets.insert(newest.clone(), offset);
            }
            *active_log = Some(newest.clone());
            changed = true;
        }
        changed |= if let Some(tracker) = dot_tracker {
            process_log_with_dots(database, &newest, offsets, last_mob, tracker)?
        } else {
            process_log_internal(database, &newest, offsets, last_mob, None)?
        };
        if character_changed {
            database
                .connect()
                .map_err(|error| error.to_string())?
                .execute("DELETE FROM current_group", [])
                .map_err(|error| error.to_string())?;
        }
    }
    let output = services::output_directory(&directory);
    if watched_export_directory.as_ref() != Some(&output) {
        exports.clear();
        reconcile_exports(database, &output, exports)?;
        *watched_export_directory = Some(output.clone());
        let _writer_guard = database.writer_guard();
        let connection = database.connect().map_err(|error| error.to_string())?;
        connection.execute("INSERT INTO app_settings(key,value) VALUES('export_directory',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[output.display().to_string()]).map_err(|error|error.to_string())?;
        log_with(
            &connection,
            "info",
            "watcher",
            &format!("Watching exports in {}", output.display()),
        );
        changed = true;
    } else {
        changed |= process_exports(database, &output, exports)?;
    }
    Ok(changed)
}

#[derive(Debug, Default, Clone, Copy)]
struct HistoryScanSummary {
    files: usize,
    pub inserted: usize,
}

pub fn scan_history(database: &Database) -> Result<serde_json::Value, String> {
    let directory = configured_log_directory(database)
        .ok_or("Choose a Logs folder on the System page first")?;
    if !directory.is_dir() {
        return Err(format!(
            "Logs folder does not exist: {}",
            directory.display()
        ));
    }
    let summary = scan_history_directory(database, &directory)?;
    Ok(json!({"files":summary.files,"inserted":summary.inserted}))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DamageScanProgress {
    pub completed: usize,
    pub total: usize,
    pub current_file: String,
    inserted: usize,
    pub done: bool,
}

pub fn rescan_damage(
    database: &Database,
    spell_catalog: SpellCatalog,
    mut report: impl FnMut(DamageScanProgress),
) -> Result<serde_json::Value, String> {
    let directory = configured_log_directory(database)
        .ok_or("Choose a Logs folder on the System page first")?;
    if !directory.is_dir() {
        return Err(format!(
            "Logs folder does not exist: {}",
            directory.display()
        ));
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_log(path))
        .collect::<Vec<_>>();
    paths.sort();
    let total = paths.len();
    report(DamageScanProgress {
        completed: 0,
        total,
        current_file: String::new(),
        inserted: 0,
        done: false,
    });

    {
        let connection = database.connect().map_err(|error| error.to_string())?;
        connection
            .execute_batch(
                "DELETE FROM cleric_heal_calls;
                 DELETE FROM damage_encounters WHERE is_protected=0;
                 DELETE FROM damage_scan_cursors;",
            )
            .map_err(|error| error.to_string())?;
    }

    let mut inserted = 0;
    for (index, path) in paths.iter().enumerate() {
        let mut dot_tracker = DotTracker::new(spell_catalog.clone());
        inserted += scan_damage_file_with_dots(database, path, &mut dot_tracker)?;
        report(DamageScanProgress {
            completed: index + 1,
            total,
            current_file: path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned(),
            inserted,
            done: false,
        });
    }
    let connection = database.connect().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_tracker_last_scan_at',CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_tracker_files_scanned',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [paths.len().to_string()],
        )
        .map_err(|error| error.to_string())?;
    log_with(
        &connection,
        "info",
        "damage",
        &format!(
            "Rebuilt {inserted} combat and CH-chain events from {} character logs",
            paths.len()
        ),
    );
    report(DamageScanProgress {
        completed: total,
        total,
        current_file: String::new(),
        inserted,
        done: true,
    });
    Ok(json!({"files":paths.len(),"inserted":inserted}))
}

fn scan_history_directory(
    database: &Database,
    directory: &Path,
) -> Result<HistoryScanSummary, String> {
    let mut paths = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_log(path))
        .collect::<Vec<_>>();
    paths.sort();

    let mut summary = HistoryScanSummary::default();
    for path in paths {
        summary.files += 1;
        summary.inserted += scan_history_file(database, &path)?;
        summary.inserted += scan_death_report_file(database, &path)?;
        summary.inserted += scan_damage_file(database, &path)?;
    }

    let connection = database.connect().map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('activity_history_last_scan_at',CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('activity_history_files_scanned',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [summary.files.to_string()],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('death_reports_last_scan_at',CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('death_report_files_scanned',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [summary.files.to_string()],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_tracker_last_scan_at',CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('damage_tracker_files_scanned',?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [summary.files.to_string()],
        )
        .map_err(|error| error.to_string())?;
    if summary.inserted > 0 {
        log_with(
            &connection,
            "info",
            "history",
            &format!(
                "Archived {} event{} from {} character log{}",
                summary.inserted,
                if summary.inserted == 1 { "" } else { "s" },
                summary.files,
                if summary.files == 1 { "" } else { "s" }
            ),
        );
    }
    Ok(summary)
}

fn scan_history_file(database: &Database, path: &Path) -> Result<usize, String> {
    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    let source = path.display().to_string();
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let _writer_guard = database.writer_guard();
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let saved_offset = connection
        .query_row(
            "SELECT byte_offset FROM log_history_cursors WHERE source_file=?",
            [&source],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(0);
    let mut line_offset = if size < saved_offset { 0 } else { saved_offset };
    if size == line_offset {
        return Ok(0);
    }

    file.seek(SeekFrom::Start(line_offset))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut inserted = 0;
    let mut line_bytes = Vec::new();
    loop {
        line_bytes.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut line_bytes)
            .map_err(|error| error.to_string())?;
        if bytes_read == 0 || !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(&line_bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        if let Some(event) = parse_log_event(line, &character) {
            inserted += record_activity_event(
                &transaction,
                path,
                line_offset as i64,
                line,
                &character,
                &event,
            )?;
        }
        line_offset += bytes_read as u64;
    }
    transaction
        .execute(
            "INSERT INTO log_history_cursors(source_file,character_name,byte_offset,file_size,scanned_at)
             VALUES(?,?,?,?,CURRENT_TIMESTAMP)
             ON CONFLICT(source_file) DO UPDATE SET
                character_name=excluded.character_name,
                byte_offset=excluded.byte_offset,
                file_size=excluded.file_size,
                scanned_at=excluded.scanned_at",
            params![source, character, line_offset, size],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(inserted)
}

fn scan_death_report_file(database: &Database, path: &Path) -> Result<usize, String> {
    const CONTEXT_LINES: usize = 30;

    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    let source = path.display().to_string();
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let _writer_guard = database.writer_guard();
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let saved = connection
        .query_row(
            "SELECT byte_offset,context_json FROM death_report_scan_cursors WHERE source_file=?",
            [&source],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let (mut line_offset, mut context) = match saved {
        Some((offset, raw_context)) if offset <= size => {
            let lines = serde_json::from_str::<Vec<String>>(&raw_context).unwrap_or_default();
            (offset, VecDeque::from(lines))
        }
        _ => (0, VecDeque::new()),
    };
    while context.len() > CONTEXT_LINES {
        context.pop_front();
    }
    if size == line_offset {
        return Ok(0);
    }

    file.seek(SeekFrom::Start(line_offset))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let mut inserted = 0;
    let mut line_bytes = Vec::new();
    loop {
        line_bytes.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut line_bytes)
            .map_err(|error| error.to_string())?;
        if bytes_read == 0 || !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(&line_bytes);
        let line = text.trim_end_matches(['\r', '\n']).to_owned();
        if let Some(LogEvent::PlayerDeath {
            happened_at,
            killer_name,
        }) = parse_log_event(&line, &character)
        {
            let created = transaction
                .execute(
                    "INSERT OR IGNORE INTO death_reports(
                        happened_at,character_name,killer_name,raw_line,source_file,source_offset
                     ) VALUES(?,?,?,?,?,?)",
                    params![
                        happened_at.to_string(),
                        character,
                        killer_name,
                        line,
                        source,
                        line_offset as i64
                    ],
                )
                .map_err(|error| error.to_string())?;
            if created > 0 {
                let report_id = transaction.last_insert_rowid();
                for (index, context_line) in context.iter().enumerate() {
                    transaction
                        .execute(
                            "INSERT INTO death_report_entries(
                                death_report_id,sequence_number,raw_line
                             ) VALUES(?,?,?)",
                            params![report_id, index as i64 + 1, context_line],
                        )
                        .map_err(|error| error.to_string())?;
                }
                inserted += 1;
            }
        }
        context.push_back(line);
        while context.len() > CONTEXT_LINES {
            context.pop_front();
        }
        line_offset += bytes_read as u64;
    }

    let context_json = serde_json::to_string(&context.iter().cloned().collect::<Vec<_>>())
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO death_report_scan_cursors(
                source_file,character_name,byte_offset,file_size,context_json,scanned_at
             ) VALUES(?,?,?,?,?,CURRENT_TIMESTAMP)
             ON CONFLICT(source_file) DO UPDATE SET
                character_name=excluded.character_name,
                byte_offset=excluded.byte_offset,
                file_size=excluded.file_size,
                context_json=excluded.context_json,
                scanned_at=excluded.scanned_at",
            params![source, character, line_offset, size, context_json],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(inserted)
}

fn scan_damage_file(database: &Database, path: &Path) -> Result<usize, String> {
    scan_damage_file_internal(database, path, None)
}

pub(super) fn scan_damage_file_with_dots(
    database: &Database,
    path: &Path,
    dot_tracker: &mut DotTracker,
) -> Result<usize, String> {
    scan_damage_file_internal(database, path, Some(dot_tracker))
}

fn scan_damage_file_internal(
    database: &Database,
    path: &Path,
    mut dot_tracker: Option<&mut DotTracker>,
) -> Result<usize, String> {
    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    let source = path.display().to_string();
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let _writer_guard = database.writer_guard();
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let saved_offset = connection
        .query_row(
            "SELECT byte_offset FROM damage_scan_cursors WHERE source_file=?",
            [&source],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .unwrap_or(0);
    let mut line_offset = if size < saved_offset { 0 } else { saved_offset };
    if size == line_offset {
        return Ok(0);
    }

    file.seek(SeekFrom::Start(line_offset))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(file);
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let suppressed_ranges =
        load_suppressed_combat_ranges(&transaction, &source, line_offset as i64)?;
    let mut inserted = 0;
    let mut line_bytes = Vec::new();
    loop {
        line_bytes.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut line_bytes)
            .map_err(|error| error.to_string())?;
        if bytes_read == 0 || !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(&line_bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        let event = parse_log_event(line, &character);
        let suppressed = offset_in_ranges(&suppressed_ranges, line_offset as i64);
        if let Some(event) = event
            .clone()
            .filter(|event| !suppressed || !is_combat_event(event))
        {
            match event {
                LogEvent::Damage {
                    happened_at,
                    attacker_name,
                    mob_name,
                    attack,
                    amount,
                    damage_type,
                } => {
                    inserted += record_damage_event(
                        &transaction,
                        &source,
                        line_offset as i64,
                        line,
                        &character,
                        &attacker_name,
                        happened_at,
                        &mob_name,
                        &attack,
                        amount,
                        damage_type.as_str(),
                    )?;
                }
                LogEvent::PetAttack {
                    happened_at,
                    pet_name,
                    target_name,
                } => {
                    inserted += record_pet_evidence(
                        &transaction,
                        &source,
                        line_offset as i64,
                        &character,
                        happened_at,
                        &pet_name,
                        &target_name,
                    )?;
                }
                LogEvent::ObservedMelee {
                    happened_at,
                    subject_name,
                    target_name,
                    attack,
                    amount,
                } => {
                    inserted += record_observed_melee_event(
                        &transaction,
                        &source,
                        line_offset as i64,
                        line,
                        &character,
                        happened_at,
                        &subject_name,
                        &target_name,
                        &attack,
                        amount,
                    )?;
                }
                LogEvent::IncomingDamage {
                    happened_at,
                    attacker_name,
                    target_name,
                    attack,
                    amount,
                } => {
                    inserted += record_incoming_damage_event(
                        &transaction,
                        &source,
                        line_offset as i64,
                        line,
                        &character,
                        happened_at,
                        &attacker_name,
                        &target_name,
                        &attack,
                        amount,
                    )?;
                }
                LogEvent::ClericHealCall {
                    happened_at,
                    cleric_name,
                    call_number,
                    target_name,
                    channel,
                    message,
                } => {
                    inserted += record_cleric_heal_call(
                        &transaction,
                        &source,
                        line_offset as i64,
                        line,
                        &character,
                        happened_at,
                        &cleric_name,
                        call_number,
                        target_name.as_deref(),
                        channel.as_str(),
                        &message,
                    )?;
                }
                LogEvent::GuildSlowCall {
                    happened_at,
                    speaker_name,
                    mob_name,
                    message,
                } => {
                    inserted += record_guild_slow_call(
                        &transaction,
                        &source,
                        line_offset as i64,
                        line,
                        &character,
                        happened_at,
                        &speaker_name,
                        &mob_name,
                        &message,
                    )?;
                }
                LogEvent::MobSlain {
                    happened_at,
                    mob_name,
                    ..
                } => {
                    transaction
                        .execute(
                            "UPDATE damage_encounters
                             SET outcome='slain',ended_at=?,last_source_offset=MAX(last_source_offset,?)
                             WHERE id=(
                                SELECT id FROM damage_encounters
                                WHERE source_file=? AND character_name=? COLLATE NOCASE
                                  AND mob_name=? COLLATE NOCASE AND outcome='active'
                                ORDER BY last_source_offset DESC LIMIT 1
                             )",
                            params![
                                happened_at.to_string(),
                                line_offset as i64,
                                source,
                                character,
                                mob_name
                            ],
                        )
                        .map_err(|error| error.to_string())?;
                }
                LogEvent::PlayerDeath { happened_at, .. } => {
                    transaction
                        .execute(
                            "UPDATE damage_encounters
                             SET outcome='playerDeath',ended_at=?,last_source_offset=MAX(last_source_offset,?)
                             WHERE source_file=? AND character_name=? COLLATE NOCASE AND outcome='active'",
                            params![
                                happened_at.to_string(),
                                line_offset as i64,
                                source,
                                character
                            ],
                        )
                        .map_err(|error| error.to_string())?;
                }
                _ => {}
            }
        }
        if !suppressed {
            if let Some(tracker) = dot_tracker.as_deref_mut() {
                inserted += tracker.process_line(
                    &transaction,
                    &source,
                    line_offset as i64,
                    line,
                    &character,
                    event.as_ref(),
                )?;
            }
        }
        line_offset += bytes_read as u64;
    }

    transaction
        .execute(
            "INSERT INTO damage_scan_cursors(
                source_file,character_name,byte_offset,file_size,scanned_at
             ) VALUES(?,?,?,?,CURRENT_TIMESTAMP)
             ON CONFLICT(source_file) DO UPDATE SET
                character_name=excluded.character_name,
                byte_offset=excluded.byte_offset,
                file_size=excluded.file_size,
                scanned_at=excluded.scanned_at",
            params![source, character, line_offset, size],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(inserted)
}

#[allow(clippy::too_many_arguments)]
fn record_cleric_heal_call(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    raw: &str,
    character: &str,
    happened_at: NaiveDateTime,
    cleric_name: &str,
    call_number: u32,
    target_name: Option<&str>,
    channel: &str,
    message: &str,
) -> Result<usize, String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO cleric_heal_calls(
                happened_at,character_name,cleric_name,call_number,target_name,channel,
                message,raw_line,source_file,source_offset
             ) VALUES(?,?,?,?,?,?,?,?,?,?)",
            params![
                happened_at.to_string(),
                character,
                cleric_name,
                call_number as i64,
                target_name,
                channel,
                message,
                raw,
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
fn record_guild_slow_call(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    raw: &str,
    character: &str,
    happened_at: NaiveDateTime,
    speaker_name: &str,
    mob_name: &str,
    message: &str,
) -> Result<usize, String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO guild_slow_calls(
                happened_at,character_name,speaker_name,mob_name,message,raw_line,source_file,source_offset
             ) VALUES(?,?,?,?,?,?,?,?)",
            params![
                happened_at.to_string(),
                character,
                speaker_name,
                mob_name,
                message,
                raw,
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())
}

fn record_pet_evidence(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    character: &str,
    happened_at: NaiveDateTime,
    pet_name: &str,
    target_name: &str,
) -> Result<usize, String> {
    connection
        .execute(
            "INSERT OR IGNORE INTO combat_pet_evidence(
                character_name,pet_name,target_name,last_seen_at,source_file,source_offset
             ) VALUES(?,?,?,?,?,?)",
            params![
                character,
                pet_name,
                target_name,
                happened_at.to_string(),
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
fn record_observed_melee_event(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    raw: &str,
    character: &str,
    happened_at: NaiveDateTime,
    subject_name: &str,
    target_name: &str,
    attack: &str,
    amount: u64,
) -> Result<usize, String> {
    let known_target = target_name.eq_ignore_ascii_case(character)
        || connection
            .query_row(
                "SELECT 1 FROM known_members WHERE name=? COLLATE NOCASE",
                [target_name],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some()
        || connection
            .query_row(
                "SELECT 1 FROM combat_pet_evidence
                 WHERE source_file=? AND character_name=? COLLATE NOCASE
                   AND pet_name=? COLLATE NOCASE AND last_seen_at>=datetime(?,'-10 minutes')
                 ORDER BY last_seen_at DESC LIMIT 1",
                params![source, character, target_name, happened_at.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some();
    let known_subject = subject_name.eq_ignore_ascii_case(character)
        || connection
            .query_row(
                "SELECT 1 FROM known_members WHERE name=? COLLATE NOCASE",
                [subject_name],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some()
        || connection
            .query_row(
                "SELECT 1 FROM combat_pet_evidence
                 WHERE source_file=? AND character_name=? COLLATE NOCASE
                   AND pet_name=? COLLATE NOCASE AND last_seen_at>=datetime(?,'-10 minutes')
                 ORDER BY last_seen_at DESC LIMIT 1",
                params![source, character, subject_name, happened_at.to_string()],
                |_| Ok(()),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .is_some();
    let attacks_active_target = connection
        .query_row(
            "SELECT 1 FROM damage_encounters
             WHERE source_file=? AND character_name=? COLLATE NOCASE
               AND mob_name=? COLLATE NOCASE AND outcome='active'
             ORDER BY last_source_offset DESC LIMIT 1",
            params![source, character, target_name],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    let subject_is_active_mob = connection
        .query_row(
            "SELECT 1 FROM damage_encounters
             WHERE source_file=? AND character_name=? COLLATE NOCASE
               AND mob_name=? COLLATE NOCASE AND outcome='active'
             ORDER BY last_source_offset DESC LIMIT 1",
            params![source, character, subject_name],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    let subject_lower = subject_name.to_ascii_lowercase();
    let looks_like_mob = subject_name.contains(' ')
        || subject_lower.starts_with("a ")
        || subject_lower.starts_with("an ")
        || subject_lower.starts_with("the ");
    let target_looks_like_player =
        !target_name.contains(' ') && target_name.chars().next().is_some_and(char::is_uppercase);
    // Target identity wins over stale or simultaneous attacker encounters. Once X is an
    // active target, every observed hit against X belongs on X's outgoing DPS meter.
    if attacks_active_target {
        record_damage_event(
            connection,
            source,
            source_offset,
            raw,
            character,
            subject_name,
            happened_at,
            target_name,
            attack,
            amount,
            "melee",
        )
    } else if known_target || subject_is_active_mob {
        record_incoming_damage_event(
            connection,
            source,
            source_offset,
            raw,
            character,
            happened_at,
            subject_name,
            target_name,
            attack,
            amount,
        )
    } else if known_subject {
        record_damage_event(
            connection,
            source,
            source_offset,
            raw,
            character,
            subject_name,
            happened_at,
            target_name,
            attack,
            amount,
            "melee",
        )
    } else if looks_like_mob && target_looks_like_player {
        record_incoming_damage_event(
            connection,
            source,
            source_offset,
            raw,
            character,
            happened_at,
            subject_name,
            target_name,
            attack,
            amount,
        )
    } else {
        record_damage_event(
            connection,
            source,
            source_offset,
            raw,
            character,
            subject_name,
            happened_at,
            target_name,
            attack,
            amount,
            "melee",
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn record_incoming_damage_event(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    raw: &str,
    character: &str,
    happened_at: NaiveDateTime,
    attacker_name: &str,
    target_name: &str,
    attack: &str,
    amount: u64,
) -> Result<usize, String> {
    const ENCOUNTER_GAP_SECONDS: i64 = 120;
    let already_recorded = connection
        .query_row(
            "SELECT 1 FROM damage_received_events WHERE source_file=? AND source_offset=?",
            params![source, source_offset],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if already_recorded {
        return Ok(0);
    }
    let active = connection
        .query_row(
            "SELECT id,last_damage_at FROM damage_encounters
             WHERE source_file=? AND character_name=? COLLATE NOCASE
               AND mob_name=? COLLATE NOCASE AND outcome='active'
             ORDER BY last_source_offset DESC LIMIT 1",
            params![source, character, attacker_name],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut encounter_id = active.as_ref().map(|value| value.0);
    if let Some((id, last_damage_at)) = active {
        let last = NaiveDateTime::parse_from_str(&last_damage_at, "%Y-%m-%d %H:%M:%S")
            .map_err(|error| error.to_string())?;
        let gap = happened_at.signed_duration_since(last).num_seconds();
        if !(0..=ENCOUNTER_GAP_SECONDS).contains(&gap) {
            connection
                .execute(
                    "UPDATE damage_encounters
                     SET outcome='disengaged',ended_at=last_damage_at WHERE id=?",
                    [id],
                )
                .map_err(|error| error.to_string())?;
            encounter_id = None;
        }
    }
    let encounter_id = match encounter_id {
        Some(id) => id,
        None => insert_or_get_damage_encounter(
            connection,
            source,
            source_offset,
            character,
            happened_at,
            attacker_name,
        )?,
    };
    let created = connection
        .execute(
            "INSERT OR IGNORE INTO damage_received_events(
                encounter_id,happened_at,attacker_name,target_name,attack_kind,damage,
                raw_line,source_file,source_offset
             ) VALUES(?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                happened_at.to_string(),
                attacker_name,
                target_name,
                attack,
                amount as i64,
                raw,
                source,
                source_offset
            ],
        )
        .map_err(|error| error.to_string())?;
    if created > 0 {
        connection
            .execute(
                "UPDATE damage_encounters
                 SET last_damage_at=?,last_source_offset=MAX(last_source_offset,?)
                 WHERE id=?",
                params![happened_at.to_string(), source_offset, encounter_id],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(created)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn record_damage_event(
    connection: &rusqlite::Connection,
    source: &str,
    source_offset: i64,
    raw: &str,
    character: &str,
    attacker_name: &str,
    happened_at: NaiveDateTime,
    mob_name: &str,
    attack: &str,
    amount: u64,
    damage_type: &str,
) -> Result<usize, String> {
    const ENCOUNTER_GAP_SECONDS: i64 = 120;
    let already_recorded = connection
        .query_row(
            "SELECT 1 FROM damage_events WHERE source_file=? AND source_offset=?",
            params![source, source_offset],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .is_some();
    if already_recorded {
        return Ok(0);
    }

    // Generic non-melee lines do not identify their caster. Keep them only when a
    // recent landing was explicitly attributed to this log's active character.
    // Confirmed procs already write their catalog damage through proc:// events.
    let resolved_attack = if damage_type == "spell" && attack.eq_ignore_ascii_case("non-melee") {
        let window_start = happened_at - ChronoDuration::seconds(6);
        let confirmed = connection
            .query_row(
                "SELECT spell_name,source_kind FROM combat_spell_activity
                 WHERE source_file=? AND target_name=? COLLATE NOCASE
                   AND caster_name=? COLLATE NOCASE AND happened_at BETWEEN ? AND ?
                   AND source_kind IN ('direct','item_click','proc')
                 ORDER BY happened_at DESC,id DESC LIMIT 1",
                params![
                    source,
                    mob_name,
                    character,
                    window_start.to_string(),
                    happened_at.to_string()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        match confirmed {
            Some((_, source_kind)) if source_kind == "proc" => return Ok(0),
            Some((spell_name, _)) => Some(spell_name),
            None => return Ok(0),
        }
    } else {
        None
    };
    let attack = resolved_attack.as_deref().unwrap_or(attack);

    let active = connection
        .query_row(
            "SELECT id,last_damage_at FROM damage_encounters
             WHERE source_file=? AND character_name=? COLLATE NOCASE
               AND mob_name=? COLLATE NOCASE AND outcome='active'
             ORDER BY last_source_offset DESC LIMIT 1",
            params![source, character, mob_name],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let mut encounter_id = active.as_ref().map(|value| value.0);
    if let Some((id, last_damage_at)) = active {
        let last = NaiveDateTime::parse_from_str(&last_damage_at, "%Y-%m-%d %H:%M:%S")
            .map_err(|error| error.to_string())?;
        let gap = happened_at.signed_duration_since(last).num_seconds();
        if !(0..=ENCOUNTER_GAP_SECONDS).contains(&gap) {
            connection
                .execute(
                    "UPDATE damage_encounters
                     SET outcome='disengaged',ended_at=last_damage_at WHERE id=?",
                    [id],
                )
                .map_err(|error| error.to_string())?;
            encounter_id = None;
        }
    }
    let encounter_id = match encounter_id {
        Some(id) => id,
        None => insert_or_get_damage_encounter(
            connection,
            source,
            source_offset,
            character,
            happened_at,
            mob_name,
        )?,
    };
    let happened_at_text = happened_at.to_string();
    let weapon_loadout_id = if attacker_name.eq_ignore_ascii_case(character) {
        connection
            .query_row(
                "SELECT id FROM character_weapon_loadouts
                 WHERE character_name=? COLLATE NOCASE AND captured_at<=?
                 ORDER BY captured_at DESC,id DESC LIMIT 1",
                params![character, happened_at_text],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
    } else {
        None
    };
    let created = connection
        .execute(
            "INSERT OR IGNORE INTO damage_events(
                encounter_id,happened_at,damage_type,attack_kind,damage,raw_line,source_file,
                source_offset,weapon_loadout_id,attacker_name
             ) VALUES(?,?,?,?,?,?,?,?,?,?)",
            params![
                encounter_id,
                happened_at_text,
                damage_type,
                attack,
                amount as i64,
                raw,
                source,
                source_offset,
                weapon_loadout_id,
                attacker_name
            ],
        )
        .map_err(|error| error.to_string())?;
    if created > 0 {
        let melee = if damage_type == "melee" {
            amount as i64
        } else {
            0
        };
        let spell = if damage_type == "spell" {
            amount as i64
        } else {
            0
        };
        connection
            .execute(
                "UPDATE damage_encounters SET
                    last_damage_at=?,last_source_offset=?,
                    total_damage=total_damage+?,melee_damage=melee_damage+?,
                    spell_damage=spell_damage+?,hit_count=hit_count+1,max_hit=MAX(max_hit,?)
                 WHERE id=?",
                params![
                    happened_at.to_string(),
                    source_offset,
                    amount as i64,
                    melee,
                    spell,
                    amount as i64,
                    encounter_id
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(created)
}

fn record_activity_event(
    connection: &rusqlite::Connection,
    path: &Path,
    source_offset: i64,
    raw: &str,
    character: &str,
    event: &LogEvent,
) -> Result<usize, String> {
    let source = path.display().to_string();
    match event {
        LogEvent::Loot {
            happened_at,
            looter,
            item_name,
        } => connection
            .execute(
                "INSERT OR IGNORE INTO activity_loot_history(
                    happened_at,character_name,item_name,looter_name,raw_line,source_file,source_offset
                 ) VALUES(?,?,?,?,?,?,?)",
                params![
                    happened_at.to_string(),
                    character,
                    item_name,
                    looter,
                    raw,
                    source,
                    source_offset
                ],
            )
            .map_err(|error| error.to_string()),
        LogEvent::MobSlain {
            happened_at,
            mob_name,
            killer,
        } => connection
            .execute(
                "INSERT OR IGNORE INTO activity_mob_history(
                    happened_at,character_name,mob_name,killer_name,raw_line,source_file,source_offset
                 ) VALUES(?,?,?,?,?,?,?)",
                params![
                    happened_at.to_string(),
                    character,
                    mob_name,
                    killer,
                    raw,
                    source,
                    source_offset
                ],
            )
            .map_err(|error| error.to_string()),
        LogEvent::LevelChanged {
            happened_at,
            level,
            direction,
        } => connection
            .execute(
                "INSERT OR IGNORE INTO activity_level_history(
                    happened_at,character_name,level,direction,raw_line,source_file,source_offset
                 ) VALUES(?,?,?,?,?,?,?)",
                params![
                    happened_at.to_string(),
                    character,
                    level,
                    direction.as_str(),
                    raw,
                    source,
                    source_offset
                ],
            )
            .map_err(|error| error.to_string()),
        LogEvent::TradeOffer {
            happened_at,
            offerer,
            message,
            item_names,
        } => {
            let items = if item_names.is_empty() {
                resolve_linked_items(connection, message, item_names)?
            } else {
                item_names.clone()
            };
            let mut inserted = 0;
            for (item_index, item_name) in items.iter().enumerate() {
                inserted += connection
                    .execute(
                        "INSERT OR IGNORE INTO activity_offer_history(
                            happened_at,character_name,offerer_name,item_name,item_index,raw_line,source_file,source_offset
                         ) VALUES(?,?,?,?,?,?,?,?)",
                        params![
                            happened_at.to_string(),
                            character,
                            offerer,
                            item_name,
                            item_index as i64,
                            raw,
                            source,
                            source_offset
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            Ok(inserted)
        }
        _ => Ok(0),
    }
}

#[cfg(test)]
fn process_log(
    database: &Database,
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
) -> Result<bool, String> {
    process_log_internal(database, path, offsets, last_mob, None)
}

fn process_log_with_dots(
    database: &Database,
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    dot_tracker: &mut DotTracker,
) -> Result<bool, String> {
    process_log_internal(database, path, offsets, last_mob, Some(dot_tracker))
}

fn process_log_internal(
    database: &Database,
    path: &Path,
    offsets: &mut HashMap<PathBuf, u64>,
    last_mob: &mut HashMap<PathBuf, String>,
    mut dot_tracker: Option<&mut DotTracker>,
) -> Result<bool, String> {
    let before = log_data_signature(database)?;
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let size = file.metadata().map_err(|error| error.to_string())?.len();
    let offset = offsets.entry(path.to_owned()).or_insert(size);
    if size < *offset {
        *offset = 0
    }
    if size == *offset {
        return Ok(false);
    }
    let mut line_offset = *offset;
    file.seek(SeekFrom::Start(line_offset))
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    let _writer_guard = database.writer_guard();
    let mut connection = database.connect().map_err(|error| error.to_string())?;
    let transaction = connection
        .transaction()
        .map_err(|error| error.to_string())?;
    let source_file = path.display().to_string();
    let purged_ranges =
        load_suppressed_combat_ranges(&transaction, &source_file, line_offset as i64)?;
    for line_bytes in bytes.split_inclusive(|byte| *byte == b'\n') {
        if !line_bytes.ends_with(b"\n") {
            break;
        }
        let text = String::from_utf8_lossy(line_bytes);
        let line = text.trim_end_matches(['\r', '\n']);
        let event = parse_log_event(line, &character);
        let purged = offset_in_ranges(&purged_ranges, line_offset as i64);
        if let Some(event) = event
            .as_ref()
            .filter(|event| !purged || !is_combat_event(event))
        {
            apply_event(
                &transaction,
                path,
                line_offset as i64,
                line,
                event,
                last_mob,
            )?;
        } else if line.to_ascii_lowercase().contains(" looted ") {
            log_with(
                &transaction,
                "warning",
                "parser",
                &format!("Unrecognized loot line in {}: {line}", path.display()),
            );
        }
        if !purged {
            if let Some(tracker) = dot_tracker.as_deref_mut() {
                tracker.process_line(
                    &transaction,
                    &path.display().to_string(),
                    line_offset as i64,
                    line,
                    &character,
                    event.as_ref(),
                )?;
            }
        }
        line_offset += line_bytes.len() as u64;
    }
    transaction.execute(
        "INSERT INTO live_log_cursors(source_file,byte_offset,file_size,updated_at)
         VALUES(?,?,?,CURRENT_TIMESTAMP)
         ON CONFLICT(source_file) DO UPDATE SET
             byte_offset=excluded.byte_offset,file_size=excluded.file_size,updated_at=CURRENT_TIMESTAMP",
        params![path.display().to_string(),line_offset as i64,size as i64],
    ).map_err(|error|error.to_string())?;
    transaction.execute(
        "INSERT INTO app_settings(key,value) VALUES('active_log_path',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [path.display().to_string()],
    ).map_err(|error|error.to_string())?;
    transaction.execute(
        "INSERT INTO app_settings(key,value) VALUES('active_log_offset',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [line_offset.to_string()],
    ).map_err(|error|error.to_string())?;
    transaction.execute(
        "INSERT INTO app_settings(key,value) VALUES('last_log_read_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [],
    ).map_err(|error|error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    *offset = line_offset;
    Ok(before != log_data_signature(database)?)
}

fn offset_in_ranges(ranges: &[(i64, i64)], offset: i64) -> bool {
    let index = ranges.partition_point(|(_, last)| *last < offset);
    ranges
        .get(index)
        .is_some_and(|(first, last)| *first <= offset && offset <= *last)
}

fn load_suppressed_combat_ranges(
    connection: &rusqlite::Connection,
    source_file: &str,
    from_offset: i64,
) -> Result<Vec<(i64, i64)>, String> {
    let mut statement = connection
        .prepare(
            "SELECT first_source_offset,last_source_offset FROM purged_damage_encounter_ranges
             WHERE source_file=? AND last_source_offset>=?
             UNION ALL
             SELECT first_source_offset,last_source_offset FROM damage_encounters
             WHERE source_file=? AND is_protected=1 AND last_source_offset>=?
             ORDER BY first_source_offset",
        )
        .map_err(|error| error.to_string())?;
    let ranges = statement
        .query_map(
            params![source_file, from_offset, source_file, from_offset],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(ranges)
}

fn is_combat_event(event: &LogEvent) -> bool {
    matches!(
        event,
        LogEvent::Damage { .. }
            | LogEvent::ObservedMelee { .. }
            | LogEvent::IncomingDamage { .. }
            | LogEvent::ItemGlow { .. }
            | LogEvent::SpellCastStarted { .. }
            | LogEvent::SpellInterrupted { .. }
            | LogEvent::SpellResisted { .. }
            | LogEvent::CombatAttempt { .. }
            | LogEvent::PetAttack { .. }
    )
}
fn initial_live_offset(database: &Database, path: &Path, size: u64) -> Result<u64, String> {
    let source_file = path.display().to_string();
    let connection = database.connect().map_err(|error| error.to_string())?;
    let saved = connection
        .query_row(
            "SELECT byte_offset FROM live_log_cursors WHERE source_file=?",
            [&source_file],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let offset = match saved {
        Some(saved) if saved <= size => saved,
        Some(_) => 0,
        None => connection
            .query_row(
                "SELECT MAX(source_offset) FROM loot_drops WHERE source_file=?",
                [&source_file],
                |row| row.get::<_, Option<u64>>(0),
            )
            .map_err(|error| error.to_string())?
            .unwrap_or(size),
    };

    connection
        .execute(
            "INSERT INTO live_log_cursors(source_file,byte_offset,file_size,updated_at)
             VALUES(?,?,?,CURRENT_TIMESTAMP)
             ON CONFLICT(source_file) DO UPDATE SET
                 byte_offset=excluded.byte_offset,
                 file_size=excluded.file_size,
                 updated_at=CURRENT_TIMESTAMP",
            params![source_file, offset as i64, size as i64],
        )
        .map_err(|error| error.to_string())?;
    Ok(offset)
}

type LogDataSignature = (i64, i64, i64, i64, i64, i64, i64, i64, i64, String, i64);

fn log_data_signature(database: &Database) -> Result<LogDataSignature, String> {
    database.connect().map_err(|error| error.to_string())?.query_row(
        "SELECT
            COALESCE((SELECT MAX(id) FROM loot_drops),0),
            COALESCE((SELECT MAX(id) FROM linked_loot_items),0),
            COALESCE((SELECT MAX(id) FROM merchant_messages),0),
            COALESCE((SELECT MAX(id) FROM mobs),0),
            COALESCE((SELECT MAX(id) FROM application_logs),0),
            COALESCE((SELECT MAX(id) FROM damage_events),0),
            COALESCE((SELECT MAX(id) FROM damage_received_events),0),
            COALESCE((SELECT MAX(id) FROM cleric_heal_calls),0) +
                COALESCE((SELECT MAX(id) FROM guild_slow_calls),0),
            COALESCE((SELECT SUM(last_source_offset) FROM damage_encounters),0),
            COALESCE((SELECT GROUP_CONCAT(member_id, ',') FROM (SELECT member_id FROM current_group ORDER BY member_id)),''),
            COALESCE((SELECT SUM(id+ticks_applied+CASE WHEN status='active' THEN 1 ELSE 0 END) FROM dot_applications),0)",
        [],
        |row| Ok((
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
        )),
    ).map_err(|error| error.to_string())
}

fn apply_event(
    connection: &rusqlite::Connection,
    path: &Path,
    source_offset: i64,
    raw: &str,
    event: &LogEvent,
    last_mob: &mut HashMap<PathBuf, String>,
) -> Result<(), String> {
    let c = connection;
    let character = character_from_log(path).unwrap_or_else(|| "Unknown".into());
    record_activity_event(c, path, source_offset, raw, &character, event)?;
    match event {
        LogEvent::MobSlain {
            happened_at,
            mob_name,
            ..
        } => {
            c.execute(
                "INSERT INTO mobs(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                [mob_name],
            )
            .map_err(|e| e.to_string())?;
            last_mob.insert(path.to_owned(), mob_name.clone());
            c.execute(
                "UPDATE damage_encounters
                 SET outcome='slain',ended_at=?,last_source_offset=MAX(last_source_offset,?)
                 WHERE id=(
                    SELECT id FROM damage_encounters
                    WHERE source_file=? AND character_name=? COLLATE NOCASE
                      AND mob_name=? COLLATE NOCASE AND outcome='active'
                    ORDER BY last_source_offset DESC LIMIT 1
                 )",
                params![
                    happened_at.to_string(),
                    source_offset,
                    path.display().to_string(),
                    character,
                    mob_name
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        LogEvent::GroupChange {
            character, change, ..
        } => {
            c.execute(
                "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                [character],
            )
            .map_err(|e| e.to_string())?;
            match change {
                GroupChangeKind::Left => {
                    c.execute("DELETE FROM current_group WHERE member_id=(SELECT id FROM known_members WHERE name=? COLLATE NOCASE)",[character]).map_err(|e|e.to_string())?;
                }
                _ => {
                    c.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",[character]).map_err(|e|e.to_string())?;
                    if let Some(local_character) = character_from_log(path) {
                        c.execute(
                            "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                            [&local_character],
                        )
                        .map_err(|e| e.to_string())?;
                        c.execute("INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",[&local_character]).map_err(|e|e.to_string())?;
                    }
                }
            }
        }
        LogEvent::GroupCleared { .. } => {
            c.execute("DELETE FROM current_group", [])
                .map_err(|e| e.to_string())?;
            log_with(
                c,
                "info",
                "group",
                "Local player was removed; current group cleared",
            );
        }
        LogEvent::Loot {
            happened_at,
            looter,
            item_name,
        } => {
            let source = path.display().to_string();
            let inserted = c.execute("INSERT OR IGNORE INTO loot_drops(happened_at,item_name,mob_name,looter_name,raw_line,source_file,source_offset) VALUES(?,?,?,?,?,?,?)",params![happened_at.to_string(),item_name,last_mob.get(path),looter,raw,source,source_offset]).map_err(|e|e.to_string())?;
            if inserted > 0 {
                let id = c.last_insert_rowid();
                c.execute("INSERT OR IGNORE INTO loot_drop_members(loot_drop_id,member_name) SELECT ?,m.name FROM current_group g JOIN known_members m ON m.id=g.member_id",[id]).map_err(|e|e.to_string())?;
                log_with(c, "info", "loot", &format!("{looter} looted {item_name}"));
            }
        }
        LogEvent::MerchantListing {
            happened_at,
            speaker,
            action,
            message,
        } => {
            if !merchant_mode_enabled(c) {
                return Ok(());
            }
            let source = path.display().to_string();
            let inserted = c
                .execute(
                    "INSERT OR IGNORE INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(?,?,?,?,?,?,?)",
                    params![happened_at.to_string(), action.as_str(), speaker, message, raw, source, source_offset],
                )
                .map_err(|error| error.to_string())?;
            if inserted > 0 {
                let message_id = c.last_insert_rowid();
                let catalog = merchant_catalog(c)?;
                for (order, item) in parse_listing_items(message, &catalog).iter().enumerate() {
                    c.execute(
                        "INSERT INTO merchant_message_items(merchant_message_id,item_name,item_id,asking_price_pp,sort_order) VALUES(?,?,?,?,?)",
                        params![message_id, item.item_name, item.item_id, item.asking_price_pp, order as i64],
                    )
                    .map_err(|error| error.to_string())?;
                }
                finish_merchant_capture(c)?;
            }
        }
        LogEvent::PetAttack {
            happened_at,
            pet_name,
            target_name,
        } => {
            record_pet_evidence(
                c,
                &path.display().to_string(),
                source_offset,
                &character,
                *happened_at,
                pet_name,
                target_name,
            )?;
        }
        LogEvent::DirectTell {
            happened_at,
            speaker,
            message,
        } => {
            if !merchant_mode_enabled(c) {
                return Ok(());
            }
            let source = path.display().to_string();
            let inserted = c
                .execute(
                    "INSERT OR IGNORE INTO merchant_messages(happened_at,kind,speaker_name,message,raw_line,source_file,source_offset) VALUES(?,'tell',?,?,?,?,?)",
                    params![happened_at.to_string(), speaker, message, raw, source, source_offset],
                )
                .map_err(|error| error.to_string())?;
            if inserted > 0 {
                finish_merchant_capture(c)?;
            }
        }
        LogEvent::Damage {
            happened_at,
            attacker_name,
            mob_name,
            attack,
            amount,
            damage_type,
        } => {
            record_damage_event(
                c,
                &path.display().to_string(),
                source_offset,
                raw,
                &character,
                attacker_name,
                *happened_at,
                mob_name,
                attack,
                *amount,
                damage_type.as_str(),
            )?;
        }
        LogEvent::ObservedMelee {
            happened_at,
            subject_name,
            target_name,
            attack,
            amount,
        } => {
            record_observed_melee_event(
                c,
                &path.display().to_string(),
                source_offset,
                raw,
                &character,
                *happened_at,
                subject_name,
                target_name,
                attack,
                *amount,
            )?;
        }
        LogEvent::IncomingDamage {
            happened_at,
            attacker_name,
            target_name,
            attack,
            amount,
        } => {
            record_incoming_damage_event(
                c,
                &path.display().to_string(),
                source_offset,
                raw,
                &character,
                *happened_at,
                attacker_name,
                target_name,
                attack,
                *amount,
            )?;
        }
        LogEvent::ClericHealCall {
            happened_at,
            cleric_name,
            call_number,
            target_name,
            channel,
            message,
        } => {
            record_cleric_heal_call(
                c,
                &path.display().to_string(),
                source_offset,
                raw,
                &character,
                *happened_at,
                cleric_name,
                *call_number,
                target_name.as_deref(),
                channel.as_str(),
                message,
            )?;
            if *channel == ChatChannel::Group {
                c.execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [cleric_name],
                )
                .map_err(|error| error.to_string())?;
                c.execute(
                    "INSERT OR IGNORE INTO current_group(member_id)
                     SELECT id FROM known_members WHERE name=? COLLATE NOCASE",
                    [cleric_name],
                )
                .map_err(|error| error.to_string())?;
            }
        }
        LogEvent::GuildSlowCall {
            happened_at,
            speaker_name,
            mob_name,
            message,
        } => {
            record_guild_slow_call(
                c,
                &path.display().to_string(),
                source_offset,
                raw,
                &character,
                *happened_at,
                speaker_name,
                mob_name,
                message,
            )?;
        }
        LogEvent::PlayerDeath { happened_at, .. } => {
            c.execute(
                "UPDATE damage_encounters
                 SET outcome='playerDeath',ended_at=?,last_source_offset=MAX(last_source_offset,?)
                 WHERE source_file=? AND character_name=? COLLATE NOCASE AND outcome='active'",
                params![
                    happened_at.to_string(),
                    source_offset,
                    path.display().to_string(),
                    character
                ],
            )
            .map_err(|error| error.to_string())?;
        }
        LogEvent::TradeOffer { .. } | LogEvent::LevelChanged { .. } => {}
        LogEvent::LinkedItems {
            happened_at,
            speaker,
            channel,
            message,
            item_names,
        } => {
            if *channel == ChatChannel::Group {
                c.execute(
                    "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                    [speaker],
                )
                .map_err(|error| error.to_string())?;
                c.execute(
                    "INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",
                    [speaker],
                )
                .map_err(|error| error.to_string())?;
                if let Some(local_character) = character_from_log(path) {
                    c.execute(
                        "INSERT INTO known_members(name) VALUES(?) ON CONFLICT(name) DO NOTHING",
                        [&local_character],
                    )
                    .map_err(|error| error.to_string())?;
                    c.execute(
                        "INSERT OR IGNORE INTO current_group(member_id) SELECT id FROM known_members WHERE name=? COLLATE NOCASE",
                        [&local_character],
                    )
                    .map_err(|error| error.to_string())?;
                }
            }
            let source = path.display().to_string();
            let resolved_items = resolve_linked_items(c, message, item_names)?;
            let mut inserted = 0;
            for (link_index, item_name) in resolved_items.iter().enumerate() {
                inserted += c
                    .execute(
                        "INSERT OR IGNORE INTO linked_loot_items(happened_at,channel,speaker_name,item_name,raw_line,source_file,source_offset,link_index)
                         VALUES(?,?,?,?,?,?,?,?)",
                        params![
                            happened_at.to_string(),
                            channel.as_str(),
                            speaker,
                            item_name,
                            raw,
                            source,
                            source_offset,
                            link_index as i64
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
            if inserted > 0 {
                log_with(
                    c,
                    "info",
                    "linked-loot",
                    &format!(
                        "{speaker} linked {} item{} in {} chat",
                        inserted,
                        if inserted == 1 { "" } else { "s" },
                        channel.as_str()
                    ),
                );
            }
        }
        LogEvent::ItemGlow { .. }
        | LogEvent::SpellCastStarted { .. }
        | LogEvent::SpellInterrupted { .. }
        | LogEvent::SpellResisted { .. }
        | LogEvent::CombatAttempt { .. } => {}
    }
    Ok(())
}

pub(crate) fn unquote_chat_message(message: &str) -> &str {
    let message = message.trim();
    let bytes = message.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == 39 || bytes[0] == 34) && bytes[bytes.len() - 1] == bytes[0]
    {
        message[1..message.len() - 1].trim()
    } else {
        message
    }
}

fn strip_linked_item_apostrophes(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(*character as u32, 0x27 | 0x60 | 0x2018 | 0x2019 | 0x00b4))
        .collect()
}

fn normalize_linked_item_text(value: &str) -> String {
    strip_linked_item_apostrophes(value)
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

pub(crate) fn resolve_linked_items(
    connection: &rusqlite::Connection,
    message: &str,
    encoded_items: &[String],
) -> Result<Vec<String>, String> {
    if !encoded_items.is_empty() {
        return Ok(encoded_items.to_vec());
    }
    let message = unquote_chat_message(message);
    let display_message = strip_linked_item_apostrophes(message);
    let normalized_message = normalize_linked_item_text(message);
    let mut statement = connection
        .prepare(
            "SELECT item_name FROM master_items
             WHERE item_name<>'' AND instr(
               replace(replace(replace(replace(replace(lower(?),char(39),''),'`',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¾ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¹ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã¢â‚¬Å“',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â´',''),
               replace(replace(replace(replace(replace(lower(item_name),char(39),''),'`',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¾ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¹ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã¢â‚¬Å“',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â´','')
             )>0
             UNION
             SELECT item_name FROM item_market_values
             WHERE item_name<>'' AND instr(
               replace(replace(replace(replace(replace(lower(?),char(39),''),'`',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¾ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¹ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã¢â‚¬Å“',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â´',''),
               replace(replace(replace(replace(replace(lower(item_name),char(39),''),'`',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¾ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â¹ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã‚Â¦ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã¢â‚¬Å“',''),'ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã¢â‚¬Â ÃƒÂ¢Ã¢â€šÂ¬Ã¢â€žÂ¢ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã¢â‚¬Â¦Ãƒâ€šÃ‚Â¡ÃƒÆ’Ã†â€™Ãƒâ€ Ã¢â‚¬â„¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€¦Ã‚Â¡ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â´','')
             )>0",
        )
        .map_err(|error| error.to_string())?;
    let names = statement
        .query_map(params![message, message], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .map(|row| row.map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut raw_matches = Vec::new();
    for name in names {
        let normalized_name = normalize_linked_item_text(&name);
        if normalized_name.is_empty() {
            continue;
        }
        for (start, _) in normalized_message.match_indices(&normalized_name) {
            let end = start + normalized_name.len();
            raw_matches.push((start, end, name.clone()));
        }
    }
    let mut matches = raw_matches
        .iter()
        .filter(|candidate| {
            let before = normalized_message[..candidate.0].chars().next_back();
            let after = normalized_message[candidate.1..].chars().next();
            let joins_previous = raw_matches.iter().any(|other| other.1 == candidate.0);
            let joins_next = raw_matches.iter().any(|other| other.0 == candidate.1);
            let follows_known_item = raw_matches.iter().any(|other| {
                other.1 <= candidate.0
                    && normalized_message[other.1..candidate.0]
                        .chars()
                        .all(|value| value.is_whitespace() || ",;/|:-".contains(value))
            });
            let suspicious_title_prefix = display_message[..candidate.0]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace)
                && display_message[..candidate.0]
                    .split_whitespace()
                    .next_back()
                    .map(|word| word.trim_matches(|value: char| !value.is_ascii_alphanumeric()))
                    .is_some_and(|word| {
                        let lower = word.to_ascii_lowercase();
                        word.chars()
                            .next()
                            .is_some_and(|value| value.is_ascii_uppercase())
                            && word.chars().skip(1).any(|value| value.is_ascii_lowercase())
                            && !matches!(
                                lower.as_str(),
                                "anyone"
                                    | "buying"
                                    | "check"
                                    | "found"
                                    | "getting"
                                    | "got"
                                    | "have"
                                    | "here"
                                    | "link"
                                    | "look"
                                    | "need"
                                    | "price"
                                    | "selling"
                                    | "someone"
                                    | "that"
                                    | "this"
                                    | "want"
                            )
                    });
            (before.is_none_or(|value| !value.is_ascii_alphanumeric()) || joins_previous)
                && (after.is_none_or(|value| !value.is_ascii_alphanumeric()) || joins_next)
                && (!suspicious_title_prefix || follows_known_item)
        })
        .cloned()
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| (right.1 - right.0).cmp(&(left.1 - left.0)))
    });
    let mut selected: Vec<(usize, usize, String)> = Vec::new();
    for candidate in matches {
        if selected
            .iter()
            .all(|existing| candidate.1 <= existing.0 || candidate.0 >= existing.1)
        {
            selected.push(candidate);
        }
    }
    selected.sort_by_key(|value| value.0);
    Ok(selected.into_iter().map(|value| value.2).collect())
}

fn merchant_mode_enabled(connection: &rusqlite::Connection) -> bool {
    connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='merchant_mode_enabled'",
            [],
            |row| row.get::<_, String>(0),
        )
        .is_ok_and(|value| value.eq_ignore_ascii_case("true") || value == "1")
}

fn merchant_catalog(connection: &rusqlite::Connection) -> Result<Vec<CatalogItem>, String> {
    let mut statement = connection
        .prepare("SELECT item_id,item_name FROM master_items ORDER BY LENGTH(item_name) DESC")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(CatalogItem {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

fn finish_merchant_capture(connection: &rusqlite::Connection) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('merchant_last_capture_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [],
        )
        .map_err(|error| error.to_string())?;
    connection
        .execute(
            "DELETE FROM merchant_messages WHERE id NOT IN (SELECT id FROM merchant_messages ORDER BY id DESC LIMIT 2000)",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn enqueue_planner_upload(database: &Database, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("export.txt");
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute(
            "INSERT INTO planner_upload_jobs(file_name,payload_text) VALUES(?,?)",
            params![file_name, text],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}
fn process_exports(
    database: &Database,
    directory: &Path,
    seen: &mut HashMap<PathBuf, (u64, SystemTime)>,
) -> Result<bool, String> {
    let mut changed = false;
    for entry in fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let lower = path
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !(lower.ends_with("-inventory.txt") || lower.ends_with("-spellbook.txt")) {
            continue;
        }
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let signature = (
            metadata.len(),
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        );
        match seen.get(&path) {
            None | Some(_) if seen.get(&path) != Some(&signature) => {
                changed = true;
                let _writer_guard = database.writer_guard();
                match data::mutate(
                    database,
                    "inventory.import",
                    &json!({"path":path.display().to_string()}),
                ) {
                    Ok(_) => {
                        record_export_import(database, &path, signature)?;
                        seen.insert(path.clone(), signature);
                        if let Ok(connection) = database.connect() {
                            let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('last_export_file',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[path.display().to_string()]);
                            let _=connection.execute("INSERT INTO app_settings(key,value) VALUES('last_export_import_at',CURRENT_TIMESTAMP) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[]);
                        }
                        log(
                            database,
                            "info",
                            "inventory",
                            &format!("Imported {}", path.display()),
                        );
                        if let Err(error) = enqueue_planner_upload(database, &path) {
                            log(
                                database,
                                "error",
                                "planner",
                                &format!("Could not queue inventory upload: {error}"),
                            );
                        }
                    }
                    Err(e) => log(database, "error", "inventory", &e),
                }
            }
            _ => {}
        }
    }
    Ok(changed)
}

fn baseline_exports(
    directory: &Path,
    seen: &mut HashMap<PathBuf, (u64, SystemTime)>,
) -> Result<(), String> {
    if !directory.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !(name.ends_with("-inventory.txt") || name.ends_with("-spellbook.txt")) {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        seen.insert(
            path,
            (
                metadata.len(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            ),
        );
    }
    Ok(())
}

fn reconcile_exports(
    database: &Database,
    directory: &Path,
    seen: &mut HashMap<PathBuf, (u64, SystemTime)>,
) -> Result<bool, String> {
    if !directory.is_dir() {
        return Ok(false);
    }
    baseline_exports(directory, seen)?;
    let connection = database.connect().map_err(|error| error.to_string())?;
    seen.retain(|path, signature| {
        let stored = connection
            .query_row(
                "SELECT file_size,modified_unix_ns FROM export_import_state WHERE source_file=?",
                [path.display().to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .ok()
            .flatten();
        stored == Some((signature.0 as i64, system_time_unix_ns(signature.1)))
    });
    drop(connection);
    process_exports(database, directory, seen)
}

fn record_export_import(
    database: &Database,
    path: &Path,
    signature: (u64, SystemTime),
) -> Result<(), String> {
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute(
            "INSERT INTO export_import_state(source_file,file_size,modified_unix_ns,imported_at)              VALUES(?,?,?,CURRENT_TIMESTAMP) ON CONFLICT(source_file) DO UPDATE SET              file_size=excluded.file_size,modified_unix_ns=excluded.modified_unix_ns,imported_at=CURRENT_TIMESTAMP",
            params![
                path.display().to_string(),
                signature.0 as i64,
                system_time_unix_ns(signature.1)
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn system_time_unix_ns(value: SystemTime) -> i64 {
    value
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn is_log(path: &Path) -> bool {
    log_file_identity(path).is_some()
}

fn is_active_log(path: &Path) -> bool {
    log_file_identity(path).is_some_and(|(_, active)| active)
}

fn log_file_identity(path: &Path) -> Option<(String, bool)> {
    let name = path.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let rest = lower.strip_prefix("eqlog_")?;
    let marker = rest.rfind("_p1999green")?;
    let character = &name[6..6 + marker];
    if character.is_empty() {
        return None;
    }
    let suffix = &rest[marker + "_p1999green".len()..];
    let active = suffix == ".txt";
    let rotated_after = suffix
        .strip_prefix(".txt.")
        .is_some_and(valid_rotation_token);
    let rotated_before = suffix.strip_suffix(".txt").is_some_and(|value| {
        let mut characters = value.chars();
        matches!(characters.next(), Some('.' | '_' | '-'))
            && valid_rotation_token(characters.as_str())
    });
    (active || rotated_after || rotated_before).then(|| (character.to_owned(), active))
}

fn valid_rotation_token(value: &str) -> bool {
    !value.is_empty()
        && value.chars().any(|character| character.is_ascii_digit())
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '.' | '_' | '-' | '(' | ')' | '[' | ']')
        })
}

fn character_from_log(path: &Path) -> Option<String> {
    log_file_identity(path).map(|(character, _)| character)
}
fn log(database: &Database, level: &str, area: &str, message: &str) {
    if let Ok(c) = database.connect() {
        log_with(&c, level, area, message)
    }
}
fn log_with(c: &rusqlite::Connection, level: &str, area: &str, message: &str) {
    let _ = c.execute(
        "INSERT INTO application_logs(level,area,message) VALUES(?,?,?)",
        params![level, area, message],
    );
}

#[cfg(test)]
mod tests {
    use super::{
        character_from_log, is_active_log, is_log, poll, process_log, reconcile_exports,
        record_damage_event, record_incoming_damage_event, record_observed_melee_event,
        resolve_linked_items, scan_damage_file, scan_death_report_file, scan_history_directory,
    };
    use crate::application::database_management::{delete_protected_fights, set_fight_protection};
    use crate::infrastructure::database::Database;
    use chrono::NaiveDateTime;
    use std::{collections::HashMap, fs, io::Write};

    #[test]
    fn recognizes_common_split_log_names_without_treating_them_as_live() {
        for name in [
            "eqlog_Youngman_P1999Green.txt.1",
            "eqlog_Youngman_P1999Green_2.txt",
            "eqlog_Youngman_P1999Green-2026-09-06.txt",
            "eqlog_Youngman_P1999Green.20260906.140000.txt",
        ] {
            let path = std::path::Path::new(name);
            assert!(is_log(path), "{name} should be a recognized segment");
            assert!(!is_active_log(path), "{name} must never become live");
            assert_eq!(character_from_log(path).as_deref(), Some("Youngman"));
        }
        assert!(is_active_log(std::path::Path::new(
            "eqlog_Youngman_P1999Green.txt"
        )));
        assert!(!is_log(std::path::Path::new(
            "eqlog_Youngman_P1999Green.txt.bak"
        )));
    }

    #[test]
    fn protected_and_purged_fights_are_not_duplicated_or_recreated_by_rescan() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Tester_P1999Green.txt");
        fs::write(&log, b"[Sat Sep 05 10:00:00 2026] You crush a frost giant for 50 points of damage.\r\n[Sat Sep 05 10:00:01 2026] You have slain a frost giant!\r\n").unwrap();
        assert_eq!(scan_damage_file(&database, &log).unwrap(), 1);
        let id = database
            .connect()
            .unwrap()
            .query_row("SELECT id FROM damage_encounters", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        set_fight_protection(&database, id, true).unwrap();
        database
            .connect()
            .unwrap()
            .execute("DELETE FROM damage_scan_cursors", [])
            .unwrap();
        assert_eq!(scan_damage_file(&database, &log).unwrap(), 0);
        assert_eq!(
            database
                .connect()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        delete_protected_fights(&database, &[id]).unwrap();
        database
            .connect()
            .unwrap()
            .execute("DELETE FROM damage_scan_cursors", [])
            .unwrap();
        assert_eq!(scan_damage_file(&database, &log).unwrap(), 0);
        assert_eq!(
            database
                .connect()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn history_reconciliation_reads_every_split_segment_once() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let first = directory.path().join("eqlog_Youngman_P1999Green.txt.1");
        let second = directory.path().join("eqlog_Youngman_P1999Green_2.txt");
        fs::write(
            first,
            b"[Mon Aug 03 07:09:18 2026] --You have looted A Blue Throne.--\r\n",
        )
        .unwrap();
        fs::write(
            second,
            b"[Mon Aug 03 07:10:18 2026] --You have looted A White Throne.--\r\n",
        )
        .unwrap();
        let first_scan = scan_history_directory(&database, directory.path()).unwrap();
        let second_scan = scan_history_directory(&database, directory.path()).unwrap();
        assert_eq!(first_scan.files, 2);
        assert_eq!(first_scan.inserted, 2);
        assert_eq!(second_scan.inserted, 0);
        let count: i64 = database
            .connect()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM activity_loot_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn rotated_segment_cannot_override_the_live_character() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [directory.path().display().to_string()],
            )
            .unwrap();
        let live = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let rotated = directory.path().join("eqlog_Other_P1999Green.txt.1");
        fs::write(&live, b"").unwrap();
        fs::write(&rotated, b"").unwrap();
        let mut active = None;
        poll(
            &database,
            &mut active,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&rotated),
        )
        .unwrap();
        assert_eq!(active.as_deref(), Some(live.as_path()));
    }

    #[test]
    fn initial_folder_poll_imports_existing_exports() {
        let directory = tempfile::tempdir().unwrap();
        let logs = directory.path().join("Logs");
        fs::create_dir(&logs).unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [logs.display().to_string()],
            )
            .unwrap();
        let export = directory.path().join("Youngman-Inventory.txt");
        fs::write(&export, b"Primary\tA Blue Crown\t12345\t1\r\n").unwrap();
        let changed = poll(
            &database,
            &mut None,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            None,
        )
        .unwrap();
        let connection = database.connect().unwrap();
        let imported: i64 = connection
            .query_row("SELECT COUNT(*) FROM inventory_items", [], |row| row.get(0))
            .unwrap();
        let states: i64 = connection
            .query_row("SELECT COUNT(*) FROM export_import_state", [], |row| {
                row.get(0)
            })
            .unwrap();
        let queued: i64 = connection
            .query_row("SELECT COUNT(*) FROM planner_upload_jobs WHERE status='pending' AND payload_text<>''", [], |row| row.get(0))
            .unwrap();
        assert!(changed);
        assert_eq!(imported, 1);
        assert_eq!(states, 1);
        assert_eq!(queued, 1);
    }

    #[test]
    fn startup_reconciliation_skips_unchanged_exports_and_imports_offline_changes() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let export = directory.path().join("Youngman-Inventory.txt");
        fs::write(&export, b"Primary\tFirst Weapon\t1001\t1\r\n").unwrap();
        assert!(reconcile_exports(&database, directory.path(), &mut HashMap::new()).unwrap());
        assert!(!reconcile_exports(&database, directory.path(), &mut HashMap::new()).unwrap());
        fs::write(
            &export,
            b"Primary\tReplacement Weapon With Longer Name\t1002\t1\r\n",
        )
        .unwrap();
        assert!(reconcile_exports(&database, directory.path(), &mut HashMap::new()).unwrap());
        let connection = database.connect().unwrap();
        let imports: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM import_uploads WHERE status='auto import'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let item: String = connection
            .query_row("SELECT item_name FROM inventory_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        let queued: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM planner_upload_jobs WHERE status='pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imports, 2);
        assert_eq!(queued, 2);
        assert_eq!(item, "Replacement Weapon With Longer Name");
    }

    #[test]
    fn damage_scan_aggregates_encounters_and_closes_on_kill() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 10:00:00 2026] You slash a frost giant for 100 points of damage.\r\n\
[Sat Sep 05 10:00:00 2026] You have healed Legiteral for 999 points.\r\n\
[Sat Sep 05 10:00:01 2026] a frost giant was hit by non-melee for 250 points of damage.\r\n\
[Sat Sep 05 10:00:02 2026] a frost giant has taken 50 damage from your Engulfing Darkness.\r\n\
[Sat Sep 05 10:00:02 2026] Legiteral crushes a frost giant for 81 points of damage.\r\n\
[Sat Sep 05 10:00:02 2026] a frost giant hits YOU for 90 points of damage.\r\n\
[Sat Sep 05 10:00:02 2026] a frost giant bashes Legiteral for 60 points of damage.\r\n\
[Sat Sep 05 10:00:02 2026] Bakamore tells the guild, 'LoF 001 CH - Forsure'\r\n\
[Sat Sep 05 10:00:12 2026] Clerica tells the guild, 'lof 002 ch - Forsure'\r\n\
[Sat Sep 05 10:00:13 2026] You have slain a frost giant!\r\n",
        )
        .unwrap();

        assert_eq!(scan_damage_file(&database, &log).unwrap(), 7);
        assert_eq!(scan_damage_file(&database, &log).unwrap(), 0);
        let connection = database.connect().unwrap();
        let encounter: (i64, i64, i64, i64, i64, String) = connection
            .query_row(
                "SELECT total_damage,melee_damage,spell_damage,hit_count,max_hit,outcome
                 FROM damage_encounters",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(encounter, (231, 181, 50, 3, 100, "slain".into()));
        let player_damage = {
            let mut statement = connection
                .prepare(
                    "SELECT attacker_name,SUM(damage) FROM damage_events
                     GROUP BY attacker_name ORDER BY SUM(damage) DESC",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            player_damage,
            vec![("Youngman".into(), 150), ("Legiteral".into(), 81)]
        );
        let received = {
            let mut statement = connection
                .prepare(
                    "SELECT target_name,SUM(damage) FROM damage_received_events
                     GROUP BY target_name ORDER BY SUM(damage) DESC",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            received,
            vec![("Youngman".into(), 90), ("Legiteral".into(), 60)]
        );
        let participant_summary: i64 = connection
            .query_row(
                "SELECT SUM(total_damage) FROM damage_participant_summaries",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let target_summary: i64 = connection
            .query_row(
                "SELECT SUM(total_damage) FROM damage_target_summaries",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(participant_summary, 231);
        assert_eq!(target_summary, 150);
        let calls = {
            let mut statement = connection
                .prepare(
                    "SELECT cleric_name,call_number,target_name,happened_at
                     FROM cleric_heal_calls ORDER BY happened_at",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "Bakamore");
        assert_eq!(calls[0].1, 1);
        assert_eq!(calls[0].2.as_deref(), Some("Forsure"));
        assert_eq!(calls[1].0, "Clerica");
        assert_eq!(calls[1].1, 2);
    }

    #[test]
    fn backlog_replay_reuses_closed_encounters_when_detail_rows_are_missing() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let source = "eqlog_Replay_P1999Green.txt";
        connection
            .execute(
                "INSERT INTO damage_encounters(
                    character_name,mob_name,started_at,last_damage_at,ended_at,outcome,
                    source_file,first_source_offset,last_source_offset
                 ) VALUES
                    ('Replay','Target One','2026-09-07 10:00:00','2026-09-07 10:00:00',
                     '2026-09-07 10:00:00','slain',?,10,10),
                    ('Replay','Target Two','2026-09-07 10:01:00','2026-09-07 10:01:00',
                     '2026-09-07 10:01:00','slain',?,20,20)",
                rusqlite::params![source, source],
            )
            .unwrap();
        let outgoing_at =
            NaiveDateTime::parse_from_str("2026-09-07 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
        let incoming_at =
            NaiveDateTime::parse_from_str("2026-09-07 10:01:00", "%Y-%m-%d %H:%M:%S").unwrap();

        assert_eq!(
            record_damage_event(
                &connection,
                source,
                10,
                "Replay crushes Target One for 50 points of damage.",
                "Replay",
                "Replay",
                outgoing_at,
                "Target One",
                "crush",
                50,
                "melee",
            )
            .unwrap(),
            1
        );
        assert_eq!(
            record_incoming_damage_event(
                &connection,
                source,
                20,
                "Target Two hits Replay for 25 points of damage.",
                "Replay",
                incoming_at,
                "Target Two",
                "Replay",
                "hit",
                25,
            )
            .unwrap(),
            1
        );

        let encounter_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        let outgoing_encounter: i64 = connection
            .query_row("SELECT encounter_id FROM damage_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        let incoming_encounter: i64 = connection
            .query_row(
                "SELECT encounter_id FROM damage_received_events",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(encounter_count, 2);
        assert_eq!((outgoing_encounter, incoming_encounter), (1, 2));
    }
    #[test]
    fn generic_non_melee_requires_recent_confirmed_local_spell_activity() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let source = "eqlog_Asquatii_P1999Green.txt";
        let started =
            chrono::NaiveDateTime::parse_from_str("2026-09-06 10:53:00", "%Y-%m-%d %H:%M:%S")
                .unwrap();
        record_damage_event(
            &connection,
            source,
            1,
            "melee",
            "Asquatii",
            "Asquatii",
            started,
            "Hexbone skeleton",
            "crush",
            37,
            "melee",
        )
        .unwrap();
        let encounter_id: i64 = connection
            .query_row("SELECT id FROM damage_encounters", [], |row| row.get(0))
            .unwrap();
        connection
            .execute(
                "INSERT INTO combat_spell_activity(
                    encounter_id,spell_name,target_name,caster_name,source_kind,happened_at,
                    source_file,landing_source_offset
                 ) VALUES(?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    encounter_id,
                    "Dawncall",
                    "Hexbone skeleton",
                    "Asquatii",
                    "direct",
                    "2026-09-06 10:53:01",
                    source,
                    2
                ],
            )
            .unwrap();
        let recorded = record_damage_event(
            &connection,
            source,
            3,
            "generic direct damage",
            "Asquatii",
            "Asquatii",
            started + chrono::Duration::seconds(2),
            "Hexbone skeleton",
            "non-melee",
            125,
            "spell",
        )
        .unwrap();
        assert_eq!(recorded, 1);
        let resolved: (String, i64) = connection
            .query_row(
                "SELECT attack_kind,damage FROM damage_events WHERE source_offset=3",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(resolved, ("Dawncall".into(), 125));

        connection
            .execute(
                "INSERT INTO combat_spell_activity(
                    encounter_id,spell_name,target_name,caster_name,source_kind,happened_at,
                    source_file,landing_source_offset
                 ) VALUES(?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    encounter_id,
                    "Essence Tap",
                    "Hexbone skeleton",
                    "Asquatii",
                    "proc",
                    "2026-09-06 10:53:03",
                    source,
                    4
                ],
            )
            .unwrap();
        let duplicate_proc_damage = record_damage_event(
            &connection,
            source,
            5,
            "generic proc damage",
            "Asquatii",
            "Asquatii",
            started + chrono::Duration::seconds(4),
            "Hexbone skeleton",
            "non-melee",
            20,
            "spell",
        )
        .unwrap();
        assert_eq!(duplicate_proc_damage, 0);
    }
    #[test]
    fn damage_scan_tracks_pet_outgoing_and_incoming_damage() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Valmezz_P1999Green.txt");
        fs::write(
            &log,
            b"[Mon Sep 07 06:18:16 2026] Treasure Chest tells you, 'Attacking Grink Master.'\r\n\
[Mon Sep 07 06:18:16 2026] Treasure Chest hits Grink for 149 points of damage.\r\n\
[Mon Sep 07 06:18:17 2026] You crush Grink for 50 points of damage.\r\n\
[Mon Sep 07 06:18:18 2026] Guard McStinkles bites Grink for 122 points of damage.\r\n\
[Mon Sep 07 06:18:18 2026] Grink bites Treasure Chest for 86 points of damage.\r\n\
[Mon Sep 07 06:18:18 2026] Grink hits Guard McStinkles for 65 points of damage.\r\n\
[Mon Sep 07 06:18:19 2026] You have slain Grink!\r\n",
        )
        .unwrap();

        assert_eq!(scan_damage_file(&database, &log).unwrap(), 6);
        assert_eq!(scan_damage_file(&database, &log).unwrap(), 0);
        let connection = database.connect().unwrap();
        let fighters = {
            let mut statement = connection
                .prepare(
                    "SELECT attacker_name,SUM(damage) FROM damage_events
                     GROUP BY attacker_name ORDER BY SUM(damage) DESC",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            fighters,
            vec![
                ("Treasure Chest".into(), 149),
                ("Guard McStinkles".into(), 122),
                ("Valmezz".into(), 50),
            ]
        );
        let incoming = {
            let mut statement = connection
                .prepare(
                    "SELECT target_name,SUM(damage) FROM damage_received_events
                     GROUP BY target_name ORDER BY SUM(damage) DESC",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            incoming,
            vec![
                ("Treasure Chest".into(), 86),
                ("Guard McStinkles".into(), 65)
            ]
        );
        let target_summary: i64 = connection
            .query_row(
                "SELECT SUM(total_damage) FROM damage_target_summaries",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(target_summary, 151);
    }
    #[test]
    fn active_target_wins_over_stale_attacker_encounter() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        let source = "eqlog_Valmezz_P1999Green.txt";
        connection
            .execute(
                "INSERT INTO damage_encounters(
                    character_name,mob_name,started_at,last_damage_at,source_file,
                    first_source_offset,last_source_offset
                 ) VALUES
                    ('Valmezz','Treasure Chest','2026-09-08 10:32:30','2026-09-08 10:32:30',?,10,10),
                    ('Valmezz','Tunare Puppet','2026-09-08 10:32:36','2026-09-08 10:32:36',?,20,20)",
                rusqlite::params![source, source],
            )
            .unwrap();
        let happened_at =
            NaiveDateTime::parse_from_str("2026-09-08 10:32:43", "%Y-%m-%d %H:%M:%S").unwrap();

        assert_eq!(
            record_observed_melee_event(
                &connection,
                source,
                30,
                "Treasure Chest hits Tunare Puppet for 239 points of damage.",
                "Valmezz",
                happened_at,
                "Treasure Chest",
                "Tunare Puppet",
                "hit",
                239,
            )
            .unwrap(),
            1
        );
        let routed: (i64, String, i64) = connection
            .query_row(
                "SELECT encounter_id,attacker_name,damage FROM damage_events WHERE source_offset=30",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(routed, (2, "Treasure Chest".into(), 239));
        let incoming: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_received_events", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(incoming, 0);
    }
    #[test]
    fn damage_events_use_the_latest_known_character_weapon_loadout() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO character_weapon_loadouts(
                character_name,captured_at,primary_weapon_name,secondary_weapon_name,source_file
             ) VALUES('Youngman','2026-09-05 09:59:00','Wurmslayer','Sarnak Warhammer','first'),
                     ('Youngman','2026-09-05 10:00:01','Epic Blade',NULL,'second')",
                [],
            )
            .unwrap();
        drop(connection);
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 10:00:00 2026] You slash a frost giant for 100 points of damage.\r\n\
[Sat Sep 05 10:00:02 2026] You slash a frost giant for 110 points of damage.\r\n",
        )
        .unwrap();

        assert_eq!(scan_damage_file(&database, &log).unwrap(), 2);
        let connection = database.connect().unwrap();
        let weapons = {
            let mut statement = connection
                .prepare(
                    "SELECT w.primary_weapon_name,w.secondary_weapon_name
                 FROM damage_events e JOIN character_weapon_loadouts w ON w.id=e.weapon_loadout_id
                 ORDER BY e.happened_at",
                )
                .unwrap();
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(
            weapons[0],
            (Some("Wurmslayer".into()), Some("Sarnak Warhammer".into()))
        );
        assert_eq!(weapons[1], (Some("Epic Blade".into()), None));
    }

    #[test]
    fn damage_scan_splits_same_mob_after_two_minute_gap() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 10:00:00 2026] You hit a skeleton for 10 points of damage.\r\n\
[Sat Sep 05 10:02:01 2026] You hit a skeleton for 20 points of damage.\r\n",
        )
        .unwrap();

        assert_eq!(scan_damage_file(&database, &log).unwrap(), 2);
        let connection = database.connect().unwrap();
        let encounters: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        let disengaged: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM damage_encounters WHERE outcome='disengaged'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(encounters, 2);
        assert_eq!(disengaged, 1);
    }

    #[test]
    fn notified_log_path_wins_over_directory_timestamp_order() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let first = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let second = directory.path().join("eqlog_Other_P1999Green.txt");
        fs::write(&first, b"").unwrap();
        fs::write(&second, b"").unwrap();
        database.connect().unwrap().execute(
            "INSERT INTO app_settings(key,value) VALUES('logs_directory',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [directory.path().display().to_string()],
        ).unwrap();
        let mut active = None;
        poll(
            &database,
            &mut active,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&first),
        )
        .unwrap();
        let character: String = database
            .connect()
            .unwrap()
            .query_row(
                "SELECT value FROM app_settings WHERE key='active_character'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active.as_deref(), Some(first.as_path()));
        assert_eq!(character, "Youngman");
    }
    #[test]
    fn character_switch_finishes_with_an_empty_group_even_when_replaying_backlog() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [directory.path().display().to_string()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('active_character','Youngman')",
                [],
            )
            .unwrap();
        connection.execute_batch("INSERT INTO app_settings(key,value) VALUES('damage_target_character','Youngman'); INSERT INTO app_settings(key,value) VALUES('damage_target_encounter_id','99');").unwrap();
        connection
            .execute(
                "INSERT INTO known_members(name) VALUES('Youngman'),('Posed')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO current_group(member_id) SELECT id FROM known_members",
                [],
            )
            .unwrap();
        drop(connection);

        let previous_log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let next_log = directory.path().join("eqlog_Valmezz_P1999Green.txt");
        fs::write(&previous_log, b"").unwrap();
        fs::write(
            &next_log,
            b"[Fri Sep 04 12:27:20 2026] Posed tells the group, 'ready'\r\n",
        )
        .unwrap();

        let mut active = Some(previous_log);
        let mut offsets = HashMap::from([(next_log.clone(), 0)]);
        poll(
            &database,
            &mut active,
            &mut offsets,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&next_log),
        )
        .unwrap();

        let connection = database.connect().unwrap();
        let group_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let active_character: String = connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='active_character'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let target_preferences: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key LIKE 'damage_target_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(group_count, 0);
        assert_eq!(active_character, "Valmezz");
        assert_eq!(target_preferences, 0);
    }

    #[test]
    fn unknown_longer_item_phrase_does_not_collapse_to_known_suffix() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute("DELETE FROM master_items WHERE item_id=4294", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source) VALUES(17789,'Shackles','test')",
                [],
            )
            .unwrap();

        assert!(resolve_linked_items(
            &connection,
            "'Dusty Rusted Shackles where did that other lizard go'",
            &[],
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn resolves_case_and_apostrophe_variants_to_the_master_item() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source)
                 VALUES(20819,'Elders Earring','test')
                 ON CONFLICT(item_id) DO UPDATE SET item_name=excluded.item_name",
                [],
            )
            .unwrap();

        assert_eq!(
            resolve_linked_items(&connection, "'Elder's Earring'", &[]).unwrap(),
            vec!["Elders Earring"]
        );
        assert_eq!(
            resolve_linked_items(&connection, "'still 7500 for elders earring'", &[]).unwrap(),
            vec!["Elders Earring"]
        );
    }

    #[test]
    fn live_damage_uses_notified_character_without_running_directory_backfill() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [directory.path().display().to_string()],
            )
            .unwrap();
        let current = directory.path().join("eqlog_Valmonk_P1999Green.txt");
        let archived = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &current,
            b"[Sat Sep 05 09:15:00 2026] You crush a snow cougar for 79 points of damage.\r\n",
        )
        .unwrap();
        fs::write(
            &archived,
            b"[Sat Sep 05 08:00:00 2026] You hit a skeleton for 10 points of damage.\r\n",
        )
        .unwrap();

        let mut active = None;
        let mut offsets = HashMap::from([(current.clone(), 0)]);
        let changed = poll(
            &database,
            &mut active,
            &mut offsets,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&current),
        )
        .unwrap();

        let connection = database.connect().unwrap();
        let active_character: String = connection
            .query_row(
                "SELECT value FROM app_settings WHERE key='active_character'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let damage: i64 = connection
            .query_row("SELECT total_damage FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        let backfill_cursors: i64 = connection
            .query_row("SELECT COUNT(*) FROM damage_scan_cursors", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(changed, "live damage must request a UI refresh");
        assert_eq!(active_character, "Valmonk");
        assert_eq!(damage, 79);
        assert_eq!(
            backfill_cursors, 0,
            "live polling must not start a full history scan"
        );
    }

    #[test]
    fn isolated_cleric_call_requests_an_immediate_ui_refresh() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 16:40:29 2026] Bakamore tells the guild, 'LoF 001 CH - Forsure'\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        let changed = process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        assert!(changed, "a CH call must request an immediate UI refresh");
        let connection = database.connect().unwrap();
        let call: (String, i64, Option<String>) = connection
            .query_row(
                "SELECT cleric_name,call_number,target_name FROM cleric_heal_calls",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(call, ("Bakamore".into(), 1, Some("Forsure".into())));
    }

    #[test]
    fn guild_slow_call_is_persisted_and_requests_an_immediate_ui_refresh() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 16:40:29 2026] Shaman tells the guild, 'slow a frost giant'\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        let changed = process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        assert!(
            changed,
            "a guild slow call must immediately refresh CH state"
        );
        let connection = database.connect().unwrap();
        let evidence: (String, String, String) = connection
            .query_row(
                "SELECT character_name,speaker_name,mob_name FROM guild_slow_calls",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            evidence,
            ("Youngman".into(), "Shaman".into(), "a frost giant".into())
        );
    }

    #[test]
    fn isolated_mob_death_requests_ui_refresh_for_chain_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Sat Sep 05 16:40:20 2026] You hit a frost giant for 10 points of damage.\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let mut file = fs::OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all(b"[Sat Sep 05 16:40:30 2026] You have slain a frost giant!\r\n")
            .unwrap();
        let changed = process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        assert!(changed, "a mob death must immediately refresh CH state");
        let connection = database.connect().unwrap();
        let outcome: String = connection
            .query_row("SELECT outcome FROM damage_encounters", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(outcome, "slain");
    }

    #[test]
    fn processes_multiple_new_loot_lines_with_distinct_offsets() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(&log, b"[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--\r\n[Mon Aug 03 07:09:19 2026] --Vinkledoo has looted Blue Throne.--\r\n").unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        let changed = process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        assert!(changed, "inserted loot must request a UI refresh");
        let connection = database.connect().unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM loot_drops", [], |row| row.get(0))
            .unwrap();
        let distinct: i64 = connection
            .query_row(
                "SELECT COUNT(DISTINCT source_offset) FROM loot_drops",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((count, distinct), (2, 2));
    }

    #[test]
    fn persisted_live_cursor_recovers_loot_written_while_stopped() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [directory.path().display().to_string()],
            )
            .unwrap();

        let log = directory.path().join("eqlog_Valmezz_P1999Green.txt");
        fs::write(
            &log,
            b"[Fri Sep 04 12:20:00 2026] --You have looted A Blue Throne.--\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();

        fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(b"[Fri Sep 04 12:27:20 2026] --Hansz has looted a A White Throne.--\r\n")
            .unwrap();

        let mut active = None;
        poll(
            &database,
            &mut active,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&log),
        )
        .unwrap();

        let connection = database.connect().unwrap();
        let recovered: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM loot_drops
                 WHERE looter_name='Hansz' AND item_name='A White Throne'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(recovered, 1);
    }

    #[test]
    fn upgrade_without_cursor_replays_from_last_live_loot() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO app_settings(key,value) VALUES('logs_directory',?)",
                [directory.path().display().to_string()],
            )
            .unwrap();

        let log = directory.path().join("eqlog_Valmezz_P1999Green.txt");
        let first = "[Fri Sep 04 12:20:00 2026] --You have looted A Blue Throne.--";
        fs::write(
            &log,
            format!(
                "{first}\r\n[Fri Sep 04 12:27:20 2026] --Hansz has looted a A White Throne.--\r\n"
            ),
        )
        .unwrap();
        database
            .connect()
            .unwrap()
            .execute(
                "INSERT INTO loot_drops(
                    happened_at,item_name,looter_name,raw_line,source_file,source_offset
                 ) VALUES('2026-09-04 12:20:00','A Blue Throne','Valmezz',?,?,0)",
                rusqlite::params![first, log.display().to_string()],
            )
            .unwrap();

        let mut active = None;
        poll(
            &database,
            &mut active,
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut HashMap::new(),
            &mut None,
            Some(&log),
        )
        .unwrap();

        let connection = database.connect().unwrap();
        let total: i64 = connection
            .query_row("SELECT COUNT(*) FROM loot_drops", [], |row| row.get(0))
            .unwrap();
        let recovered: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM loot_drops
                 WHERE looter_name='Hansz' AND item_name='A White Throne'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((total, recovered), (2, 1));
    }

    #[test]
    fn death_report_backfill_captures_exactly_the_previous_thirty_complete_lines() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let log = directory.path().join("eqlog_Derpscleric_P1999Green.txt");
        let mut contents = String::new();
        for index in 1..=35 {
            contents.push_str(&format!(
                "[Tue Sep 01 12:31:20 2026] context line {index}\r\n"
            ));
        }
        contents
            .push_str("[Tue Sep 01 12:31:21 2026] You have been slain by Overking Bathezid!\r\n");
        fs::write(&log, contents).unwrap();

        assert_eq!(scan_death_report_file(&database, &log).unwrap(), 1);
        assert_eq!(scan_death_report_file(&database, &log).unwrap(), 0);

        let connection = database.connect().unwrap();
        let report: (String, String, i64) = connection
            .query_row(
                "SELECT character_name,killer_name,source_offset FROM death_reports",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let context: Vec<String> = connection
            .prepare(
                "SELECT raw_line FROM death_report_entries
                 ORDER BY sequence_number",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(report.0, "Derpscleric");
        assert_eq!(report.1, "Overking Bathezid");
        assert!(report.2 > 0);
        assert_eq!(context.len(), 30);
        assert!(context.first().unwrap().ends_with("context line 6"));
        assert!(context.last().unwrap().ends_with("context line 35"));
    }

    #[test]
    fn history_scan_backfills_all_characters_and_resumes_without_duplicates() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let youngman = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let posed = directory.path().join("eqlog_Posed_P1999Green.txt");
        fs::write(
            &youngman,
            b"[Mon Aug 03 07:09:18 2026] --You have looted a Tears of Prexus.--\r\n[Mon Aug 03 07:09:19 2026] You have slain a mortiferous golem!\r\n",
        )
        .unwrap();
        fs::write(
            &posed,
            b"[Mon Aug 03 07:09:20 2026] [Youngman] has offered you a Blue Diamond.\r\n[Mon Aug 03 07:09:22 2026] You have gained a level! Welcome to level 54!\r\n",
        )
        .unwrap();

        let first = scan_history_directory(&database, directory.path()).unwrap();
        let second = scan_history_directory(&database, directory.path()).unwrap();
        assert_eq!((first.files, first.inserted), (2, 4));
        assert_eq!(second.inserted, 0);

        fs::OpenOptions::new()
            .append(true)
            .open(&youngman)
            .unwrap()
            .write_all(b"[Mon Aug 03 07:09:21 2026] a fire giant has been slain by Posed!\r\n")
            .unwrap();
        let third = scan_history_directory(&database, directory.path()).unwrap();
        assert_eq!(third.inserted, 1);

        let connection = database.connect().unwrap();
        let loot: (String, String) = connection
            .query_row(
                "SELECT character_name,item_name FROM activity_loot_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let offered: (String, String, String) = connection
            .query_row(
                "SELECT character_name,offerer_name,item_name FROM activity_offer_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let mobs: i64 = connection
            .query_row("SELECT COUNT(*) FROM activity_mob_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        let level: (String, i64, String) = connection
            .query_row(
                "SELECT character_name,level,direction FROM activity_level_history",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(loot, ("Youngman".into(), "Tears of Prexus".into()));
        assert_eq!(
            offered,
            ("Posed".into(), "Youngman".into(), "Blue Diamond".into())
        );
        assert_eq!(mobs, 2);
        assert_eq!(level, ("Posed".into(), 54, "gained".into()));
    }

    #[test]
    fn clears_the_entire_group_when_the_local_player_is_removed() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO known_members(name) VALUES('Youngman'),('Posed'),('Nukeman')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO current_group(member_id) SELECT id FROM known_members",
                [],
            )
            .unwrap();
        drop(connection);

        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Mon Aug 03 07:35:16 2026] You have been removed from the group.\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();

        let connection = database.connect().unwrap();
        let active_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let remembered_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM known_members", [], |row| row.get(0))
            .unwrap();
        assert_eq!(active_count, 0);
        assert_eq!(remembered_count, 3);
    }

    #[test]
    fn captures_merchant_activity_only_while_enabled() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection.execute("INSERT INTO master_items(item_id,item_name,source) VALUES(1,'This Item','test'),(2,'That Item','test')",[]).unwrap();
        drop(connection);

        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        fs::write(
            &log,
            b"[Mon Aug 03 07:09:18 2026] Trader auctions, 'WTS This Item 1300, That Item'\r\n",
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let connection = database.connect().unwrap();
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM merchant_messages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        connection
            .execute(
                "UPDATE app_settings SET value='true' WHERE key='merchant_mode_enabled'",
                [],
            )
            .unwrap();
        drop(connection);

        fs::write(&log, b"[Mon Aug 03 07:09:19 2026] Buyer auctions, 'WTB This Item 1.5k / That Item'\r\n[Mon Aug 03 07:09:20 2026] Buyer tells you, 'Still available?'\r\n").unwrap();
        offsets.insert(log.clone(), 0);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();
        let connection = database.connect().unwrap();
        let messages = connection
            .query_row("SELECT COUNT(*) FROM merchant_messages", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let items = connection
            .query_row("SELECT COUNT(*) FROM merchant_message_items", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap();
        let price = connection
            .query_row(
                "SELECT asking_price_pp FROM merchant_message_items ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!((messages, items, price), (2, 2, 1500));
    }

    #[test]
    fn captures_group_and_guild_item_links_with_group_presence() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path().join("loot.db")).unwrap();
        database.migrate().unwrap();
        let connection = database.connect().unwrap();
        connection
            .execute(
                "INSERT INTO master_items(item_id,item_name,source)
                 VALUES(1,'Water Sprinkler of Nem Ankh','test')",
                [],
            )
            .unwrap();
        drop(connection);
        let log = directory.path().join("eqlog_Youngman_P1999Green.txt");
        let group_link = format!("\u{12}{}A Blue Crown\u{12}", "0".repeat(45));
        let guild_link = format!("\u{12}{}      White Dragon Scale \u{12}", "A".repeat(45));
        fs::write(
            &log,
            format!(
                "[Mon Aug 03 07:16:30 2026] Posed tells the group, '{group_link}'\r\n[Mon Aug 03 07:16:31 2026] Skriz tells the guild, '{guild_link}'\r\n[Thu Aug 27 12:41:43 2026] Dubbyl tells the group, 'Water Sprinkler of Nem Ankh'\r\n[Tue Sep 08 08:48:06 2026] Tranquellious tells the guild, 'Gleaming Serrated Blade rotting in CoM. 5m25s'\r\n[Thu Aug 27 12:41:44 2026] Dubbyl tells the group, 'ordinary conversation'\r\n"
            ),
        )
        .unwrap();
        let mut offsets = HashMap::from([(log.clone(), 0)]);
        process_log(&database, &log, &mut offsets, &mut HashMap::new()).unwrap();

        let connection = database.connect().unwrap();
        let linked: i64 = connection
            .query_row("SELECT COUNT(*) FROM linked_loot_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        let grouped: i64 = connection
            .query_row("SELECT COUNT(*) FROM current_group", [], |row| row.get(0))
            .unwrap();
        let guild_member_active: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM current_group g JOIN known_members m ON m.id=g.member_id WHERE m.name='Skriz'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let plain_item: String = connection
            .query_row(
                "SELECT item_name FROM linked_loot_items WHERE speaker_name='Dubbyl'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let contextual_item: String = connection
            .query_row(
                "SELECT item_name FROM linked_loot_items WHERE speaker_name='Tranquellious'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!((linked, grouped, guild_member_active), (4, 3, 0));
        assert_eq!(plain_item, "Water Sprinkler of Nem Ankh");
        assert_eq!(contextual_item, "Gleaming Serrated Blade");
    }
}
