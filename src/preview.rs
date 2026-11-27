use crate::types::{FileEntry, FileType, PreviewContent};
use crate::icons::get_icon;
use tar::Archive;
use anyhow::Result;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

const MAX_TEXT_LINES: usize = 200;
const MAX_ARCHIVE_ENTRIES: usize = 100;
const MAX_DIR_ENTRIES: usize = 200;

pub async fn generate_preview(
    entry: &FileEntry,
    max_size: u64,
    show_images: bool,
    show_icons: bool,
) -> Result<PreviewContent> {
    if entry.is_dir() {
        return Ok(read_dir_preview(&entry.path, show_icons).await);
    }

    if entry.size > max_size {
        return Ok(PreviewContent::text(format!(
            "File is too large to preview\n\n{} (limit: {})",
            format_size(entry.size),
            format_size(max_size)
        )));
    }

    let mime = entry.mime_type.as_deref().unwrap_or("");
    let ext = entry.extension().unwrap_or("").to_lowercase();
    let name = entry.name.to_lowercase();

    // SVG is text to the image crate, so rasterize it first.
    if show_images && (ext == "svg" || mime == "image/svg+xml") {
        if let Some(thumb) = make_thumbnail(&entry.path, ThumbKind::Svg).await {
            return Ok(PreviewContent::image(thumb));
        }
        return read_text_preview(&entry.path).await;
    }

    if show_images && is_image(mime, &ext) {
        return Ok(PreviewContent::image(entry.path.clone()));
    }

    if ext == "pdf" || mime == "application/pdf" {
        if show_images {
            if let Some(thumb) = make_thumbnail(&entry.path, ThumbKind::Pdf).await {
                return Ok(PreviewContent::image(thumb));
            }
        }
        return Ok(read_pdf_text(&entry.path).await);
    }

    if is_archive(&name, &ext, mime) {
        return Ok(read_archive_preview(&entry.path).await);
    }

    if is_video(&ext, mime) {
        if show_images {
            if let Some(thumb) = make_thumbnail(&entry.path, ThumbKind::Video).await {
                return Ok(PreviewContent::image(thumb));
            }
        }
        return Ok(read_media_preview(&entry.path, true).await);
    }

    if is_audio(&ext, mime) {
        return Ok(read_media_preview(&entry.path, false).await);
    }

    if is_text(mime, &ext) {
        return read_text_preview(&entry.path).await;
    }

    Ok(PreviewContent::binary())
}

fn is_image(mime: &str, ext: &str) -> bool {
    mime.starts_with("image/")
        || matches!(
            ext,
            "png"
                | "jpg"
                | "jpeg"
                | "gif"
                | "bmp"
                | "webp"
                | "ico"
                | "avif"
                | "tiff"
                | "tif"
                | "heic"
                | "jxl"
                | "qoi"
        )
}

fn is_archive(name: &str, ext: &str, mime: &str) -> bool {
    name.ends_with(".tar.gz")
        || name.ends_with(".tar.xz")
        || name.ends_with(".tar.bz2")
        || name.ends_with(".tar.zst")
        || name.ends_with(".tar.zstd")
        || matches!(
            ext,
            "zip" | "tar" | "tgz" | "gz" | "xz" | "bz2" | "zst" | "zstd" | "7z" | "rar"
        )
        || mime.contains("zip")
        || mime.contains("gzip")
        || mime.contains("x-tar")
}

fn is_video(ext: &str, mime: &str) -> bool {
    mime.starts_with("video/")
        || matches!(
            ext,
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "flv" | "m4v" | "mpg" | "mpeg"
                | "3gp" | "ogv"
        )
}

fn is_audio(ext: &str, mime: &str) -> bool {
    mime.starts_with("audio/")
        || matches!(
            ext,
            "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma" | "aiff" | "ape"
        )
}

fn is_text(mime: &str, ext: &str) -> bool {
    mime.starts_with("text/")
        || matches!(
            mime,
            "application/json"
                | "application/xml"
                | "application/x-yaml"
                | "application/toml"
                | "application/javascript"
                | "application/x-shellscript"
        )
        || matches!(
            ext,
            "rs" | "py"
                | "js"
                | "ts"
                | "jsx"
                | "tsx"
                | "go"
                | "java"
                | "kt"
                | "cpp"
                | "cc"
                | "c"
                | "h"
                | "hpp"
                | "cs"
                | "php"
                | "rb"
                | "lua"
                | "pl"
                | "sh"
                | "bash"
                | "zsh"
                | "fish"
                | "ps1"
                | "bat"
                | "cmd"
                | "html"
                | "htm"
                | "css"
                | "scss"
                | "sass"
                | "less"
                | "xml"
                | "svg"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
                | "ini"
                | "cfg"
                | "conf"
                | "md"
                | "rst"
                | "txt"
                | "log"
                | "sql"
                | "dockerfile"
                | "lock"
                | "env"
                | "gitignore"
                | "gitconfig"
        )
}

