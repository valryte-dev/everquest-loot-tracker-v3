use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use keyring::{Entry, Error as KeyringError};
use reqwest::blocking::{multipart, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};

use crate::infrastructure::paths;

const SERVICE: &str = "EverQuestLootTracker";
const MAX_REFERENCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROMPT_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitProviderStatus {
    id: &'static str,
    label: &'static str,
    configured: bool,
    can_generate: bool,
    can_refine: bool,
    default_model: &'static str,
    note: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitCharacterDocument {
    name: String,
    x: f64,
    y: f64,
    scale: f64,
    layer: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitProjectDocument {
    version: u32,
    id: String,
    name: String,
    updated_at: String,
    background: String,
    aspect_ratio: String,
    characters: Vec<PortraitCharacterDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitGenerateRequest {
    provider: String,
    model: String,
    prompt: String,
    reference_image_data_url: Option<String>,
    custom_base_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitGenerateResult {
    provider: String,
    model: String,
    image_data_url: String,
    revised_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortraitRefineResult {
    provider: String,
    model: String,
    prompt: String,
}

#[tauri::command]
pub fn portrait_ai_status() -> Result<Vec<PortraitProviderStatus>, String> {
    Ok(vec![
        provider_status("openai", "OpenAI / ChatGPT", true, false, "gpt-image-1", "Generates or edits an illustration from the exact EQ lineup." )?,
        provider_status("gemini", "Google Gemini", true, false, "gemini-2.5-flash-image", "Generates or edits an illustration from the exact EQ lineup." )?,
        provider_status("anthropic", "Anthropic Claude", false, true, "claude-sonnet-4-5", "Claude can inspect the reference and refine the prompt, but does not return generated images." )?,
        provider_status("custom-openai", "OpenAI-compatible", true, false, "gpt-image-1", "For a self-hosted or compatible Images API endpoint." )?,
        PortraitProviderStatus { id:"cursor", label:"Cursor", configured:false, can_generate:false, can_refine:false, default_model:"", note:"Cursor is an editor and API client, not an image-generation provider. Configure its underlying OpenAI, Gemini, or Anthropic provider here." },
    ])
}

#[tauri::command]
pub fn portrait_ai_save_key(
    provider: String,
    api_key: String,
) -> Result<Vec<PortraitProviderStatus>, String> {
    validate_provider_with_credentials(&provider)?;
    let key = api_key.trim();
    if key.len() < 12 {
        return Err("Enter a valid provider API key".into());
    }
    credential_entry(&provider)?
        .set_password(key)
        .map_err(|error| credential_error("save", &provider, error))?;
    portrait_ai_status()
}

#[tauri::command]
pub fn portrait_ai_clear_key(provider: String) -> Result<Vec<PortraitProviderStatus>, String> {
    validate_provider_with_credentials(&provider)?;
    match credential_entry(&provider)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => portrait_ai_status(),
        Err(error) => Err(credential_error("delete", &provider, error)),
    }
}

#[tauri::command]
pub async fn portrait_ai_generate(
    request: PortraitGenerateRequest,
) -> Result<PortraitGenerateResult, String> {
    tauri::async_runtime::spawn_blocking(move || generate(request))
        .await
        .map_err(|error| format!("Portrait generation worker failed: {error}"))?
}

fn generate(request: PortraitGenerateRequest) -> Result<PortraitGenerateResult, String> {
    validate_prompt(&request.prompt)?;
    let model = normalized_model(&request.provider, &request.model);
    let key = read_key(&request.provider)?;
    let reference = request
        .reference_image_data_url
        .as_deref()
        .map(parse_data_url)
        .transpose()?;
    let client = http_client()?;
    match request.provider.as_str() {
        "openai" => generate_openai(&client, "https://api.openai.com/v1", &key, &model, &request.prompt, reference),
        "custom-openai" => {
            let base = validate_custom_base_url(request.custom_base_url.as_deref())?;
            generate_openai(&client, &base, &key, &model, &request.prompt, reference)
        }
        "gemini" => generate_gemini(&client, &key, &model, &request.prompt, reference),
        "anthropic" => Err("Claude can refine the portrait prompt but cannot generate the final image. Use Refine prompt, then select OpenAI or Gemini.".into()),
        "cursor" => Err("Cursor is not an image-generation API. Select OpenAI, Gemini, or an OpenAI-compatible endpoint.".into()),
        _ => Err("Unsupported portrait provider".into()),
    }
}

#[tauri::command]
pub async fn portrait_ai_refine_prompt(
    request: PortraitGenerateRequest,
) -> Result<PortraitRefineResult, String> {
    tauri::async_runtime::spawn_blocking(move || refine_prompt(request))
        .await
        .map_err(|error| format!("Portrait prompt worker failed: {error}"))?
}

fn refine_prompt(request: PortraitGenerateRequest) -> Result<PortraitRefineResult, String> {
    if request.provider != "anthropic" {
        return Err("Prompt refinement currently uses Anthropic Claude".into());
    }
    validate_prompt(&request.prompt)?;
    let model = normalized_model("anthropic", &request.model);
    let key = read_key("anthropic")?;
    let reference = request
        .reference_image_data_url
        .as_deref()
        .map(parse_data_url)
        .transpose()?;
    let mut content = Vec::new();
    if let Some((mime, bytes)) = reference {
        content.push(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":STANDARD.encode(bytes)}}));
    }
    content.push(json!({"type":"text","text":format!("Rewrite this as a concise image-generation prompt. Preserve every named character, race, visible armor, weapon, handedness, formation, and relative height from the reference. Request a polished hand-drawn fantasy illustration while keeping equipment faithful to the reference. Return only the improved prompt.\n\n{}", request.prompt)}));
    let response = http_client()?.post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", key).header("anthropic-version", "2023-06-01")
        .json(&json!({"model":model,"max_tokens":1200,"messages":[{"role":"user","content":content}]}))
        .send().map_err(|error| format!("Claude prompt refinement failed: {error}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|error| format!("Claude returned an unreadable response: {error}"))?;
    if !status.is_success() {
        return Err(provider_error("Claude", status.as_u16(), &body));
    }
    let prompt = body
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find_map(|block| {
            (block.get("type").and_then(Value::as_str) == Some("text"))
                .then(|| block.get("text").and_then(Value::as_str))
                .flatten()
        })
        .ok_or_else(|| "Claude returned no refined prompt".to_string())?
        .trim()
        .to_string();
    Ok(PortraitRefineResult {
        provider: "anthropic".into(),
        model,
        prompt,
    })
}

#[tauri::command]
pub fn portrait_project_list() -> Result<Vec<PortraitProjectDocument>, String> {
    let directory = paths::portrait_projects_directory().map_err(|error| error.to_string())?;
    let mut projects = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(project) = serde_json::from_str::<PortraitProjectDocument>(&text) else {
            continue;
        };
        if validate_project(&project).is_ok() {
            projects.push(project);
        }
    }
    projects.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(projects)
}

#[tauri::command]
pub fn portrait_project_save(
    mut project: PortraitProjectDocument,
) -> Result<PortraitProjectDocument, String> {
    validate_project(&project)?;
    project.updated_at = Utc::now().to_rfc3339();
    let path = project_path(&project.id)?;
    let temporary = path.with_extension("json.tmp");
    let backup = path.with_extension("json.bak");
    let payload = serde_json::to_vec_pretty(&project).map_err(|error| error.to_string())?;
    fs::write(&temporary, payload)
        .map_err(|error| format!("Could not save portrait project: {error}"))?;
    if path.exists() {
        let _ = fs::remove_file(&backup);
        fs::rename(&path, &backup)
            .map_err(|error| format!("Could not prepare the existing portrait project: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, &path) {
        let _ = fs::rename(&backup, &path);
        return Err(format!("Could not commit portrait project: {error}"));
    }
    let _ = fs::remove_file(backup);
    Ok(project)
}

#[tauri::command]
pub fn portrait_project_delete(id: String) -> Result<(), String> {
    let path = project_path(&id)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not delete portrait project: {error}")),
    }
}

fn provider_status(
    id: &'static str,
    label: &'static str,
    can_generate: bool,
    can_refine: bool,
    default_model: &'static str,
    note: &'static str,
) -> Result<PortraitProviderStatus, String> {
    let configured = match credential_entry(id)?.get_password() {
        Ok(value) => !value.trim().is_empty(),
        Err(KeyringError::NoEntry) => false,
        Err(error) => return Err(credential_error("read", id, error)),
    };
    Ok(PortraitProviderStatus {
        id,
        label,
        configured,
        can_generate,
        can_refine,
        default_model,
        note,
    })
}
fn validate_provider_with_credentials(provider: &str) -> Result<(), String> {
    if matches!(
        provider,
        "openai" | "gemini" | "anthropic" | "custom-openai"
    ) {
        Ok(())
    } else {
        Err("This provider does not accept an API key".into())
    }
}
fn credential_entry(provider: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("portrait-studio-{provider}"))
        .map_err(|error| credential_error("open", provider, error))
}
fn credential_error(action: &str, provider: &str, error: KeyringError) -> String {
    format!("Could not {action} the {provider} Portrait Studio credential: {error}")
}
fn read_key(provider: &str) -> Result<String, String> {
    validate_provider_with_credentials(provider)?;
    credential_entry(provider)?
        .get_password()
        .map_err(|error| match error {
            KeyringError::NoEntry => {
                format!("Add a {provider} API key in Portrait Studio settings first")
            }
            other => credential_error("read", provider, other),
        })
}
fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| format!("Could not initialize the portrait provider: {error}"))
}
fn validate_prompt(prompt: &str) -> Result<(), String> {
    let count = prompt.chars().count();
    if count == 0 {
        Err("Enter an art prompt first".into())
    } else if count > MAX_PROMPT_CHARS {
        Err(format!(
            "Portrait prompts are limited to {MAX_PROMPT_CHARS} characters"
        ))
    } else {
        Ok(())
    }
}
fn normalized_model(provider: &str, model: &str) -> String {
    let trimmed = model.trim();
    if !trimmed.is_empty() {
        return trimmed.into();
    }
    match provider {
        "openai" | "custom-openai" => "gpt-image-1",
        "gemini" => "gemini-2.5-flash-image",
        "anthropic" => "claude-sonnet-4-5",
        _ => "",
    }
    .into()
}
fn validate_custom_base_url(value: Option<&str>) -> Result<String, String> {
    let value = value.unwrap_or("").trim().trim_end_matches('/');
    if value.starts_with("https://")
        || value.starts_with("http://127.0.0.1")
        || value.starts_with("http://localhost")
    {
        Ok(value.into())
    } else {
        Err("Custom provider URLs must use HTTPS (or localhost for a local server)".into())
    }
}
fn parse_data_url(value: &str) -> Result<(String, Vec<u8>), String> {
    let (header, data) = value
        .split_once(',')
        .ok_or_else(|| "The reference image is not a valid data URL".to_string())?;
    if !header.starts_with("data:image/") || !header.ends_with(";base64") {
        return Err("The reference must be a base64 image data URL".into());
    }
    let mime = header
        .trim_start_matches("data:")
        .trim_end_matches(";base64")
        .to_string();
    let bytes = STANDARD
        .decode(data)
        .map_err(|_| "The reference image could not be decoded".to_string())?;
    if bytes.len() > MAX_REFERENCE_BYTES {
        return Err("The reference image is larger than 16 MB".into());
    }
    Ok((mime, bytes))
}

fn generate_openai(
    client: &Client,
    base: &str,
    key: &str,
    model: &str,
    prompt: &str,
    reference: Option<(String, Vec<u8>)>,
) -> Result<PortraitGenerateResult, String> {
    let response = if let Some((mime, bytes)) = reference {
        let part = multipart::Part::bytes(bytes)
            .file_name("eq-lineup.png")
            .mime_str(&mime)
            .map_err(|error| format!("Invalid reference image: {error}"))?;
        client
            .post(format!("{base}/images/edits"))
            .bearer_auth(key)
            .multipart(
                multipart::Form::new()
                    .text("model", model.to_string())
                    .text("prompt", prompt.to_string())
                    .text("size", "1536x1024")
                    .part("image", part),
            )
            .send()
    } else {
        client
            .post(format!("{base}/images/generations"))
            .bearer_auth(key)
            .json(&json!({"model":model,"prompt":prompt,"size":"1536x1024"}))
            .send()
    }
    .map_err(|error| format!("OpenAI image request failed: {error}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|error| format!("OpenAI returned an unreadable response: {error}"))?;
    if !status.is_success() {
        return Err(provider_error("OpenAI", status.as_u16(), &body));
    }
    let item = body
        .get("data")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .ok_or_else(|| "OpenAI returned no image".to_string())?;
    let image_data_url = if let Some(data) = item.get("b64_json").and_then(Value::as_str) {
        format!("data:image/png;base64,{data}")
    } else if let Some(url) = item.get("url").and_then(Value::as_str) {
        url.to_string()
    } else {
        return Err("OpenAI returned no usable image data".into());
    };
    Ok(PortraitGenerateResult {
        provider: "openai".into(),
        model: model.into(),
        image_data_url,
        revised_prompt: item
            .get("revised_prompt")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}
fn generate_gemini(
    client: &Client,
    key: &str,
    model: &str,
    prompt: &str,
    reference: Option<(String, Vec<u8>)>,
) -> Result<PortraitGenerateResult, String> {
    let mut parts = vec![json!({"text":prompt})];
    if let Some((mime, bytes)) = reference {
        parts.push(json!({"inlineData":{"mimeType":mime,"data":STANDARD.encode(bytes)}}))
    }
    let response=client.post(format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}")).json(&json!({"contents":[{"role":"user","parts":parts}],"generationConfig":{"responseModalities":["TEXT","IMAGE"]}})).send().map_err(|error|format!("Gemini image request failed: {error}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .map_err(|error| format!("Gemini returned an unreadable response: {error}"))?;
    if !status.is_success() {
        return Err(provider_error("Gemini", status.as_u16(), &body));
    }
    let parts = body
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
        .ok_or_else(|| "Gemini returned no image".to_string())?;
    let image = parts
        .iter()
        .find_map(|part| part.get("inlineData").or_else(|| part.get("inline_data")))
        .ok_or_else(|| "Gemini returned text but no image".to_string())?;
    let mime = image
        .get("mimeType")
        .or_else(|| image.get("mime_type"))
        .and_then(Value::as_str)
        .unwrap_or("image/png");
    let data = image
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| "Gemini returned an empty image".to_string())?;
    let revised_prompt = parts
        .iter()
        .find_map(|part| part.get("text").and_then(Value::as_str))
        .map(str::to_string);
    Ok(PortraitGenerateResult {
        provider: "gemini".into(),
        model: model.into(),
        image_data_url: format!("data:{mime};base64,{data}"),
        revised_prompt,
    })
}
fn provider_error(provider: &str, status: u16, body: &Value) -> String {
    let message = body
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or("The provider rejected the request");
    format!("{provider} request failed ({status}): {message}")
}
fn project_path(id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    {
        return Err(
            "Portrait project IDs may contain only letters, numbers, dashes, and underscores"
                .into(),
        );
    }
    Ok(paths::portrait_projects_directory()
        .map_err(|error| error.to_string())?
        .join(format!("{id}.json")))
}
fn validate_project(project: &PortraitProjectDocument) -> Result<(), String> {
    if project.version != 1 {
        return Err("Unsupported portrait project version".into());
    }
    if project.name.trim().is_empty() || project.name.chars().count() > 100 {
        return Err("Portrait project names must be between 1 and 100 characters".into());
    }
    project_path(&project.id)?;
    if project.characters.len() > 24 {
        return Err("Portrait projects support up to 24 characters".into());
    }
    for character in &project.characters {
        if character.name.trim().is_empty()
            || character.name.chars().count() > 80
            || !character.x.is_finite()
            || !character.y.is_finite()
            || !character.scale.is_finite()
            || !(0.25..=3.0).contains(&character.scale)
        {
            return Err("A portrait character placement is invalid".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_urls_require_tls_except_localhost() {
        assert!(validate_custom_base_url(Some("https://example.test/v1")).is_ok());
        assert!(validate_custom_base_url(Some("http://localhost:11434/v1")).is_ok());
        assert!(validate_custom_base_url(Some("http://example.test/v1")).is_err())
    }
    #[test]
    fn data_url_rejects_non_images() {
        assert!(parse_data_url("data:text/plain;base64,SGk=").is_err())
    }
    #[test]
    fn project_validation_bounds_scale() {
        let project = PortraitProjectDocument {
            version: 1,
            id: "family".into(),
            name: "Family".into(),
            updated_at: String::new(),
            background: "studio".into(),
            aspect_ratio: "16:9".into(),
            characters: vec![PortraitCharacterDocument {
                name: "Valryte".into(),
                x: 0.5,
                y: 0.5,
                scale: 4.0,
                layer: 0,
            }],
        };
        assert!(validate_project(&project).is_err())
    }
}
