use crate::config::{ColorConfig, SortBy};
use crate::types::{AppMode, ClipboardOp, FileEntry, GitStatus};
use crate::ui::format::{format_age, format_permissions, format_size};
use crate::ui::parse_color;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    mode: AppMode,
    selected_entry: Option<&FileEntry>,
    clipboard: ClipboardOp,
    clipboard_count: usize,
    marked_count: usize,
    show_hidden: bool,
    show_preview: bool,
    show_git: bool,
    sort_by: SortBy,
    reverse_sort: bool,
    search_query: &str,
    colors: &ColorConfig,
) {
    let bg = parse_color(&colors.status_bg);
    let fg = parse_color(&colors.fg);
    let muted = parse_color(&colors.muted);
    let accent = parse_color(&colors.accent);

    frame.render_widget(Block::default().style(Style::default().bg(bg)), area);

    let (mode_label, mode_color) = mode_style(mode, accent);
    let mut left = vec![
        Span::styled(
            format!(" {mode_label} "),
            Style::default()
                .bg(mode_color)
                .fg(parse_color(&colors.bg))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    if !search_query.is_empty() {
        left.push(Span::styled(
            format!(" /{search_query} "),
            Style::default()
                .bg(parse_color(&colors.border))
                .fg(accent)
                .add_modifier(Modifier::BOLD),
        ));
        left.push(Span::raw(" "));
    }

    if let Some(entry) = selected_entry {
        let icon = crate::icons::get_icon(&entry.path, entry.file_type.clone(), true);
        left.push(Span::styled(
            format!("{icon} "),
            Style::default().fg(crate::icons::icon_color(entry, colors)),
        ));
        left.push(Span::styled(
            entry.name.clone(),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        ));
        left.push(Span::raw("  "));
        left.push(Span::styled(format_size(entry.size), Style::default().fg(muted)));
        left.push(Span::raw("  "));
        left.push(Span::styled(
            format_age(entry.modified),
            Style::default().fg(muted),
        ));
        left.push(Span::raw("  "));
        left.push(Span::styled(
            format_permissions(entry.permissions),
            Style::default().fg(muted),
        ));

        if entry.git_status != GitStatus::Unmodified {
            left.push(Span::raw("  "));
            let (r, g, b) = crate::icons::git_status_color(&entry.git_status);
            left.push(Span::styled(
                format!(" {} ", git_status_str(&entry.git_status)),
                Style::default().bg(Color::Rgb(r, g, b)).fg(Color::Black),
            ));
        }
    }

    if marked_count > 0 {
        left.push(Span::raw(" "));
        left.push(Span::styled(
            format!(" {marked_count} marked "),
            Style::default()
                .bg(parse_color(&colors.git_modified))
                .fg(parse_color(&colors.bg))
                .add_modifier(Modifier::BOLD),
        ));
    }

    if clipboard != ClipboardOp::None && clipboard_count > 0 {
        let label = match clipboard {
            ClipboardOp::Copy => "COPY",
            ClipboardOp::Cut => "CUT",
            ClipboardOp::None => "",
        };
        left.push(Span::raw(" "));
        left.push(Span::styled(
            format!(" {label} {clipboard_count} "),
            Style::default()
                .bg(parse_color(&colors.git_added))
                .fg(parse_color(&colors.bg))
                .add_modifier(Modifier::BOLD),
        ));
    }

    let mut flags = Vec::new();
    if show_hidden {
        flags.push("hidden");
    }
    if show_preview {
        flags.push("preview");
    }
    if show_git {
        flags.push("git");
    }
    let sort = match sort_by {
        SortBy::Name => "name",
        SortBy::Size => "size",
        SortBy::Modified => "time",
        SortBy::Extension => "ext",
    };
    let arrow = if reverse_sort { "↑" } else { "↓" };

    let mut right: Vec<Span> = Vec::new();
    for flag in &flags {
        right.push(Span::styled(
            format!(" {flag} "),
            Style::default()
                .bg(parse_color(&colors.border))
                .fg(muted),
        ));
        right.push(Span::raw(" "));
    }
    right.push(Span::styled(
        format!(" {sort}{arrow} "),
        Style::default()
            .bg(parse_color(&colors.border))
            .fg(accent),
    ));
    right.push(Span::raw(" "));
    right.push(Span::styled(
        " ? ",
        Style::default()
            .bg(accent)
            .fg(parse_color(&colors.bg))
            .add_modifier(Modifier::BOLD),
    ));
    right.push(Span::styled(" help ", Style::default().fg(muted)));

    let left_width: usize = left.iter().map(|span| span.content.width()).sum();
    let right_width: usize = right.iter().map(|span| span.content.width()).sum();
    let total = area.width as usize;

    if left_width + right_width < total {
        let gap = total - left_width - right_width;
        left.push(Span::raw(" ".repeat(gap)));
        left.extend(right);
    }

    let paragraph = Paragraph::new(Line::from(left)).style(Style::default().bg(bg));
    frame.render_widget(paragraph, area);
}

fn mode_style(mode: AppMode, accent: Color) -> (&'static str, Color) {
    match mode {
        AppMode::Normal => ("NORMAL", accent),
        AppMode::Search => ("SEARCH", Color::Rgb(125, 207, 255)),
        AppMode::Rename => ("RENAME", Color::Rgb(224, 175, 104)),
        AppMode::NewFile => ("NEW FILE", Color::Rgb(158, 206, 106)),
        AppMode::NewDir => ("NEW DIR", Color::Rgb(158, 206, 106)),
        AppMode::ConfirmDelete => ("DELETE", Color::Rgb(247, 118, 142)),
        AppMode::ConfirmQuit => ("QUIT", Color::Rgb(247, 118, 142)),
        AppMode::ConfirmOverwrite => ("OVERWRITE", Color::Rgb(224, 175, 104)),
        AppMode::Visual => ("VISUAL", Color::Rgb(224, 175, 104)),
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
