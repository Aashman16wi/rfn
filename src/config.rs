use config::{Config, Environment, File};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub behavior: BehaviorConfig,
    pub preview: PreviewConfig,
    pub git: GitConfig,
    pub keys: KeyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub show_icons: bool,
    pub show_hidden: bool,
    pub show_git_status: bool,
    pub show_parent: bool,
    /// Built-in color theme, or "custom" to use the [ui.colors] values.
    pub theme: String,
    pub colors: ColorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorConfig {
    pub bg: String,
    pub fg: String,
    pub muted: String,
    pub accent: String,
    pub border: String,
    pub topbar_bg: String,
    pub status_bg: String,
    pub selected_bg: String,
    pub selected_fg: String,
    pub dir: String,
    pub file: String,
    pub symlink: String,
    pub exec: String,
    pub git_added: String,
    pub git_modified: String,
    pub git_deleted: String,
    pub git_untracked: String,
    pub git_ignored: String,
    pub preview_bg: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub confirm_delete: bool,
    pub use_trash: bool,
    pub confirm_quit: bool,
    pub sort_by: SortBy,
    pub sort_dirs_first: bool,
    pub reverse_sort: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    Name,
    Size,
    Modified,
    Extension,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewConfig {
    pub enabled: bool,
    pub max_size: u64,
    pub show_images: bool,
    pub image_backend: ImageBackend,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageBackend {
    Kitty,
    #[serde(alias = "itterm")]
    Iterm,
    Sixel,
    Chafa,
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub quit: Vec<String>,
    pub up: Vec<String>,
    pub down: Vec<String>,
    pub left: Vec<String>,
    pub right: Vec<String>,
    pub enter: Vec<String>,
    pub backspace: Vec<String>,
    pub delete: Vec<String>,
    pub rename: Vec<String>,
    pub new_file: Vec<String>,
    pub new_dir: Vec<String>,
    pub copy: Vec<String>,
    pub paste: Vec<String>,
    pub cut: Vec<String>,
    pub search: Vec<String>,
    pub toggle_hidden: Vec<String>,
    pub toggle_preview: Vec<String>,
    pub toggle_git: Vec<String>,
    pub help: Vec<String>,
    pub refresh: Vec<String>,
    pub goto_parent: Vec<String>,
    pub goto_home: Vec<String>,
    pub sort: Vec<String>,
    pub reverse_sort: Vec<String>,
    pub tab_new: Vec<String>,
    pub tab_close: Vec<String>,
    pub tab_next: Vec<String>,
    pub tab_prev: Vec<String>,
    pub visual: Vec<String>,
    pub mark: Vec<String>,
    pub bulk_rename: Vec<String>,
    pub preview_scroll_up: Vec<String>,
    pub preview_scroll_down: Vec<String>,
    pub preview_zoom_in: Vec<String>,
    pub preview_zoom_out: Vec<String>,
    pub preview_reset: Vec<String>,
    pub bookmark_add: Vec<String>,
    pub bookmark_jump: Vec<String>,
    pub trash: Vec<String>,
    pub open_with: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui: UiConfig::default(),
            behavior: BehaviorConfig::default(),
            preview: PreviewConfig::default(),
            git: GitConfig::default(),
            keys: KeyConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_icons: true,
            show_hidden: false,
            show_git_status: true,
            show_parent: true,
            theme: "tokyo-night".to_string(),
            colors: ColorConfig::default(),
        }
    }
}