async fn read_text_preview(path: &Path) -> Result<PreviewContent> {
    let bytes = tokio::fs::read(path).await?;

    if bytes.iter().take(8192).any(|b| *b == 0) {
        return Ok(PreviewContent::binary());
    }

    let content = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = content.lines().collect();
    let truncated = lines.len() > MAX_TEXT_LINES;
    lines.truncate(MAX_TEXT_LINES);

    let mut text = lines.join("\n");
    if truncated {
        text.push_str("\n\n… truncated");
    }
    if text.is_empty() {
        text.push_str("[empty file]");
    }

    Ok(PreviewContent::text(text))
}

async fn read_dir_preview(path: &Path, show_icons: bool) -> PreviewContent {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let reader = match std::fs::read_dir(&path) {
            Ok(r) => r,
            Err(e) => return format!("Cannot read directory: {e}"),
        };

        let mut dirs: Vec<(String, String)> = Vec::new();
        let mut files: Vec<(String, String, u64)> = Vec::new();

        for entry in reader.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let file_type = match entry.file_type() {
                Ok(ft) if ft.is_dir() => FileType::Directory,
                Ok(ft) if ft.is_symlink() => FileType::Symlink,
                _ => FileType::File,
            };
            let icon = if show_icons {
                get_icon(&entry.path(), file_type, true).to_string()
            } else {
                String::new()
            };

            if file_type == FileType::Directory {
                dirs.push((icon, name));
            } else {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                files.push((icon, name, size));
            }
        }

        dirs.sort_by_key(|(_, name)| name.to_lowercase());
        files.sort_by_key(|(_, name, _)| name.to_lowercase());

        let total = dirs.len() + files.len();
        let mut out = String::new();
        out.push_str(&format!("{total} items\n\n"));

        for (icon, name) in dirs.iter().take(MAX_DIR_ENTRIES) {
            if icon.is_empty() {
                out.push_str(&format!("▸ {name}/\n"));
            } else {
                out.push_str(&format!("{icon} {name}/\n"));
            }
        }
        for (icon, name, size) in files
            .iter()
            .take(MAX_DIR_ENTRIES.saturating_sub(dirs.len()))
        {
            if icon.is_empty() {
                out.push_str(&format!("  {name}  {}\n", format_size(*size)));
            } else {
                out.push_str(&format!("{icon} {name}  {}\n", format_size(*size)));
            }
        }

        if total > MAX_DIR_ENTRIES {
            out.push_str(&format!("\n… and {} more", total - MAX_DIR_ENTRIES));
        }

        out
    })
    .await;

    PreviewContent::text(result.unwrap_or_else(|_| "[preview unavailable]".to_string()))
}

async fn read_pdf_text(path: &Path) -> PreviewContent {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let output = Command::new("pdftotext")
            .args(["-l", "5", "-layout"])
            .arg(&path)
            .arg("-")
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let mut lines: Vec<&str> = text.lines().collect();
                let truncated = lines.len() > MAX_TEXT_LINES;
                lines.truncate(MAX_TEXT_LINES);
                let mut result = lines.join("\n").trim_end().to_string();

                if result.trim().is_empty() {
                    result = "No extractable text in the first pages.".to_string();
                }
                if truncated {
                    result.push_str("\n\n… truncated (first 5 pages)");
                }
                result
            }
            _ => "[PDF preview needs poppler-utils (pdftotext)]".to_string(),
        }
    })
    .await;

    PreviewContent::text(result.unwrap_or_else(|_| "[preview unavailable]".to_string()))
}

