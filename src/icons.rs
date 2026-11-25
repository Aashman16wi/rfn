use crate::config::ColorConfig;
use crate::types::{FileEntry, FileType};
use crate::ui::parse_color;
use ratatui::style::Color;
use std::path::Path;

pub fn get_icon(path: &Path, file_type: FileType, show_icons: bool) -> &'static str {
    if !show_icons {
        return "";
    }

    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    // Special filenames
    let icon = match name {
        // Config files
        ".gitignore" | ".gitmodules" | ".gitattributes" | ".gitconfig" => "󰊢",
        ".dockerignore" => "󰡨",
        ".editorconfig" => "",
        ".env" | ".env.local" | ".env.production" => "",
        ".eslintrc*" | ".prettierrc*" => "󰱺",
        ".rustfmt.toml" => "",
        ".github" => "",
        ".gitlab" => "",
        ".vscode" => "󰨞",
        ".idea" => "",
        "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => "󰡨",
        "Makefile" | "makefile" | "CMakeLists.txt" | "meson.build" => "",
        "Cargo.toml" | "Cargo.lock" => "",
        "package.json" | "package-lock.json" | "pnpm-lock.yaml" | "yarn.lock" => "󰎙",
        "tsconfig.json" | "jsconfig.json" => "",
        "webpack.config.*" | "vite.config.*" | "rollup.config.*" => "󰜫",
        "pyproject.toml" | "requirements.txt" | "Pipfile" | "poetry.lock" => "󰌠",
        "go.mod" | "go.sum" => "󰟓",
        "composer.json" | "composer.lock" => "󰌟",
        "Gemfile" | "Gemfile.lock" => "󰴭",
        "pom.xml" | "build.gradle*" => "󰬷",
        "README*" | "CHANGELOG*" | "LICENSE*" | "CONTRIBUTING*" => "󰍔",
        _ => "",
    };

    if !icon.is_empty() {
        return icon;
    }

    // By extension
    match ext.as_str() {
        // Rust
        "rs" => "",
        "toml" => "",
        "lock" => "󰌠",

        // JavaScript/TypeScript
        "js" | "jsx" | "mjs" | "cjs" => "󰌞",
        "ts" | "tsx" | "mts" | "cts" => "",
        "json" | "jsonc" => "",

        // Python
        "py" | "pyw" | "pyc" => "󰌠",
        "ipynb" => "󰠮",

        // Go
        "go" | "mod" | "sum" => "󰟓",

        // C/C++
        "c" | "h" => "",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => "󰙲",

        // Java/Kotlin
        "java" => "󰬷",
        "kt" | "kts" => "󰘦",
        "scala" => "󰜷",

        // Web
        "html" | "htm" => "󰌝",
        "css" | "scss" | "sass" | "less" => "󰌛",
        "svg" => "󰜡",

        // Shell
        "sh" | "bash" | "zsh" | "fish" => "",
        "ps1" => "󰨊",

        // Config
        "yaml" | "yml" => "",
        "ini" | "cfg" | "conf" => "",
        "env" => "",

        // Docs
        "md" | "markdown" => "󰍔",
        "txt" | "log" => "󰈙",
        "pdf" => "󰈦",

        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" => "󰈟",

        // Video
        "mp4" | "mkv" | "webm" | "mov" => "󰈫",

        // Audio
        "mp3" | "flac" | "wav" | "ogg" => "󰎆",

        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" => "󰗄",

        // Executables
        "exe" | "msi" | "app" | "apk" => "󰏓",
        "so" | "dylib" | "dll" => "󰆧",

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => "󰛖",

        // Other
        "php" => "󰌟",
        "rb" => "󰴭",
        "lua" => "󰢱",
        "zig" => "󰩳",
        "nim" => "󰆥",
        "dart" => "󰟓",
        "swift" => "󰛥",

        _ => match file_type {
            FileType::Directory => "󰉋",
            FileType::Symlink => "󰉒",
            FileType::File => "󰈙",
            _ => "󰈙",
        },
    }
}

