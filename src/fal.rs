use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const FAL_BASE: &str = "https://fal.run";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
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
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
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
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
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

/// Render an SVG file to a PNG preview image
pub fn render_svg_to_png(svg_path: &Path, png_path: &Path, size: u32) -> Result<(), String> {
    let svg_data = std::fs::read(svg_path).map_err(|e| format!("Failed to read SVG: {}", e))?;
    let tree = resvg::usvg::Tree::from_data(&svg_data, &resvg::usvg::Options::default())
        .map_err(|e| format!("Failed to parse SVG: {}", e))?;

    let tree_size = tree.size();
    let scale = (size as f32 / tree_size.width()).min(size as f32 / tree_size.height());
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| "Failed to create pixmap".to_string())?;
    // Fill with white background
    pixmap.fill(resvg::tiny_skia::Color::WHITE);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.save_png(png_path).map_err(|e| format!("Failed to save PNG: {}", e))?;
    eprintln!("[fal] rendered SVG preview -> {:?}", png_path);
    Ok(())
}

/// Download an image from URL to disk (blocking)
pub fn download_image(url: &str, save_path: &Path) -> Result<(), String> {
    eprintln!("[fal] downloading {} -> {:?}", url, save_path);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
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