impl ColorConfig {
    pub const THEMES: [&'static str; 4] =
        ["tokyo-night", "catppuccin-mocha", "gruvbox-dark", "nord"];

    pub fn preset(name: &str) -> Option<Self> {
        // bg, fg, muted, accent, border, topbar_bg, status_bg, selected_bg,
        // selected_fg, dir, file, symlink, exec, git_added, git_modified,
        // git_deleted, git_untracked, git_ignored, preview_bg
        let palette: [&str; 19] = match name {
            "tokyo-night" => [
                "#1a1b26", "#c0caf5", "#565f89", "#7aa2f7", "#2f3549", "#16161e",
                "#16161e", "#283457", "#c0caf5", "#7aa2f7", "#c0caf5", "#bb9af7",
                "#9ece6a", "#9ece6a", "#e0af68", "#f7768e", "#ff9e64", "#565f89",
                "#1a1b26",
            ],
            "catppuccin-mocha" => [
                "#1e1e2e", "#cdd6f4", "#6c7086", "#89b4fa", "#313244", "#181825",
                "#181825", "#45475a", "#cdd6f4", "#89b4fa", "#cdd6f4", "#cba6f7",
                "#a6e3a1", "#a6e3a1", "#f9e2af", "#f38ba8", "#fab387", "#6c7086",
                "#1e1e2e",
            ],
            "gruvbox-dark" => [
                "#282828", "#ebdbb2", "#928374", "#83a598", "#3c3836", "#1d2021",
                "#1d2021", "#504945", "#fbf1c7", "#83a598", "#ebdbb2", "#d3869b",
                "#b8bb26", "#b8bb26", "#fabd2f", "#fb4934", "#fe8019", "#928374",
                "#282828",
            ],
            "nord" => [
                "#2e3440", "#d8dee9", "#4c566a", "#88c0d0", "#3b4252", "#292e39",
                "#292e39", "#434c5e", "#eceff4", "#88c0d0", "#d8dee9", "#b48ead",
                "#a3be8c", "#a3be8c", "#ebcb8b", "#bf616a", "#d08770", "#4c566a",
                "#2e3440",
            ],
            _ => return None,
        };

        Some(Self {
            bg: palette[0].to_string(),
            fg: palette[1].to_string(),
            muted: palette[2].to_string(),
            accent: palette[3].to_string(),
            border: palette[4].to_string(),
            topbar_bg: palette[5].to_string(),
            status_bg: palette[6].to_string(),
            selected_bg: palette[7].to_string(),
            selected_fg: palette[8].to_string(),
            dir: palette[9].to_string(),
            file: palette[10].to_string(),
            symlink: palette[11].to_string(),
            exec: palette[12].to_string(),
            git_added: palette[13].to_string(),
            git_modified: palette[14].to_string(),
            git_deleted: palette[15].to_string(),
            git_untracked: palette[16].to_string(),
            git_ignored: palette[17].to_string(),
            preview_bg: palette[18].to_string(),
        })
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self::preset("tokyo-night").expect("tokyo-night preset exists")
    }
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            confirm_delete: true,
            use_trash: true,
            confirm_quit: false,
            sort_by: SortBy::Name,
            sort_dirs_first: true,
            reverse_sort: false,
        }
    }
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size: 10_000_000,
            show_images: true,
            image_backend: ImageBackend::Auto,
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
        }
    }
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            quit: vec!["q".into(), "ctrl+c".into()],
            up: vec!["k".into(), "up".into()],
            down: vec!["j".into(), "down".into()],
            left: vec!["h".into(), "left".into()],
            right: vec!["l".into(), "right".into()],
            enter: vec!["enter".into(), "l".into()],
            backspace: vec!["backspace".into(), "h".into()],
            delete: vec!["d".into(), "delete".into()],
            rename: vec!["r".into()],
            new_file: vec!["n".into()],
            new_dir: vec!["shift+n".into()],
            copy: vec!["y".into()],
            paste: vec!["p".into()],
            cut: vec!["x".into()],
            search: vec!["/".into()],
            toggle_hidden: vec![".".into()],
            toggle_preview: vec!["P".into()],
            toggle_git: vec!["g".into()],
            help: vec!["?".into()],
            refresh: vec!["R".into(), "ctrl+r".into()],
            goto_parent: vec!["H".into(), "ctrl+h".into()],
            goto_home: vec!["~".into()],
            sort: vec!["s".into()],
            reverse_sort: vec!["S".into()],
            tab_new: vec!["t".into()],
            tab_close: vec!["w".into()],
            tab_next: vec!["tab".into()],
            tab_prev: vec!["backtab".into()],
            visual: vec!["v".into()],
            mark: vec!["space".into()],
            bulk_rename: vec!["b".into()],
            preview_scroll_up: vec!["K".into()],
            preview_scroll_down: vec!["J".into()],
            preview_zoom_in: vec!["=".into()],
            preview_zoom_out: vec!["-".into()],
            preview_reset: vec!["0".into()],
            bookmark_add: vec!["m".into()],
            bookmark_jump: vec!["M".into()],
            trash: vec!["T".into()],
            open_with: vec!["o".into()],
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            .set_default("ui.show_icons", true)?
            .set_default("ui.show_hidden", false)?
            .set_default("ui.show_git_status", true)?
            .set_default("ui.show_parent", true)?
            .set_default("ui.theme", "tokyo-night")?
            .set_default("ui.colors.bg", "#1a1b26")?
            .set_default("ui.colors.fg", "#c0caf5")?
            .set_default("ui.colors.muted", "#565f89")?
            .set_default("ui.colors.accent", "#7aa2f7")?
            .set_default("ui.colors.border", "#2f3549")?
            .set_default("ui.colors.topbar_bg", "#16161e")?
            .set_default("ui.colors.status_bg", "#16161e")?
            .set_default("ui.colors.selected_bg", "#283457")?
            .set_default("ui.colors.selected_fg", "#c0caf5")?
            .set_default("ui.colors.dir", "#7aa2f7")?
            .set_default("ui.colors.file", "#c0caf5")?
            .set_default("ui.colors.symlink", "#bb9af7")?
            .set_default("ui.colors.exec", "#9ece6a")?
            .set_default("ui.colors.git_added", "#9ece6a")?
            .set_default("ui.colors.git_modified", "#e0af68")?
            .set_default("ui.colors.git_deleted", "#f7768e")?
            .set_default("ui.colors.git_untracked", "#ff9e64")?
            .set_default("ui.colors.git_ignored", "#565f89")?
            .set_default("ui.colors.preview_bg", "#1a1b26")?
            .set_default("behavior.confirm_delete", true)?
            .set_default("behavior.use_trash", true)?
            .set_default("behavior.confirm_quit", false)?
            .set_default("behavior.sort_by", "name")?
            .set_default("behavior.sort_dirs_first", true)?
            .set_default("behavior.reverse_sort", false)?
            .set_default("preview.enabled", true)?
            .set_default("preview.max_size", 10_000_000)?
            .set_default("preview.show_images", true)?
            .set_default("preview.image_backend", "auto")?
            .set_default("git.enabled", true)?
            .set_default("keys.quit", vec!["q", "ctrl+c"])?
            .set_default("keys.up", vec!["k", "up"])?
            .set_default("keys.down", vec!["j", "down"])?
            .set_default("keys.left", vec!["h", "left"])?
            .set_default("keys.right", vec!["l", "right"])?
            .set_default("keys.enter", vec!["enter", "l"])?
            .set_default("keys.backspace", vec!["backspace", "h"])?
            .set_default("keys.delete", vec!["d", "delete"])?
            .set_default("keys.rename", vec!["r"])?
            .set_default("keys.new_file", vec!["n"])?
            .set_default("keys.new_dir", vec!["shift+n"])?
            .set_default("keys.copy", vec!["y"])?
            .set_default("keys.paste", vec!["p"])?
            .set_default("keys.cut", vec!["x"])?
            .set_default("keys.search", vec!["/"])?
            .set_default("keys.toggle_hidden", vec!["."])?
            .set_default("keys.toggle_preview", vec!["P"])?
            .set_default("keys.toggle_git", vec!["g"])?
            .set_default("keys.help", vec!["?"])?
            .set_default("keys.refresh", vec!["R", "ctrl+r"])?
            .set_default("keys.goto_parent", vec!["H", "ctrl+h"])?
            .set_default("keys.goto_home", vec!["~"])?
            .set_default("keys.sort", vec!["s"])?
            .set_default("keys.reverse_sort", vec!["S"])?
            .set_default("keys.tab_new", vec!["t"])?
            .set_default("keys.tab_close", vec!["w"])?
            .set_default("keys.tab_next", vec!["tab"])?
            .set_default("keys.tab_prev", vec!["backtab"])?
            .set_default("keys.visual", vec!["v"])?
            .set_default("keys.mark", vec!["space"])?
            .set_default("keys.bulk_rename", vec!["b"])?
            .set_default("keys.preview_scroll_up", vec!["K"])?
            .set_default("keys.preview_scroll_down", vec!["J"])?
            .set_default("keys.preview_zoom_in", vec!["="])?
            .set_default("keys.preview_zoom_out", vec!["-"])?
            .set_default("keys.preview_reset", vec!["0"])?
            .set_default("keys.bookmark_add", vec!["m"])?
            .set_default("keys.bookmark_jump", vec!["M"])?
            .set_default("keys.trash", vec!["T"])?
            .set_default("keys.open_with", vec!["o"])?;

        if let Some(config_dir) = config_dir() {
            let config_path = config_dir.join("rfn").join("config.toml");
            if config_path.exists() {
                builder = builder.add_source(File::from(config_path).required(false));
            }
        }

        builder = builder.add_source(
            Environment::with_prefix("RFN")
                .separator("__")
                .try_parsing(true),
        );

        let mut config: AppConfig = builder.build()?.try_deserialize()?;
        if config.ui.theme != "custom" {
            if let Some(colors) = ColorConfig::preset(&config.ui.theme) {
                config.ui.colors = colors;
            }
        }
        Ok(config)
    }

    pub fn generate_default() -> String {
        toml::to_string_pretty(&Self::default()).unwrap()
    }
}