/// Icon tint, roughly yazi-ish: color carries the file kind at a glance.
pub fn icon_color(entry: &FileEntry, colors: &ColorConfig) -> Color {
    if entry.is_dir() {
        return parse_color(&colors.dir);
    }
    if entry.is_symlink() {
        return parse_color(&colors.symlink);
    }
    if entry.permissions & 0o111 != 0 {
        return parse_color(&colors.exec);
    }

    let ext = entry
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        // Rust
        "rs" => Color::Rgb(255, 158, 100),
        "toml" | "lock" => Color::Rgb(187, 154, 247),

        // JS/TS/web
        "js" | "jsx" | "mjs" | "cjs" => Color::Rgb(224, 175, 104),
        "ts" | "tsx" | "mts" | "cts" => Color::Rgb(125, 207, 255),
        "json" | "jsonc" => Color::Rgb(224, 175, 104),
        "html" | "htm" | "css" | "scss" | "sass" | "less" => Color::Rgb(247, 118, 142),
        "svg" => Color::Rgb(255, 158, 100),

        // Python/go/other languages
        "py" | "pyw" | "ipynb" => Color::Rgb(125, 207, 255),
        "go" | "mod" | "sum" => Color::Rgb(115, 218, 202),
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hh" => Color::Rgb(122, 162, 247),
        "java" | "kt" | "kts" | "scala" => Color::Rgb(247, 118, 142),
        "rb" | "php" | "lua" | "zig" | "nim" | "dart" | "swift" => Color::Rgb(187, 154, 247),

        // Shell
        "sh" | "bash" | "zsh" | "fish" | "ps1" => Color::Rgb(158, 206, 106),

        // Docs
        "md" | "markdown" | "txt" | "log" => Color::Rgb(122, 162, 247),
        "pdf" => Color::Rgb(247, 118, 142),

        // Media
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "avif" | "tiff" => {
            Color::Rgb(187, 154, 247)
        }
        "mp4" | "mkv" | "webm" | "mov" | "avi" => Color::Rgb(247, 118, 142),
        "mp3" | "flac" | "wav" | "ogg" | "m4a" => Color::Rgb(158, 206, 106),

        // Archives
        "zip" | "tar" | "gz" | "bz2" | "xz" | "zst" | "7z" | "rar" => Color::Rgb(224, 175, 104),

        // Config
        "yaml" | "yml" | "ini" | "cfg" | "conf" | "env" => Color::Rgb(115, 218, 202),

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" => Color::Rgb(224, 175, 104),

        _ => parse_color(&colors.muted),
    }
}

pub fn git_status_icon(status: &crate::types::GitStatus) -> &'static str {
    match status {
        crate::types::GitStatus::Added => "󰐕",
        crate::types::GitStatus::Modified => "󰷥",
        crate::types::GitStatus::Deleted => "󰍵",
        crate::types::GitStatus::Renamed => "󰑕",
        crate::types::GitStatus::Copied => "󰅍",
        crate::types::GitStatus::Untracked => "󰞋",
        crate::types::GitStatus::Ignored => "󰞋",
        crate::types::GitStatus::Conflicted => "󰈊",
        crate::types::GitStatus::Unmodified => "",
    }
}

pub fn git_status_color(status: &crate::types::GitStatus) -> (u8, u8, u8) {
    match status {
        crate::types::GitStatus::Added => (0, 200, 0),
        crate::types::GitStatus::Modified => (255, 170, 0),
        crate::types::GitStatus::Deleted => (255, 50, 50),
        crate::types::GitStatus::Renamed => (0, 200, 200),
        crate::types::GitStatus::Copied => (0, 200, 200),
        crate::types::GitStatus::Untracked => (255, 150, 0),
        crate::types::GitStatus::Ignored => (128, 128, 128),
        crate::types::GitStatus::Conflicted => (255, 0, 255),
        crate::types::GitStatus::Unmodified => (100, 100, 100),
    }
}