use chrono::Utc;
use std::{
    backtrace::Backtrace,
    fs::{self, OpenOptions},
    io::Write,
    panic,
    path::{Path, PathBuf},
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use super::paths;

const RETAINED_LOG_FILES: usize = 7;

pub fn initialize() -> Result<WorkerGuard, String> {
    let directory = paths::diagnostics_directory().map_err(|error| error.to_string())?;
    remove_expired_logs(&directory);
    let appender = tracing_appender::rolling::daily(&directory, "loot-tracker.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_writer(writer)
        .try_init()
        .map_err(|error| error.to_string())?;
    install_panic_hook(directory);
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "application process starting"
    );
    Ok(guard)
}

fn install_panic_hook(directory: PathBuf) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("non-string panic payload");
        let location = info
            .location()
            .map(|value| format!("{}:{}:{}", value.file(), value.line(), value.column()))
            .unwrap_or_else(|| "unknown location".into());
        let backtrace = Backtrace::force_capture();
        tracing::error!(panic.message = payload, panic.location = %location, %backtrace, "unhandled panic");
        append_emergency(&directory, payload, &location, &backtrace);
        previous(info);
    }));
}

fn append_emergency(directory: &Path, payload: &str, location: &str, backtrace: &Backtrace) {
    let path = directory.join("crash.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "\n{} | v{} | PANIC | {} | {}\n{}",
            Utc::now().to_rfc3339(),
            env!("CARGO_PKG_VERSION"),
            location,
            payload.replace(['\r', '\n'], " "),
            backtrace
        );
        let _ = file.flush();
    }
}

fn remove_expired_logs(directory: &Path) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut logs = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("loot-tracker.log")
        })
        .collect::<Vec<_>>();
    logs.sort_by_key(|entry| entry.metadata().and_then(|value| value.modified()).ok());
    let remove_count = logs.len().saturating_sub(RETAINED_LOG_FILES);
    for entry in logs.into_iter().take(remove_count) {
        let _ = fs::remove_file(entry.path());
    }
}

#[cfg(test)]
mod tests {
    use super::remove_expired_logs;
    use std::fs;

    #[test]
    fn retains_only_the_newest_seven_rotated_logs() {
        let directory = tempfile::tempdir().unwrap();
        for day in 0..10 {
            fs::write(
                directory
                    .path()
                    .join(format!("loot-tracker.log.2026-11-{day:02}")),
                day.to_string(),
            )
            .unwrap();
        }
        remove_expired_logs(directory.path());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 7);
    }
}
