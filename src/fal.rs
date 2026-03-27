use serde::{Deserialize, Serialize};
use std::path::Path;

const FAL_BASE: &str = "https://fal.run";
const MODEL_TEXT_TO_VECTOR: &str = "fal-ai/recraft/v4/pro/text-to-vector";
const MODEL_KONTEXT: &str = "fal-ai/flux-pro/kontext";

#[derive(Debug, Clone)]
pub struct FalImageResult {
    pub url: String,
    pub content_type: String,
}

#[derive(Debug, Deserialize)]
struct FalResponse {
    images: Vec<FalImageResponse>,
}

#[derive(Debug, Deserialize)]
struct FalImageResponse {
    url: String,
    #[serde(default)]
    content_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct TextToVectorReq {
    prompt: String,
    image_size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<String>,
}

#[derive(Debug, Serialize)]
struct KontextReq {
    prompt: String,
    image_url: String,
    output_format: String,
    aspect_ratio: String,
}

/// Generate a logo via Recraft V4 text-to-vector (blocking, run on background thread)
pub fn generate_logo(
    api_key: &str,
    prompt: &str,
    style: Option<&str>,
) -> Result<FalImageResult, String> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/{}", FAL_BASE, MODEL_TEXT_TO_VECTOR);

    let body = TextToVectorReq {
        prompt: prompt.to_string(),
        image_size: "square_hd".to_string(),
        style: Some(style.unwrap_or("vector_illustration/flat_2").to_string()),
    };

    eprintln!("[fal] POST {} (text-to-vector, prompt={:?})", url, prompt);

    let resp = client
        .post(&url)
        .header("Authorization", format!("Key {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            eprintln!("[fal] request error: {}", e);
            format!("FAL request failed: {}", e)
        })?;

    let status = resp.status();
    let text = resp.text().map_err(|e| format!("Failed to read response: {}", e))?;

    eprintln!("[fal] response status={}, body={}", status, text);

    if !status.is_success() {
        return Err(format!("FAL API error ({}): {}", status, text));
    }

    let fal_resp: FalResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {} - body: {}", e, text))?;

    let img = fal_resp.images.into_iter().next().ok_or("No images in response")?;

    Ok(FalImageResult {
        url: img.url,
        content_type: img.content_type.unwrap_or_else(|| "image/svg+xml".to_string()),
    })
}

/// Evolve a logo via FLUX Kontext (blocking, run on background thread)
pub fn evolve_logo(
    api_key: &str,
    prompt: &str,
    image_data_uri: &str,
) -> Result<FalImageResult, String> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/{}", FAL_BASE, MODEL_KONTEXT);

    let body = KontextReq {
        prompt: prompt.to_string(),
        image_url: image_data_uri.to_string(),
        output_format: "png".to_string(),
        aspect_ratio: "1:1".to_string(),
    };

    eprintln!("[fal] POST {} (kontext evolve, prompt={:?})", url, prompt);

    let resp = client
        .post(&url)
        .header("Authorization", format!("Key {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| {
            eprintln!("[fal] request error: {}", e);
            format!("FAL request failed: {}", e)
        })?;

    let status = resp.status();
    let text = resp.text().map_err(|e| format!("Failed to read response: {}", e))?;

    eprintln!("[fal] response status={}, body={}", status, text);

    if !status.is_success() {
        return Err(format!("FAL API error ({}): {}", status, text));
    }

    let fal_resp: FalResponse =
        serde_json::from_str(&text).map_err(|e| format!("Parse error: {} - body: {}", e, text))?;

    let img = fal_resp.images.into_iter().next().ok_or("No images in response")?;

    Ok(FalImageResult {
        url: img.url,
        content_type: img.content_type.unwrap_or_else(|| "image/png".to_string()),
    })
}

/// Download an image from URL to disk (blocking)
pub fn download_image(url: &str, save_path: &Path) -> Result<(), String> {
    eprintln!("[fal] downloading {} -> {:?}", url, save_path);
    let client = reqwest::blocking::Client::new();
    let resp = client.get(url).send().map_err(|e| {
        eprintln!("[fal] download error: {}", e);
        format!("Download failed: {}", e)
    })?;
    let status = resp.status();
    let bytes = resp.bytes().map_err(|e| format!("Failed to read bytes: {}", e))?;
    eprintln!("[fal] download status={}, size={} bytes", status, bytes.len());
    std::fs::write(save_path, &bytes).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}