async fn read_archive_preview(path: &Path) -> PreviewContent {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut out = String::new();

        if ext == "zip" {
            out = list_zip(&path);
        } else if name.ends_with(".tar.gz") || ext == "tgz" || ext == "gz" {
            match std::fs::File::open(&path) {
                Ok(file) => out = list_tar(flate2::read::GzDecoder::new(file)),
                Err(e) => out = format!("Cannot open archive: {e}"),
            }
        } else if name.ends_with(".tar.xz") || ext == "xz" {
            match std::fs::File::open(&path) {
                Ok(file) => out = list_tar(xz2::read::XzDecoder::new(file)),
                Err(e) => out = format!("Cannot open archive: {e}"),
            }
        } else if name.ends_with(".tar.bz2") || ext == "bz2" {
            match std::fs::File::open(&path) {
                Ok(file) => out = list_tar(bzip2::read::BzDecoder::new(file)),
                Err(e) => out = format!("Cannot open archive: {e}"),
            }
        } else if name.ends_with(".tar.zst")
            || name.ends_with(".tar.zstd")
            || ext == "zst"
            || ext == "zstd"
        {
            match std::fs::File::open(&path) {
                Ok(file) => match zstd::stream::Decoder::new(file) {
                    Ok(decoder) => out = list_tar(decoder),
                    Err(e) => out = format!("Cannot open archive: {e}"),
                },
                Err(e) => out = format!("Cannot open archive: {e}"),
            }
        } else if ext == "tar" {
            match std::fs::File::open(&path) {
                Ok(file) => out = list_tar(file),
                Err(e) => out = format!("Cannot open archive: {e}"),
            }
        } else if ext == "7z" || ext == "rar" {
            out = format!("[{} archives are not supported]", ext);
        }

        if out.trim().is_empty() {
            out = "Archive is empty".to_string();
        }
        out
    })
    .await;

    PreviewContent::text(result.unwrap_or_else(|_| "[preview unavailable]".to_string()))
}

fn list_zip(path: &Path) -> String {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return format!("Cannot open archive: {e}"),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => return format!("Cannot read zip: {e}"),
    };

    let total = archive.len();
    let mut out = format!("{total} entries\n\n");

    for i in 0..total.min(MAX_ARCHIVE_ENTRIES) {
        match archive.by_index(i) {
            Ok(entry) => {
                let name = entry.name().to_string();
                let size = entry.size();
                out.push_str(&format!("{:>8}  {}\n", format_size(size), name));
            }
            Err(_) => continue,
        }
    }

    if total > MAX_ARCHIVE_ENTRIES {
        out.push_str(&format!("\n… and {} more", total - MAX_ARCHIVE_ENTRIES));
    }
    out
}

fn list_tar<R: std::io::Read>(reader: R) -> String {
    let mut archive = Archive::new(reader);
    let entries = match archive.entries() {
        Ok(e) => e,
        Err(e) => return format!("Cannot read tar: {e}"),
    };

    let mut out = String::new();
    let mut count = 0usize;
    for entry in entries.flatten() {
        if count >= MAX_ARCHIVE_ENTRIES {
            out.push_str("\n… truncated");
            break;
        }
        let name = entry
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let size = entry.header().size().ok().unwrap_or(0);
        out.push_str(&format!("{:>8}  {}\n", format_size(size), name));
        count += 1;
    }

    if out.is_empty() {
        "[not a tar archive]".to_string()
    } else {
        out
    }
}

