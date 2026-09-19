use chrono::Utc;
use reqwest::blocking::Client;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use crate::infrastructure::{database::Database, paths};

const MANIFEST_NAME: &str = "model-pack.json";
const PACK_NAME: &str = "P99 Classic Character Appearance Models";
const PACK_VERSION: &str = "5";
const SOURCE_ROOT: &str = "https://p99planner.com/models/";
const BASE_MODELS: &[&str] = &[
    "baf", "bam", "daf", "dam", "dwf", "dwm", "elf", "elm", "erf", "erm", "gnf", "gnm", "haf",
    "ham", "hif", "him", "hof", "hom", "huf", "hum", "ikf", "ikm", "ogf", "ogm", "trf", "trm",
];
const ROBE_MODELS: &[&str] = &[
    "daf", "dam", "erf", "erm", "gnf", "gnm", "hif", "him", "huf", "hum", "ikf", "ikm",
];
// Some original EQ effects are represented by WLD particle records and are
// therefore not referenced by the converted GLB metadata. Keep those source
// sprites explicit so downloaded packs can render them offline.
const SUPPLEMENTAL_TEXTURES: &[&str] = &[
    "textures/150leaf.png",
    "textures/csmoke1.png",
    "textures/flare002.png",
    "textures/gena10.png",
    "textures/genb20.png",
    "textures/genb30.png",
    "textures/gend20.png",
    "textures/gend30.png",
    "textures/gend40.png",
    "textures/geng00.png",
    "textures/geng10.png",
    "textures/geni00.png",
    "textures/genj00.png",
    "textures/genj10.png",
    "textures/genn00.png",
    "textures/genq00.png",
    "textures/genw10.png",
    "textures/geny20.png",
    "textures/grstar1.png",
    "textures/it148note1.png",
    "textures/pcrys501.png",
    "textures/pdise501.png",
    "textures/pwbam501.png",
    "textures/pwp10663.png",
    "textures/sample1.png",
    "textures/tfire1.png",
    "textures/tsmoke.png",
    "textures/yin1.png",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackManifest {
    pub format_version: u32,
    pub name: String,
    pub version: String,
    pub created_at: String,
    pub source_url: String,
    pub models: Vec<String>,
    pub files: Vec<ModelPackFile>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackStatus {
    pub installed: bool,
    pub valid: bool,
    pub name: Option<String>,
    pub version: Option<String>,
    pub path: Option<String>,
    pub source_url: Option<String>,
    pub model_count: usize,
    pub file_count: usize,
    pub bytes: u64,
    pub verified_at: Option<String>,
    pub error: Option<String>,
}

impl ModelPackStatus {
    fn missing() -> Self {
        Self {
            installed: false,
            valid: false,
            name: None,
            version: None,
            path: None,
            source_url: None,
            model_count: 0,
            file_count: 0,
            bytes: 0,
            verified_at: None,
            error: None,
        }
    }
}

pub fn status(database: &Database) -> Result<ModelPackStatus, String> {
    let connection = database.connect().map_err(|error| error.to_string())?;
    let path = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='model_pack_path'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    let verified_at = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='model_pack_verified_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();
    drop(connection);
    let Some(path) = path else {
        return Ok(ModelPackStatus::missing());
    };
    let root = PathBuf::from(&path);
    match load_manifest(&root) {
        Ok(manifest) => {
            let missing = manifest
                .files
                .iter()
                .find(|file| !root.join(&file.path).is_file())
                .map(|file| file.path.clone());
            Ok(ModelPackStatus {
                installed: true,
                valid: missing.is_none(),
                name: Some(manifest.name),
                version: Some(manifest.version),
                path: Some(path),
                source_url: Some(manifest.source_url),
                model_count: manifest.models.len(),
                file_count: manifest.files.len(),
                bytes: manifest.files.iter().map(|file| file.bytes).sum(),
                verified_at,
                error: missing.map(|file| format!("Missing pack file: {file}")),
            })
        }
        Err(error) => Ok(ModelPackStatus {
            installed: true,
            valid: false,
            path: Some(path),
            error: Some(error),
            ..ModelPackStatus::missing()
        }),
    }
}

pub fn activate_directory(database: &Database, directory: &str) -> Result<ModelPackStatus, String> {
    let root = PathBuf::from(directory);
    verify_directory(&root)?;
    activate(database, &root)?;
    status(database)
}

pub fn disconnect(database: &Database) -> Result<ModelPackStatus, String> {
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute(
            "DELETE FROM app_settings WHERE key IN (
                'model_pack_path','model_pack_name','model_pack_version',
                'model_pack_source','model_pack_verified_at','model_pack_ready'
            )",
            [],
        )
        .map_err(|error| error.to_string())?;
    Ok(ModelPackStatus::missing())
}

pub fn verify(database: &Database) -> Result<ModelPackStatus, String> {
    let current = status(database)?;
    let path = current
        .path
        .ok_or_else(|| "No character model pack is connected".to_owned())?;
    verify_directory(Path::new(&path))?;
    let now = Utc::now().to_rfc3339();
    database
        .connect()
        .map_err(|error| error.to_string())?
        .execute(
            "INSERT INTO app_settings(key,value) VALUES('model_pack_verified_at',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [&now],
        )
        .map_err(|error| error.to_string())?;
    status(database)
}

pub fn download_base_pack<F>(
    database: &Database,
    mut progress: F,
) -> Result<ModelPackStatus, String>
where
    F: FnMut(u64, u64, &str),
{
    let packs = paths::model_packs_directory().map_err(|error| error.to_string())?;
    let target = packs.join("p99-classic-appearance-v2");
    let staging = packs.join("p99-classic-appearance-v2.staging");
    let backup = packs.join("p99-classic-appearance-v2.previous");
    remove_owned_directory(&staging, &packs)?;
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let result = download_into(&staging, &mut progress);
    if let Err(error) = result {
        let _ = remove_owned_directory(&staging, &packs);
        return Err(error);
    }
    verify_directory(&staging)?;
    remove_owned_directory(&backup, &packs)?;
    if target.exists() {
        fs::rename(&target, &backup)
            .map_err(|error| format!("Could not preserve the previous model pack: {error}"))?;
    }
    if let Err(error) = fs::rename(&staging, &target) {
        if backup.exists() {
            let _ = fs::rename(&backup, &target);
        }
        return Err(format!(
            "Could not activate the downloaded model pack: {error}"
        ));
    }
    let _ = remove_owned_directory(&backup, &packs);
    activate(database, &target)?;
    status(database)
}

fn download_into<F>(root: &Path, progress: &mut F) -> Result<(), String>
where
    F: FnMut(u64, u64, &str),
{
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| error.to_string())?;
    let mut files = Vec::new();
    let mut textures = BTreeSet::new();
    textures.extend(
        SUPPLEMENTAL_TEXTURES
            .iter()
            .map(|value| (*value).to_owned()),
    );
    let mut model_files: Vec<String> = BASE_MODELS
        .iter()
        .flat_map(|model| [format!("{model}.glb"), format!("{model}he00.glb")])
        .collect();
    model_files.extend(ROBE_MODELS.iter().map(|model| format!("{model}01.glb")));
    let mut completed = 0_u64;
    let mut total = model_files.len() as u64;
    for relative in &model_files {
        progress(completed, total, &format!("Downloading {relative}"));
        let bytes = download(&client, relative)?;
        for uri in glb_image_uris(&bytes)? {
            validate_relative(&uri)?;
            textures.insert(uri);
        }
        write_download(root, relative, &bytes, &mut files)?;
        completed += 1;
        total = model_files.len() as u64 + textures.len() as u64;
        progress(completed, total, &format!("Downloaded {relative}"));
    }
    total = model_files.len() as u64 + textures.len() as u64;
    for relative in textures {
        progress(completed, total, &format!("Downloading {relative}"));
        let bytes = download(&client, &relative)?;
        write_download(root, &relative, &bytes, &mut files)?;
        completed += 1;
        progress(completed, total, &format!("Downloaded {relative}"));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let manifest = ModelPackManifest {
        format_version: 1,
        name: PACK_NAME.to_owned(),
        version: PACK_VERSION.to_owned(),
        created_at: Utc::now().to_rfc3339(),
        source_url: SOURCE_ROOT.to_owned(),
        models: model_files
            .iter()
            .map(|value| value.trim_end_matches(".glb").to_owned())
            .collect(),
        files,
    };
    let payload = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(root.join(MANIFEST_NAME), payload).map_err(|error| error.to_string())?;
    Ok(())
}

fn download(client: &Client, relative: &str) -> Result<Vec<u8>, String> {
    client
        .get(format!("{SOURCE_ROOT}{relative}"))
        .header("User-Agent", "EverQuestLootTracker/3")
        .send()
        .map_err(|error| format!("Download failed for {relative}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download failed for {relative}: {error}"))?
        .bytes()
        .map(|bytes| bytes.to_vec())
        .map_err(|error| format!("Could not read {relative}: {error}"))
}

fn write_download(
    root: &Path,
    relative: &str,
    bytes: &[u8],
    files: &mut Vec<ModelPackFile>,
) -> Result<(), String> {
    validate_relative(relative)?;
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(&destination, bytes).map_err(|error| error.to_string())?;
    files.push(ModelPackFile {
        path: relative.replace('\\', "/"),
        bytes: bytes.len() as u64,
        sha256: hash(bytes),
    });
    Ok(())
}

fn activate(database: &Database, root: &Path) -> Result<(), String> {
    let manifest = load_manifest(root)?;
    let path = root
        .canonicalize()
        .map_err(|error| format!("Could not resolve model pack path: {error}"))?
        .display()
        .to_string();
    let verified = Utc::now().to_rfc3339();
    let connection = database.connect().map_err(|error| error.to_string())?;
    for (key, value) in [
        ("model_pack_path", path),
        ("model_pack_name", manifest.name),
        ("model_pack_version", manifest.version),
        ("model_pack_source", manifest.source_url),
        ("model_pack_verified_at", verified),
        ("model_pack_ready", "true".to_owned()),
    ] {
        connection
            .execute(
                "INSERT INTO app_settings(key,value) VALUES(?1,?2)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn load_manifest(root: &Path) -> Result<ModelPackManifest, String> {
    let payload = fs::read(root.join(MANIFEST_NAME))
        .map_err(|error| format!("Could not read {MANIFEST_NAME}: {error}"))?;
    let manifest: ModelPackManifest = serde_json::from_slice(&payload)
        .map_err(|error| format!("Invalid {MANIFEST_NAME}: {error}"))?;
    if manifest.format_version != 1 {
        return Err(format!(
            "Unsupported model pack format {}",
            manifest.format_version
        ));
    }
    if manifest.models.is_empty() || manifest.files.is_empty() {
        return Err("Model pack manifest contains no models or files".to_owned());
    }
    for file in &manifest.files {
        validate_relative(&file.path)?;
    }
    Ok(manifest)
}

fn verify_directory(root: &Path) -> Result<ModelPackManifest, String> {
    let manifest = load_manifest(root)?;
    for file in &manifest.files {
        let payload = fs::read(root.join(&file.path))
            .map_err(|error| format!("Could not read {}: {error}", file.path))?;
        if payload.len() as u64 != file.bytes {
            return Err(format!("Size mismatch for {}", file.path));
        }
        if !file.sha256.eq_ignore_ascii_case(&hash(&payload)) {
            return Err(format!("Checksum mismatch for {}", file.path));
        }
    }
    for model in &manifest.models {
        let expected = format!("{model}.glb");
        if !manifest.files.iter().any(|file| file.path == expected) {
            return Err(format!("Model {model} is missing from the file manifest"));
        }
    }
    Ok(manifest)
}

pub fn read_asset(
    database: &Database,
    request_path: &str,
) -> Option<Result<(Vec<u8>, String), String>> {
    let relative = request_path
        .split('?')
        .next()
        .unwrap_or(request_path)
        .strip_prefix("/model-assets/")?;
    let relative = match percent_decode(relative) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if let Err(error) = validate_relative(&relative) {
        return Some(Err(error));
    }
    let current = match status(database) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    let Some(root) = current.path.map(PathBuf::from) else {
        return Some(Err("No model pack is connected".to_owned()));
    };
    let manifest = match load_manifest(&root) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if !manifest.files.iter().any(|file| file.path == relative) {
        return Some(Err(
            "Asset is not listed in the active model pack".to_owned()
        ));
    }
    let content_type = match Path::new(&relative)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "glb" => "model/gltf-binary",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    Some(
        fs::read(root.join(&relative))
            .map(|bytes| (bytes, content_type.to_owned()))
            .map_err(|error| format!("Could not read model asset: {error}")),
    )
}

fn validate_relative(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("Unsafe model pack path: {value}"));
    }
    Ok(())
}

fn remove_owned_directory(target: &Path, parent: &Path) -> Result<(), String> {
    if !target.exists() {
        return Ok(());
    }
    let parent = parent
        .canonicalize()
        .map_err(|error| format!("Could not resolve model pack storage: {error}"))?;
    let target_parent = target
        .parent()
        .and_then(|value| value.canonicalize().ok())
        .ok_or_else(|| "Could not verify model pack cleanup path".to_owned())?;
    if target_parent != parent {
        return Err("Refusing to remove a directory outside model pack storage".to_owned());
    }
    fs::remove_dir_all(target).map_err(|error| error.to_string())
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn glb_image_uris(bytes: &[u8]) -> Result<Vec<String>, String> {
    if bytes.len() < 20 || &bytes[0..4] != b"glTF" {
        return Err("Downloaded model is not a valid GLB file".to_owned());
    }
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if &bytes[16..20] != b"JSON" || bytes.len() < 20 + length {
        return Err("Downloaded GLB has an invalid JSON chunk".to_owned());
    }
    let json = &bytes[20..20 + length];
    let end = json
        .iter()
        .rposition(|byte| !matches!(byte, 0 | b' '))
        .map(|index| index + 1)
        .unwrap_or(0);
    let value: serde_json::Value = serde_json::from_slice(&json[..end])
        .map_err(|error| format!("Invalid GLB metadata: {error}"))?;
    Ok(value
        .get("images")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|image| image.get("uri").and_then(serde_json::Value::as_str))
        .filter(|uri| !uri.starts_with("data:"))
        .map(ToOwned::to_owned)
        .collect())
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err("Invalid encoded model asset path".to_owned());
            }
            let high = hex(bytes[index + 1])?;
            let low = hex(bytes[index + 2])?;
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| "Model asset path is not valid UTF-8".to_owned())
}

