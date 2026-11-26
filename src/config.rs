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
            colors: ColorConfig::default(),
        }
    }
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            bg: "#1a1b26".to_string(),
            fg: "#c0caf5".to_string(),
            muted: "#565f89".to_string(),
            accent: "#7aa2f7".to_string(),
            border: "#2f3549".to_string(),
            topbar_bg: "#16161e".to_string(),
            status_bg: "#16161e".to_string(),
            selected_bg: "#283457".to_string(),
            selected_fg: "#c0caf5".to_string(),
            dir: "#7aa2f7".to_string(),
            file: "#c0caf5".to_string(),
            symlink: "#bb9af7".to_string(),
            exec: "#9ece6a".to_string(),
            git_added: "#9ece6a".to_string(),
            git_modified: "#e0af68".to_string(),
            git_deleted: "#f7768e".to_string(),
            git_untracked: "#ff9e64".to_string(),
            git_ignored: "#565f89".to_string(),
            preview_bg: "#1a1b26".to_string(),
        }
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
            .set_default("keys.bulk_rename", vec!["b"])?;

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

        let config = builder.build()?.try_deserialize()?;
        Ok(config)
    }

    pub fn generate_default() -> String {
        toml::to_string_pretty(&Self::default()).unwrap()
    }
}