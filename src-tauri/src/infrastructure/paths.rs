use std::{env, io, path::PathBuf};

pub fn data_directory() -> io::Result<PathBuf> {
    let path = if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        dirs_home().map(|home| home.join("Library").join("Application Support"))
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs_home().map(|home| home.join(".local").join("share")))
    }
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine the application data directory",
        )
    })?
    .join("EverQuestLootTracker");
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

fn dirs_home() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn database_path() -> io::Result<PathBuf> {
    Ok(data_directory()?.join("loot-tracker.db"))
}