fn hex(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("Invalid encoded model asset path".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        download_into, glb_image_uris, percent_decode, validate_relative, verify_directory,
        SUPPLEMENTAL_TEXTURES,
    };

    #[test]
    fn model_asset_paths_reject_traversal() {
        assert!(validate_relative("hum.glb").is_ok());
        assert!(validate_relative("textures/humch00.png").is_ok());
        assert!(validate_relative("../loot-tracker.db").is_err());
        assert!(validate_relative("textures/../../loot-tracker.db").is_err());
    }

    #[test]
    fn model_asset_urls_decode_safely() {
        assert_eq!(
            percent_decode("textures/a%20texture.png").unwrap(),
            "textures/a texture.png"
        );
        assert!(percent_decode("textures/%XX.png").is_err());
    }

    #[test]
    fn glb_metadata_discovers_external_textures_with_padding() {
        let mut json =
            br#"{"asset":{"version":"2.0"},"images":[{"uri":"textures/humch00.png"}]}"#.to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(20_u32 + json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        assert_eq!(glb_image_uris(&glb).unwrap(), vec!["textures/humch00.png"]);
    }

    #[test]
    fn model_pack_includes_particle_textures_omitted_from_glb_metadata() {
        assert!(SUPPLEMENTAL_TEXTURES.contains(&"textures/150leaf.png"));
        assert!(SUPPLEMENTAL_TEXTURES.contains(&"textures/grstar1.png"));
        assert!(SUPPLEMENTAL_TEXTURES.contains(&"textures/it148note1.png"));
        assert_eq!(SUPPLEMENTAL_TEXTURES.len(), 28);
    }

    #[test]
    #[ignore = "downloads the live optional asset pack"]
    fn live_pack_download_builds_a_valid_manifest() {
        let directory = tempfile::tempdir().unwrap();
        download_into(directory.path(), &mut |_, _, _| {}).unwrap();
        let manifest = verify_directory(directory.path()).unwrap();
        assert_eq!(manifest.models.len(), 64);
        assert!(manifest.models.iter().any(|model| model == "humhe00"));
        assert!(manifest.models.iter().any(|model| model == "hum01"));
        assert!(manifest.files.len() >= manifest.models.len());
    }
}
