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

    /// List built-in color themes
    #[arg(long)]
    themes: bool,

    /// Show help
    #[arg(short, long)]
    help: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
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
        for theme in crate::config::ColorConfig::THEMES {
            println!("{theme}");
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

fn print_help() {
    println!(r#"rfn - A minimal async terminal file manager

USAGE:
    rfn [OPTIONS] [PATH]

ARGS:
    <PATH>    Path to start in (default: home directory)

OPTIONS:
    -h, --help              Print help
        --generate-config   Generate default config file to stdout

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
        d / Delete      Delete
        y               Copy (yank)
        x               Cut
        p               Paste

    View:
        .               Toggle hidden files
        P               Toggle preview
        g               Toggle git status
        R / Ctrl+r      Refresh

    Sorting:
        s               Cycle name -> size -> time -> extension
        S               Reverse sort order

    Other:
        /               Search
        Esc             Clear search
        ?               Toggle help
        q / Ctrl+c      Quit
"#);
}