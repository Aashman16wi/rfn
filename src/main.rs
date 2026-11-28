mod app;
mod config;
mod fs;
mod icons;
mod keys;
mod preview;
mod state;
mod types;
mod ui;

use crate::app::App;
use crate::config::AppConfig;
use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dirs;
use ratatui::backend::CrosstermBackend;
use std::io;

#[derive(Parser, Debug)]
#[command(name = "rfn", version, about = "A minimal async terminal file manager")]
struct Args {
    /// Path to start in (default: home directory, restoring saved tabs)
    path: Option<String>,

    /// Generate default config file to stdout
    #[arg(long)]
    generate_config: bool,

    /// List built-in color themes with previews and mark the active one
    #[arg(long)]
    themes: bool,

    /// Set the color theme (use --themes to list names)
    #[arg(long, value_name = "NAME", num_args = 0..=1, default_missing_value = "")]
    theme: Option<String>,

    /// Show help
    #[arg(short, long)]
    help: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Rust ignores SIGPIPE by default, which turns `rfn --themes | head` into
    // a panic/abort. Restore the normal Unix behavior for command-line output.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args = Args::parse();

    if args.help {
        print_help();
        return Ok(());
    }

    if args.generate_config {
        println!("{}", AppConfig::generate_default());
        return Ok(());
    }

    if args.themes {
        print_themes(&current_theme());
        return Ok(());
    }

    if let Some(theme) = args.theme {
        if theme.is_empty() {
            print_themes(&current_theme());
        } else {
            set_theme(&theme)?;
        }
        return Ok(());
    }

    let config = AppConfig::load()?;
    let picker = crate::app::create_picker(&config);
    crate::preview::clean_preview_cache();

    let restore_tabs = args.path.is_none();
    let start_path = args
        .path
        .map(std::path::PathBuf::from)
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new(config, start_path, picker, restore_tabs).await?;

    let result = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn color_enabled() -> bool {
    use std::io::IsTerminal;
    io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((red, green, blue))
}

fn ansi_fg(hex: &str) -> String {
    match hex_rgb(hex) {
        Some((red, green, blue)) => format!("\x1b[38;2;{red};{green};{blue}m"),
        None => String::new(),
    }
}

fn ansi_bg(hex: &str) -> String {
    match hex_rgb(hex) {
        Some((red, green, blue)) => format!("\x1b[48;2;{red};{green};{blue}m"),
        None => String::new(),
    }
}

fn theme_swatches(colors: &crate::config::ColorConfig) -> String {
    let entries = [
        ("bg", &colors.bg),
        ("fg", &colors.fg),
        ("accent", &colors.accent),
        ("dir", &colors.dir),
        ("exec", &colors.exec),
        ("link", &colors.symlink),
        ("added", &colors.git_added),
        ("deleted", &colors.git_deleted),
    ];

    if !color_enabled() {
        return entries
            .iter()
            .map(|(label, hex)| format!("{label} {hex}"))
            .collect::<Vec<_>>()
            .join("  ");
    }

    entries
        .iter()
        .map(|(label, hex)| {
            format!(
                "{}{} {}\x1b[0m",
                ansi_fg(&colors.muted),
                label,
                format!("{}{}", ansi_fg(hex), "███")
            )
        })
        .collect::<Vec<_>>()
        .join("  ")
}

fn theme_sample(colors: &crate::config::ColorConfig) -> String {
    if !color_enabled() {
        return "src/  README.md  install.sh  link  M app.rs".to_string();
    }

    let mut out = ansi_bg(&colors.bg);
    out.push_str(&ansi_fg(&colors.dir));
    out.push_str("src/");
    out.push_str(&ansi_fg(&colors.fg));
    out.push_str("  README.md");
    out.push_str(&ansi_fg(&colors.exec));
    out.push_str("  install.sh");
    out.push_str(&ansi_fg(&colors.symlink));
    out.push_str("  link");
    out.push_str(&ansi_fg(&colors.git_modified));
    out.push_str("  M app.rs");
    out.push_str("\x1b[0m");
    out
}

fn current_theme() -> String {
    AppConfig::load()
        .map(|config| config.ui.theme)
        .unwrap_or_else(|_| "tokyo-night".to_string())
}

fn print_themes(current: &str) {
    println!("rfn built-in themes\n");

    let mut found_current = false;
    for name in crate::config::ColorConfig::THEMES {
        let Some(colors) = crate::config::ColorConfig::preset(name) else {
            continue;
        };
        let active = name == current;
        found_current |= active;
        println!(
            "  {} {:<18}{}",
            if active { "*" } else { " " },
            name,
            if active { " (active)" } else { "" }
        );
        println!("      {}", theme_swatches(&colors));
        println!("      {}", theme_sample(&colors));
        println!();
    }

    if !found_current {
        if current == "custom" {
            println!("  Active theme: custom (uses [ui.colors] from the config)\n");
        } else {
            println!("  Active theme: {current} (not a built-in theme)\n");
        }
    }

    println!("Set one with: rfn --theme <name>");
}

fn set_theme(name: &str) -> Result<()> {
    let preset = crate::config::ColorConfig::preset(name);
    if preset.is_none() && name != "custom" {
        eprintln!("error: unknown theme '{name}'");
        eprintln!(
            "available themes: {}",
            crate::config::ColorConfig::THEMES.join(", ")
        );
        eprintln!("run `rfn --themes` to preview them");
        std::process::exit(2);
    }

    let path = AppConfig::save_theme(name)?;

    println!("Theme set to {name}");
    match preset {
        Some(colors) => {
            println!("  {}", theme_swatches(&colors));
            println!("  {}", theme_sample(&colors));
        }
        None => println!("  custom: edit [ui.colors] in the config below"),
    }
    println!("\nConfig: {}", path.display());
    Ok(())
}

fn print_help() {
    println!(r#"rfn - A minimal async terminal file manager

USAGE:
    rfn [OPTIONS] [PATH]

ARGS:
    <PATH>    Path to start in (default: home directory)

OPTIONS:
    -h, --help              Print help
        --generate-config   Generate default config file to stdout
        --themes            List themes with color previews and mark the active one
        --theme <NAME>      Set the theme in ~/.config/rfn/config.toml

KEYBINDINGS:
    Navigation:
        j / Down        Move down
        k / Up          Move up
        h / Left        Parent directory
        l / Right / Enter   Enter directory / open file
        H / Ctrl+h      Go to parent
        ~               Go to home
        Home            Go to top
        G / End         Go to bottom
        Ctrl+u / Ctrl+d Page up / down
        Mouse wheel     Scroll
        Left click      Select

    File Operations:
        n               New file
        N / Shift+n     New directory
        r               Rename
        d / Delete      Delete (trash by default)
        y               Copy (yank)
        x               Cut
        p               Paste (queued in the background)
        Q               Open the transfer queue
        T               Trash browser
        o               Open with...
        b               Bulk rename marked files

    Selection:
        v               Visual mode
        Space           Toggle mark
        Esc             Leave visual mode / clear marks

    Tabs:
        t               New tab
        w               Close tab
        Tab / Shift+Tab Next / previous tab
        1 - 9           Jump to tab

    View:
        .               Toggle hidden files
        P               Toggle preview
        g               Toggle git status
        R / Ctrl+r      Refresh
        m / M           Bookmark current dir / bookmark list
        J / K / 0       Scroll preview / reset
        Ctrl+t          Change theme (in-app picker)

    Sorting:
        s               Cycle name -> size -> time -> extension
        S               Reverse sort order

    Other:
        /               Search
        ?               Toggle help
        q / Ctrl+c      Quit
"#);
}