async fn read_media_preview(path: &Path, video: bool) -> PreviewContent {
    let path = path.to_path_buf();
    let result = tokio::task::spawn_blocking(move || {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(&path)
            .output();

        let out = match output {
            Ok(o) if o.status.success() => o,
            _ => {
                return format!(
                    "{}: {}\n\n[ffprobe not available or failed]",
                    if video { "Video" } else { "Audio" },
                    path.display()
                );
            }
        };

        let json: serde_json::Value = match serde_json::from_slice(&out.stdout) {
            Ok(j) => j,
            Err(e) => return format!("[cannot parse ffprobe output: {e}]"),
        };

        let mut info = String::new();
        let format = json.get("format");

        if let Some(duration) = format
            .and_then(|f| f.get("duration"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
        {
            info.push_str(&format!("Duration   {}\n", format_duration(duration)));
        }
        if let Some(bitrate) = format
            .and_then(|f| f.get("bit_rate"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
        {
            info.push_str(&format!("Bitrate    {} kbps\n", bitrate / 1000));
        }
        if let Some(format_name) = format
            .and_then(|f| f.get("format_long_name"))
            .and_then(|v| v.as_str())
        {
            info.push_str(&format!("Format     {format_name}\n"));
        }

        if let Some(streams) = json.get("streams").and_then(|v| v.as_array()) {
            for stream in streams {
                match stream.get("codec_type").and_then(|v| v.as_str()) {
                    Some("video") => {
                        if let Some(codec) = stream.get("codec_name").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Video      {codec}\n"));
                        }
                        if let (Some(w), Some(h)) = (
                            stream.get("width").and_then(|v| v.as_u64()),
                            stream.get("height").and_then(|v| v.as_u64()),
                        ) {
                            info.push_str(&format!("Resolution {w}x{h}\n"));
                        }
                        if let Some(fps) = stream.get("r_frame_rate").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Frame rate {fps}\n"));
                        }
                    }
                    Some("audio") => {
                        if let Some(codec) = stream.get("codec_name").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Audio      {codec}\n"));
                        }
                        if let Some(rate) = stream.get("sample_rate").and_then(|v| v.as_str()) {
                            info.push_str(&format!("Sample rate {rate} Hz\n"));
                        }
                        if let Some(channels) = stream.get("channels").and_then(|v| v.as_u64()) {
                            info.push_str(&format!("Channels   {channels}\n"));
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(tags) = format
            .and_then(|f| f.get("tags"))
            .and_then(|v| v.as_object())
        {
            let interesting = ["title", "artist", "album", "genre", "date"];
            let mut printed_header = false;
            for key in interesting {
                if let Some(value) = tags.get(key).and_then(|v| v.as_str()) {
                    if !printed_header {
                        info.push_str("\nTags\n");
                        printed_header = true;
                    }
                    info.push_str(&format!("{:<10} {value}\n", capitalize(key)));
                }
            }
        }

        if info.trim().is_empty() {
            info = "[no metadata found]".to_string();
        }

        info
    })
    .await;

    PreviewContent::text(result.unwrap_or_else(|_| "[preview unavailable]".to_string()))
}

fn format_duration(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut value = size as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", value as u64, UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

#[derive(Clone, Copy)]
enum ThumbKind {
    Svg,
    Pdf,
    Video,
}

async fn make_thumbnail(path: &Path, kind: ThumbKind) -> Option<PathBuf> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || build_thumbnail(&path, kind).ok())
        .await
        .ok()
        .flatten()
}

fn thumb_target(path: &Path, kind: ThumbKind) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    if let Ok(modified) = std::fs::metadata(path).and_then(|meta| meta.modified()) {
        if let Ok(age) = modified.duration_since(UNIX_EPOCH) {
            age.as_secs().hash(&mut hasher);
        }
    }
    let prefix = match kind {
        ThumbKind::Svg => "svg",
        ThumbKind::Pdf => "pdf",
        ThumbKind::Video => "video",
    };
    preview_cache_dir().join(format!("{prefix}-{:016x}.png", hasher.finish()))
}

fn build_thumbnail(path: &Path, kind: ThumbKind) -> Result<PathBuf> {
    let out = thumb_target(path, kind);
    if out.exists() {
        return Ok(out);
    }
    std::fs::create_dir_all(preview_cache_dir())?;

    let status = match kind {
        ThumbKind::Svg => Command::new("rsvg-convert")
            .args(["-w", "1200"])
            .arg(path)
            .arg("-o")
            .arg(&out)
            .status()?,
        ThumbKind::Pdf => Command::new("pdftoppm")
            .args([
                "-f",
                "1",
                "-l",
                "1",
                "-singlefile",
                "-png",
                "-scale-to-x",
                "1200",
                "-scale-to-y",
                "-1",
            ])
            .arg(path)
            .arg(out.with_extension(""))
            .status()?,
        ThumbKind::Video => Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-ss", "1", "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-vf", "scale=1200:-2"])
            .arg(&out)
            .status()?,
    };

    if !status.success() || !out.exists() {
        anyhow::bail!("thumbnail command failed");
    }
    Ok(out)
}

/// Temporary directory for generated previews (PDF pages, video frames, SVG).
pub fn preview_cache_dir() -> PathBuf {
    let dir = std::env::temp_dir().join("rfn-previews");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Drop thumbnails older than a week so the cache does not grow forever.
pub fn clean_preview_cache() {
    use std::time::{Duration, SystemTime};

    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(7 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let Ok(entries) = std::fs::read_dir(preview_cache_dir()) else {
        return;
    };

    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .map(|modified| modified < cutoff)
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}
