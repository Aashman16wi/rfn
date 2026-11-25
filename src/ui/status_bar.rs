use crate::types::{AppMode, ClipboardOp, FileEntry, GitStatus};
use crate::config::ColorConfig;
use crate::ui::parse_color;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    mode: AppMode,
    current_path: &std::path::Path,
    selected_entry: Option<&FileEntry>,
    clipboard: ClipboardOp,
    clipboard_count: usize,
    show_hidden: bool,
    show_preview: bool,
    show_git: bool,
    sort_by: crate::config::SortBy,
    reverse_sort: bool,
    colors: &ColorConfig,
) {
    let mut spans = Vec::new();

    // Mode indicator
    let mode_str = match mode {
        AppMode::Normal => "NORMAL",
        AppMode::Search => "SEARCH",
        AppMode::Rename => "RENAME",
        AppMode::NewFile => "NEW FILE",
        AppMode::NewDir => "NEW DIR",
        AppMode::ConfirmDelete => "CONFIRM DEL",
        AppMode::ConfirmQuit => "CONFIRM QUIT",
        AppMode::Help => "HELP",
    };
    spans.push(Span::styled(
        format!(" {} ", mode_str),
        Style::default()
            .bg(parse_color(&colors.selected_bg))
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Path
    spans.push(Span::styled(
        format!("{}", current_path.display()),
        Style::default().fg(Color::Cyan),
    ));

    spans.push(Span::raw(" "));

    // Selected file info
    if let Some(entry) = selected_entry {
        let size_str = format_size(entry.size);
        spans.push(Span::styled(
            format!("{} ", size_str),
            Style::default().fg(Color::Yellow),
        ));

        if entry.git_status != GitStatus::Unmodified {
            let (r, g, b) = crate::icons::git_status_color(&entry.git_status);
            spans.push(Span::styled(
                format!("[{}] ", git_status_str(&entry.git_status)),
                Style::default().fg(Color::Rgb(r, g, b)),
            ));
        }
    }

    spans.push(Span::raw(" "));

    // Clipboard
    if clipboard != ClipboardOp::None && clipboard_count > 0 {
        let op = match clipboard {
            ClipboardOp::Copy => "COPY",
            ClipboardOp::Cut => "CUT",
            _ => "",
        };
        spans.push(Span::styled(
            format!("{} {} ", op, clipboard_count),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ));
    }

    // Flags
    let mut flags = Vec::new();
    if show_hidden {
        flags.push("H");
    }
    if show_preview {
        flags.push("P");
    }
    if show_git {
        flags.push("G");
    }
    if reverse_sort {
        flags.push("R");
    }
    let sort_char = match sort_by {
        crate::config::SortBy::Name => "n",
        crate::config::SortBy::Size => "s",
        crate::config::SortBy::Modified => "t",
        crate::config::SortBy::Extension => "e",
    };
    flags.push(sort_char);

    spans.push(Span::styled(
        format!("[{}]", flags.join("")),
        Style::default().fg(Color::DarkGray),
    ));

    let paragraph = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::NONE))
        .style(Style::default().bg(Color::Black));

    frame.render_widget(paragraph, area);
}

fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G", "T"];
    let mut size = size as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", size as u64, UNITS[unit])
    } else {
        format!("{:.1}{}", size, UNITS[unit])
    }
}

fn git_status_str(status: &GitStatus) -> &'static str {
    match status {
        GitStatus::Added => "A",
        GitStatus::Modified => "M",
        GitStatus::Deleted => "D",
        GitStatus::Renamed => "R",
        GitStatus::Copied => "C",
        GitStatus::Untracked => "?",
        GitStatus::Ignored => "!",
        GitStatus::Conflicted => "!",
        GitStatus::Unmodified => "",
    }
}