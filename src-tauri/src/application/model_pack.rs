use chrono::Utc;
use reqwest::blocking::Client;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc, OnceLock, RwLock,
    },
    thread,
    time::Duration,
};

use crate::infrastructure::{database::Database, paths};

const MANIFEST_NAME: &str = "model-pack.json";
const PACK_NAME: &str = "P99 Classic Character Appearance Models";
const PACK_VERSION: &str = "9";
const SOURCE_ROOT: &str = "https://p99planner.com/models/";
const ITEM_APPEARANCE_CATALOG: &str = include_str!("../../assets/p99-item-appearance.tsv");
const BASE_MODELS: &[&str] = &[
    "baf", "bam", "daf", "dam", "dwf", "dwm", "elf", "elm", "erf", "erm", "gnf", "gnm", "haf",
    "ham", "hif", "him", "hof", "hom", "huf", "hum", "ikf", "ikm", "ogf", "ogm", "trf", "trm",
];
const ROBE_MODELS: &[&str] = &[
    "daf", "dam", "erf", "erm", "gnf", "gnm", "hif", "him", "huf", "hum", "ikf", "ikm",
];
const CLASSIC_CUSTOM_HELM_MODELS: &[&str] = &[
    "IT530", "IT537", "IT540", "IT545", "IT550", "IT557", "IT561", "IT565", "IT570", "IT575",
    "IT580", "IT585", "IT590", "IT595", "IT600", "IT605", "IT610", "IT615", "IT620", "IT627",
    "IT630", "IT635", "IT640", "IT645", "IT650", "IT655",
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
    pub latest_version: String,
    pub update_available: bool,
    pub name: Option<String>,
    pub version: Option<String>,
    pub path: Option<String>,
    pub source_url: Option<String>,
    pub asset_base_url: Option<String>,
    pub model_count: usize,
    pub file_count: usize,
    pub bytes: u64,
    pub verified_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
struct ModelAssetIndex {
    root: PathBuf,
    files: HashSet<String>,
}

impl ModelAssetIndex {
    fn load(root: PathBuf) -> Result<Self, String> {
        let manifest = load_manifest(&root)?;
        Ok(Self {
            root,
            files: manifest.files.into_iter().map(|file| file.path).collect(),
        })
    }

    fn allows(&self, relative: &str) -> bool {
        self.files.contains(relative)
    }
}

static MODEL_ASSET_INDEX: OnceLock<RwLock<Option<ModelAssetIndex>>> = OnceLock::new();

fn model_asset_index() -> &'static RwLock<Option<ModelAssetIndex>> {
    MODEL_ASSET_INDEX.get_or_init(|| RwLock::new(None))
}

fn clear_model_asset_index() {
    if let Ok(mut current) = model_asset_index().write() {
        *current = None;
    }
}

impl ModelPackStatus {
    fn missing() -> Self {
        Self {
            installed: false,
            valid: false,
            latest_version: PACK_VERSION.to_owned(),
            update_available: false,
            name: None,
            version: None,
            path: None,
            source_url: None,
            asset_base_url: None,
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
    let asset_base_url = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='web_url'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|value| format!("{}/model-assets/", value.trim_end_matches('/')));
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
        return Ok(ModelPackStatus {
            asset_base_url,
            ..ModelPackStatus::missing()
        });
    };
    let root = PathBuf::from(&path);
    match load_manifest(&root) {
        Ok(manifest) => Ok(ModelPackStatus {
            installed: true,
            // Download and activation perform full checksum verification, and
            // the System page exposes an explicit re-verify action. Repeating
            // thousands of filesystem probes whenever a viewer mounts made a
            // complete pack much slower than the original small wardrobe pack.
            valid: true,
            latest_version: PACK_VERSION.to_owned(),
            update_available: manifest.version != PACK_VERSION,
            name: Some(manifest.name),
            version: Some(manifest.version),
            path: Some(path),
            source_url: Some(manifest.source_url),
            asset_base_url,
            model_count: manifest.models.len(),
            file_count: manifest.files.len(),
            bytes: manifest.files.iter().map(|file| file.bytes).sum(),
            verified_at,
            error: None,
        }),
        Err(error) => Ok(ModelPackStatus {
            installed: true,
            valid: false,
            update_available: true,
            path: Some(path),
            asset_base_url,
            error: Some(error),
            ..ModelPackStatus::missing()
        }),
    }
}

