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
    #[error("toml error: {0}")]
    Toml(#[from] toml::ser::Error),
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
    /// The colors of the built-in theme. They are kept apart from `colors`,
    /// so switching back to "custom" still finds the colors of the user.
    #[serde(skip)]
    pub theme_colors: Option<ColorConfig>,
    /// The colors of the user, used when the theme is "custom".
    pub colors: ColorConfig,
}

impl UiConfig {
    /// The colors that are on the screen right now.
    pub fn active_colors(&self) -> &ColorConfig {
        self.theme_colors.as_ref().unwrap_or(&self.colors)
    }
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
    pub queue: Vec<String>,
    pub theme: Vec<String>,
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
            theme_colors: ColorConfig::preset("tokyo-night"),
            colors: ColorConfig::default(),
        }
    }
}

impl ColorConfig {
    /// The name of the theme that uses the colors of the user.
    pub const CUSTOM: &'static str = "custom";

    pub const THEMES: [&'static str; 8] = [
        "tokyo-night",
        "catppuccin-mocha",
        "catppuccin-latte",
        "gruvbox-dark",
        "gruvbox-light",
        "nord",
        "dracula",
        "rose-pine",
    ];

    /// Every theme the picker shows: the colors of the user first, then the
    /// built-in ones.
    pub fn picker_list() -> Vec<&'static str> {
        let mut names = vec![Self::CUSTOM];
        names.extend_from_slice(&Self::THEMES);
        names
    }

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
            "catppuccin-latte" => [
                "#eff1f5", "#4c4f69", "#9ca0b0", "#1e66f5", "#ccd0da", "#e6e9ef",
                "#e6e9ef", "#bcc0cc", "#4c4f69", "#1e66f5", "#4c4f69", "#8839ef",
                "#40a02b", "#40a02b", "#df8e1d", "#d20f39", "#fe640b", "#9ca0b0",
                "#eff1f5",
            ],
            "gruvbox-dark" => [
                "#282828", "#ebdbb2", "#928374", "#83a598", "#3c3836", "#1d2021",
                "#1d2021", "#504945", "#fbf1c7", "#83a598", "#ebdbb2", "#d3869b",
                "#b8bb26", "#b8bb26", "#fabd2f", "#fb4934", "#fe8019", "#928374",
                "#282828",
            ],
            "gruvbox-light" => [
                "#fbf1c7", "#3c3836", "#7c6f64", "#076678", "#d5c4a1", "#ebdbb2",
                "#ebdbb2", "#d5c4a1", "#3c3836", "#076678", "#3c3836", "#8f3f71",
                "#98971a", "#98971a", "#b57614", "#9d0006", "#b57614", "#7c6f64",
                "#fbf1c7",
            ],
            "nord" => [
                "#2e3440", "#d8dee9", "#4c566a", "#88c0d0", "#3b4252", "#292e39",
                "#292e39", "#434c5e", "#eceff4", "#88c0d0", "#d8dee9", "#b48ead",
                "#a3be8c", "#a3be8c", "#ebcb8b", "#bf616a", "#d08770", "#4c566a",
                "#2e3440",
            ],
            "dracula" => [
                "#282a36", "#f8f8f2", "#6272a4", "#bd93f9", "#44475a", "#21222c",
                "#21222c", "#44475a", "#f8f8f2", "#8be9fd", "#f8f8f2", "#ff79c6",
                "#50fa7b", "#50fa7b", "#f1fa8c", "#ff5555", "#ffb86c", "#6272a4",
                "#282a36",
            ],
            "rose-pine" => [
                "#191724", "#e0def4", "#6e6a86", "#ebbcba", "#26233a", "#1f1d2e",
                "#1f1d2e", "#403d52", "#e0def4", "#31748f", "#e0def4", "#c4a7e7",
                "#f6c177", "#9ccfd8", "#f6c177", "#eb6f92", "#f6c177", "#6e6a86",
                "#191724",
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
            queue: vec!["Q".into()],
            theme: vec!["ctrl+t".into()],
        }
    }
}

impl AppConfig {
    /// Path of the user config file.
    pub fn config_path() -> std::path::PathBuf {
        config_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join("rfn")
            .join("config.toml")
    }

    /// Persist the selected theme, keeping every other setting untouched.
    pub fn save_theme(name: &str) -> Result<std::path::PathBuf, ConfigError> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            // toml_edit keeps the comments, the spacing and the order of the
            // sections. Parsing the file as plain TOML and writing it back
            // would throw away everything the user wrote themselves.
            let mut document = text.parse::<toml_edit::DocumentMut>().map_err(|error| {
                ConfigError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("cannot parse {}: {error}", path.display()),
                ))
            })?;

            match document.get_mut("ui") {
                Some(toml_edit::Item::Table(table)) => {
                    table["theme"] = toml_edit::value(name);
                }
                Some(_) => {
                    return Err(ConfigError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "ui is not a table in the config",
                    )));
                }
                None => {
                    let mut table = toml_edit::Table::new();
                    table["theme"] = toml_edit::value(name);
                    document["ui"] = toml_edit::Item::Table(table);
                }
            }

            std::fs::write(&path, document.to_string())?;
        } else {
            let mut config = AppConfig::default();
            config.ui.theme = name.to_string();
            std::fs::write(&path, toml::to_string_pretty(&config)?)?;
        }

        Ok(path)
    }

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
            .set_default("keys.open_with", vec!["o"])?
            .set_default("keys.queue", vec!["Q"])?
            .set_default("keys.theme", vec!["ctrl+t"])?;

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
        // The colors of a built-in theme are kept apart from the colors of
        // the user, so switching back to "custom" still finds them.
        config.ui.theme_colors = ColorConfig::preset(&config.ui.theme);
        Ok(config)
    }

    pub fn generate_default() -> String {
        toml::to_string_pretty(&Self::default()).unwrap()
    }
}