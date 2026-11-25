mod app;
mod config;
mod fs;
mod icons;
mod keys;
mod preview;
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
use ratatui::backend::CrosstermBackend;
use std::io;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(name = "rfn", version, about = "A minimal async terminal file manager")]
struct Args {
    /// Path to start in
    #[arg(default_value = ".")]
    path: String,

    /// Generate default config file to stdout
    #[arg(long)]
    generate_config: bool,

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

    let config = AppConfig::load()?;

    let start_path = std::path::PathBuf::from(&args.path);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = App::new(config, start_path).await?;

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
    <PATH>    Path to start in (default: current directory)

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
        gg              Go to top
        G               Go to bottom
        Ctrl+u / Ctrl+d Page up / down

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
        sn              Sort by name
        ss              Sort by size
        st              Sort by time
        se              Sort by extension
        sr              Reverse sort

    Other:
        /               Search
        ?               Toggle help
        q / Ctrl+c      Quit
"#);
}