pub fn activate_directory(database: &Database, directory: &str) -> Result<ModelPackStatus, String> {
    let root = PathBuf::from(directory);
    verify_directory(&root)?;
    activate(database, &root)?;
    clear_model_asset_index();
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
    clear_model_asset_index();
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
    clear_model_asset_index();
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
    clear_model_asset_index();
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
    let mut required_textures = BTreeSet::new();
    required_textures.extend(
        SUPPLEMENTAL_TEXTURES
            .iter()
            .map(|value| (*value).to_owned()),
    );
    let mut required_models: Vec<String> = BASE_MODELS
        .iter()
        .flat_map(|model| [format!("{model}.glb"), format!("{model}he00.glb")])
        .collect();
    required_models.extend(ROBE_MODELS.iter().map(|model| format!("{model}01.glb")));
    let mut optional_models = item_model_files();
    optional_models.extend(alternate_head_model_files());
    optional_models.sort();
    optional_models.dedup();
    let mut downloaded_models = Vec::new();
    let mut completed = 0_u64;
    let mut total = (required_models.len() + optional_models.len()) as u64;
    progress(
        completed,
        total,
        "Downloading character and equipment models",
    );
    for (relative, result) in download_parallel(&client, &required_models) {
        let bytes = result?;
        for uri in glb_asset_texture_uris(&bytes)? {
            validate_relative(&uri)?;
            required_textures.insert(uri);
        }
        write_download(root, &relative, &bytes, &mut files)?;
        downloaded_models.push(relative.clone());
        completed += 1;
        progress(completed, total, &format!("Downloaded {relative}"));
    }
    let mut optional_textures = BTreeSet::new();
    for (relative, result) in download_parallel(&client, &optional_models) {
        if let Ok(bytes) = result {
            if let Ok(uris) = glb_asset_texture_uris(&bytes) {
                for uri in uris {
                    validate_relative(&uri)?;
                    optional_textures.insert(uri);
                }
                write_download(root, &relative, &bytes, &mut files)?;
                downloaded_models.push(relative.clone());
            }
        }
        completed += 1;
        progress(completed, total, &format!("Checked {relative}"));
    }
    optional_textures.extend(armor_texture_candidates(&required_textures));
    optional_textures.retain(|relative| !required_textures.contains(relative));
    total += (required_textures.len() + optional_textures.len()) as u64;
    progress(completed, total, "Downloading model textures");
    let required_texture_files: Vec<_> = required_textures.into_iter().collect();
    for (relative, result) in download_parallel(&client, &required_texture_files) {
        let bytes = result?;
        write_download(root, &relative, &bytes, &mut files)?;
        completed += 1;
        progress(completed, total, &format!("Downloaded {relative}"));
    }
    let optional_texture_files: Vec<_> = optional_textures.into_iter().collect();
    for (relative, result) in download_parallel(&client, &optional_texture_files) {
        if let Ok(bytes) = result {
            if is_image_asset(&relative, &bytes) {
                write_download(root, &relative, &bytes, &mut files)?;
            }
        }
        completed += 1;
        progress(completed, total, &format!("Checked {relative}"));
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    downloaded_models.sort();
    let manifest = ModelPackManifest {
        format_version: 1,
        name: PACK_NAME.to_owned(),
        version: PACK_VERSION.to_owned(),
        created_at: Utc::now().to_rfc3339(),
        source_url: SOURCE_ROOT.to_owned(),
        models: downloaded_models
            .iter()
            .map(|value| value.trim_end_matches(".glb").to_owned())
            .collect(),
        files,
    };
    let payload = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(root.join(MANIFEST_NAME), payload).map_err(|error| error.to_string())?;
    Ok(())
}

fn item_model_files() -> Vec<String> {
    let mut files: BTreeSet<String> = ITEM_APPEARANCE_CATALOG
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').nth(4))
        .map(str::trim)
        .filter(|value| {
            value
                .strip_prefix("IT")
                .is_some_and(|suffix| suffix.chars().all(|character| character.is_ascii_digit()))
        })
        .map(|value| format!("items/{}.glb", value.to_ascii_lowercase()))
        .collect();
    files.extend(
        CLASSIC_CUSTOM_HELM_MODELS
            .iter()
            .map(|value| format!("items/{}.glb", value.to_ascii_lowercase())),
    );
    files.into_iter().collect()
}

fn alternate_head_model_files() -> Vec<String> {
    BASE_MODELS
        .iter()
        .flat_map(|model| (1..=23).map(move |material| format!("{model}he{material:02}.glb")))
        .collect()
}

fn visual_materials() -> BTreeSet<u8> {
    ITEM_APPEARANCE_CATALOG
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').nth(3))
        .filter_map(|value| value.parse::<u8>().ok())
        .map(|material| if material == 7 { 3 } else { material })
        .filter(|material| *material > 0 && *material < 100)
        .collect()
}

fn armor_texture_candidates(base_textures: &BTreeSet<String>) -> BTreeSet<String> {
    let materials = visual_materials();
    let mut candidates = BTreeSet::new();
    for relative in base_textures {
        let Some(stem) = relative
            .strip_prefix("textures/")
            .and_then(|value| value.strip_suffix(".png"))
        else {
            continue;
        };
        if stem.len() == 7
            && stem.starts_with("clk")
            && stem[3..]
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            for material in materials
                .iter()
                .copied()
                .filter(|material| (10..=16).contains(material))
            {
                candidates.insert(format!("textures/clk{:02}{}.png", material - 6, &stem[5..]));
            }
            continue;
        }
        if stem.len() < 9
            || !matches!(&stem[3..5], "ch" | "ua" | "fa" | "hn" | "lg" | "ft")
            || !stem[5..]
                .chars()
                .all(|character| character.is_ascii_digit())
        {
            continue;
        }
        for material in &materials {
            candidates.insert(format!(
                "textures/{}{:02}{}.png",
                &stem[..5],
                material,
                &stem[7..]
            ));
        }
    }
    candidates
}

