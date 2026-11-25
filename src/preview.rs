use crate::types::{FileEntry, PreviewContent};
use anyhow::Result;
use std::path::Path;

#[cfg(feature = "image-preview")]
use {base64, image::ImageFormat, sixel};

pub async fn generate_preview(entry: &FileEntry, max_size: u64, show_images: bool) -> Result<PreviewContent> {
    if entry.is_dir() {
        return Ok(PreviewContent::directory());
    }

    if entry.size > max_size {
        return Ok(PreviewContent::text(format!(
            "[File too large for preview: {} bytes]",
            entry.size
        )));
    }

    let mime = entry.mime_type.as_deref().unwrap_or("");
    let is_text = mime.starts_with("text/")
        || mime == "application/json"
        || mime == "application/xml"
        || mime == "application/x-yaml"
        || mime == "application/toml"
        || mime == "application/javascript"
        || mime == "application/x-shellscript"
        || mime == "text/markdown";

    let is_image = show_images
        && (mime.starts_with("image/")
            || matches!(
                entry.extension(),
                Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "svg")
            ));

    if is_image {
        return Ok(PreviewContent::image());
    }

    if is_text {
        return read_text_preview(&entry.path).await;
    }

    Ok(PreviewContent::binary())
}

async fn read_text_preview(path: &Path) -> Result<PreviewContent> {
    let content = tokio::fs::read_to_string(path).await?;
    let lines: Vec<&str> = content.lines().collect();
    let max_lines = 200;
    let preview = lines.into_iter().take(max_lines).collect::<Vec<_>>().join("\n");
    Ok(PreviewContent::text(preview))
}

#[cfg(feature = "image-preview")]
pub fn render_image_preview(
    path: &Path,
    width: u16,
    height: u16,
    backend: crate::config::ImageBackend,
) -> Result<Vec<u8>> {
    let img = image::open(path)?;
    let resized = img.resize(width as u32, height as u32, image::imageops::FilterType::Lanczos3);

    match backend {
        crate::config::ImageBackend::Kitty => render_kitty(&resized),
        crate::config::ImageBackend::Itterm => render_iterm(&resized),
        crate::config::ImageBackend::Sixel => render_sixel(&resized),
        crate::config::ImageBackend::Auto => render_kitty(&resized),
    }
}

#[cfg(feature = "image-preview")]
fn render_kitty(img: &image::DynamicImage) -> Result<Vec<u8>> {
    use std::io::Write;
    let mut buf = Vec::new();
    img.write_to(&mut buf, ImageFormat::Png)?;
    let encoded = base64::encode(&buf);
    let escape = format!("\x1b_Gq=2,a=T,f=100,m=1;{}", encoded);
    Ok(escape.into_bytes())
}

#[cfg(feature = "image-preview")]
fn render_iterm(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    img.write_to(&mut buf, ImageFormat::Png)?;
    let encoded = base64::encode(&buf);
    let escape = format!("\x1b]1337;File=inline=1:{}", encoded);
    Ok(escape.into_bytes())
}

#[cfg(feature = "image-preview")]
fn render_sixel(img: &image::DynamicImage) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    sixel::encode(&mut buf, img, sixel::EncodeOptions::default())?;
    Ok(buf)
}