fn download_parallel(
    client: &Client,
    relatives: &[String],
) -> Vec<(String, Result<Vec<u8>, String>)> {
    if relatives.is_empty() {
        return Vec::new();
    }
    let files = Arc::new(relatives.to_vec());
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        for _ in 0..files.len().min(12) {
            let files = Arc::clone(&files);
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(relative) = files.get(index) else {
                    break;
                };
                if sender
                    .send((relative.clone(), download(client, relative)))
                    .is_err()
                {
                    break;
                }
            });
        }
        drop(sender);
        receiver.into_iter().collect()
    })
}

fn is_image_asset(relative: &str, bytes: &[u8]) -> bool {
    match Path::new(relative)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        Some("jpg" | "jpeg") => bytes.starts_with(b"\xff\xd8\xff"),
        _ => false,
    }
}

fn download(client: &Client, relative: &str) -> Result<Vec<u8>, String> {
    let response = client
        .get(format!("{SOURCE_ROOT}{relative}"))
        .header("User-Agent", "EverQuestLootTracker/3")
        .send()
        .map_err(|error| format!("Download failed for {relative}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download failed for {relative}: {error}"))?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if (relative.ends_with(".glb") && !content_type.contains("model/gltf-binary"))
        || (relative.ends_with(".png") && !content_type.starts_with("image/"))
    {
        return Err(format!(
            "Download returned {content_type} instead of the requested asset for {relative}"
        ));
    }
    response
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
    include_body: bool,
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
    let root = match resolve_asset_root(database, &relative) {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
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
    if !include_body {
        return Some(
            root.join(&relative)
                .is_file()
                .then(|| (Vec::new(), content_type.to_owned()))
                .ok_or_else(|| "Model asset is missing from disk".to_owned()),
        );
    }
    Some(
        fs::read(root.join(&relative))
            .map(|bytes| (bytes, content_type.to_owned()))
            .map_err(|error| format!("Could not read model asset: {error}")),
    )
}

fn resolve_asset_root(database: &Database, relative: &str) -> Result<PathBuf, String> {
    if let Ok(current) = model_asset_index().read() {
        if let Some(index) = current.as_ref() {
            return index
                .allows(relative)
                .then(|| index.root.clone())
                .ok_or_else(|| "Asset is not listed in the active model pack".to_owned());
        }
    }
    let connection = database.connect().map_err(|error| error.to_string())?;
    let path = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key='model_pack_path'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "No model pack is connected".to_owned())?;
    drop(connection);
    let root = PathBuf::from(path);
    let index = ModelAssetIndex::load(root.clone())?;
    let allowed = index.allows(relative);
    let mut current = model_asset_index()
        .write()
        .map_err(|_| "Model asset index is unavailable".to_owned())?;
    *current = Some(index);
    if !allowed {
        return Err("Asset is not listed in the active model pack".to_owned());
    }
    Ok(root)
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

fn glb_asset_texture_uris(bytes: &[u8]) -> Result<Vec<String>, String> {
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
    let mut textures: BTreeSet<String> = value
        .get("images")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|image| image.get("uri").and_then(serde_json::Value::as_str))
        .filter(|uri| !uri.starts_with("data:"))
        .map(ToOwned::to_owned)
        .collect();
    for frame in value
        .get("materials")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|material| material.get("extras"))
        .filter_map(|extras| extras.get("frames"))
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(serde_json::Value::as_str)
    {
        let normalized = frame.to_ascii_lowercase();
        let name = normalized.strip_suffix(".png").unwrap_or(&normalized);
        textures.insert(format!("textures/{name}.png"));
    }
    Ok(textures.into_iter().collect())
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
        alternate_head_model_files, armor_texture_candidates, download_into,
        glb_asset_texture_uris, is_image_asset, item_model_files, percent_decode,
        validate_relative, verify_directory, ModelAssetIndex, ModelPackFile, ModelPackManifest,
        MANIFEST_NAME, PACK_VERSION, SUPPLEMENTAL_TEXTURES,
    };
    use std::{collections::BTreeSet, fs};

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
    fn complete_pack_includes_all_catalog_item_model_candidates() {
        let files = item_model_files();
        assert!(files.len() >= 213);
        assert!(files.contains(&"items/it150.glb".to_owned()));
        assert!(files.contains(&"items/it630.glb".to_owned()));
        assert!(files.contains(&"items/it635.glb".to_owned()));
        assert!(files.windows(2).all(|pair| pair[0] < pair[1]));
        let heads = alternate_head_model_files();
        assert_eq!(heads.len(), 26 * 23);
        assert!(heads.contains(&"humhe23.glb".to_owned()));
    }

    #[test]
    fn complete_pack_generates_every_renderer_armor_variant() {
        let base = BTreeSet::from([
            "textures/humch0001.png".to_owned(),
            "textures/clk0401.png".to_owned(),
        ]);
        let candidates = armor_texture_candidates(&base);
        assert!(candidates.contains("textures/humch2201.png"));
        assert!(candidates.contains("textures/humch0301.png"));
        assert!(candidates.contains("textures/clk1001.png"));
    }

    #[test]
    fn optional_texture_downloads_reject_html_fallbacks() {
        assert!(is_image_asset(
            "textures/example.png",
            b"\x89PNG\r\n\x1a\nrest"
        ));
        assert!(!is_image_asset("textures/example.png", b"<!doctype html>"));
    }

    #[test]
    fn model_asset_index_snapshots_the_validated_manifest_allowlist() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = ModelPackManifest {
            format_version: 1,
            name: "Test models".to_owned(),
            version: "1".to_owned(),
            created_at: "2026-09-22T00:00:00Z".to_owned(),
            source_url: "https://example.invalid/models/".to_owned(),
            models: vec!["hum".to_owned()],
            files: vec![ModelPackFile {
                path: "hum.glb".to_owned(),
                bytes: 4,
                sha256: "unused-by-request-index".to_owned(),
            }],
        };
        fs::write(
            directory.path().join(MANIFEST_NAME),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let index = ModelAssetIndex::load(directory.path().to_path_buf()).unwrap();
        fs::remove_file(directory.path().join(MANIFEST_NAME)).unwrap();

        assert!(index.allows("hum.glb"));
        assert!(!index.allows("../loot-tracker.db"));
        assert!(!index.allows("textures/not-listed.png"));
    }

    #[test]
    fn glb_metadata_discovers_external_textures_with_padding() {
        let mut json = br#"{"asset":{"version":"2.0"},"images":[{"uri":"textures/humch00.png"}],"materials":[{"extras":{"frames":["Glow01","glow02.png"]}}]}"#.to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&(20_u32 + json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        assert_eq!(
            glb_asset_texture_uris(&glb).unwrap(),
            vec![
                "textures/glow01.png",
                "textures/glow02.png",
                "textures/humch00.png"
            ]
        );
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
        assert_eq!(manifest.version, PACK_VERSION);
        assert!(manifest.models.len() >= 263);
        assert!(manifest.models.iter().any(|model| model == "humhe00"));
        assert!(manifest.models.iter().any(|model| model == "humhe03"));
        assert!(manifest.models.iter().any(|model| model == "hum01"));
        assert!(manifest.models.iter().any(|model| model == "items/it150"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "items/it150.glb"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "textures/humch2201.png"));
        assert!(manifest
            .files
            .iter()
            .any(|file| file.path == "textures/150leaf.png"));
        assert!(manifest.files.len() >= manifest.models.len());
    }
}
