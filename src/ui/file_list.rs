use crate::config::ColorConfig;
use crate::types::{FileEntry, GitStatus};
use crate::ui::format::format_size;
use crate::ui::parse_color;
use std::collections::HashSet;
use std::path::PathBuf;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    entries: &[FileEntry],
    state: &mut ListState,
    colors: &ColorConfig,
    show_icons: bool,
    show_git: bool,
    marked: &HashSet<PathBuf>,
    title: &str,
    loading: bool,
) -> Rect {
    let block = pane_block(title, colors);
    let inner = block.inner(area);

    if entries.is_empty() && loading {
        let spinner = Paragraph::new(Line::from(Span::styled(
            "  loading…",
            Style::default().fg(parse_color(&colors.muted)),
        )))
        .style(Style::default().bg(parse_color(&colors.bg)));
        frame.render_widget(spinner, inner);
        let list = List::new(Vec::<ListItem>::new())
            .block(block)
            .style(Style::default().bg(parse_color(&colors.bg)));
        frame.render_stateful_widget(list, area, state);
        return inner;
    }

    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let selected = state.selected() == Some(index);
            let is_marked = marked.contains(&entry.path);
            row(
                entry,
                inner.width,
                colors,
                show_icons,
                show_git,
                selected,
                is_marked,
            )
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(parse_color(&colors.selected_bg)));

    frame.render_stateful_widget(list, area, state);
    inner
}

pub fn render_parent(
    frame: &mut Frame,
    area: Rect,
    entries: &[FileEntry],
    current_name: &str,
    colors: &ColorConfig,
    show_icons: bool,
    title: &str,
) {
    let block = pane_block(title, colors);
    let inner = block.inner(area);

    let items: Vec<ListItem> = entries
        .iter()
        .map(|entry| {
            let is_current = entry.is_dir() && entry.name == current_name;
            let mut spans = Vec::new();
            spans.push(Span::raw("  "));

            if show_icons {
                let icon = crate::icons::get_icon(&entry.path, entry.file_type.clone(), true);
                spans.push(Span::styled(
                    format!("{icon} "),
                    Style::default().fg(if is_current {
                        parse_color(&colors.accent)
                    } else {
                        parse_color(&colors.muted)
                    }),
                ));
            }

            let name = if entry.is_dir() {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };

            spans.push(Span::styled(
                name,
                if is_current {
                    Style::default()
                        .fg(parse_color(&colors.accent))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(parse_color(&colors.muted))
                },
            ));

            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut parent_state = ListState::default();
    if let Some(index) = entries
        .iter()
        .position(|entry| entry.is_dir() && entry.name == current_name)
    {
        parent_state.select(Some(index));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(parse_color(&colors.selected_bg)));

    frame.render_stateful_widget(list, area, &mut parent_state);

    let _ = inner;
}

fn row(
    entry: &FileEntry,
    width: u16,
    colors: &ColorConfig,
    show_icons: bool,
    show_git: bool,
    selected: bool,
    marked: bool,
) -> ListItem<'static> {
    let accent = parse_color(&colors.accent);
    let muted = parse_color(&colors.muted);

    let mut spans = Vec::new();
    let gutter = if selected {
        "▍ "
    } else if marked {
        "▪ "
    } else {
        "  "
    };
    spans.push(Span::styled(
        gutter,
        Style::default().fg(if marked {
            parse_color(&colors.git_modified)
        } else {
            accent
        }),
    ));

    let icon = if show_icons {
        crate::icons::get_icon(&entry.path, entry.file_type.clone(), true)
    } else {
        ""
    };
    let icon_width = icon.width();

    if !icon.is_empty() {
        spans.push(Span::styled(
            format!("{icon} "),
            Style::default().fg(crate::icons::icon_color(entry, colors)),
        ));
    }

    let name = if entry.is_dir() {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };
    let name_width = name.width();

    let name_color = if selected {
        parse_color(&colors.selected_fg)
    } else if marked {
        parse_color(&colors.git_modified)
    } else {
        name_color(entry, colors)
    };

    let mut modifiers = if entry.is_dir() {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };
    if marked {
        modifiers |= Modifier::BOLD;
    }

    spans.push(Span::styled(
        name,
        Style::default().fg(name_color).add_modifier(modifiers),
    ));

    let git_icon = if show_git && entry.git_status != GitStatus::Unmodified {
        crate::icons::git_status_icon(&entry.git_status)
    } else {
        ""
    };
    let git_width = git_icon.width();

    let size = if entry.is_dir() {
        String::new()
    } else {
        format_size(entry.size)
    };
    let size_width = size.width();

    let right_width = if git_icon.is_empty() {
        size_width + usize::from(!size.is_empty())
    } else {
        git_width + 1 + size_width + usize::from(!size.is_empty())
    };

    let used = 2 + icon_width + usize::from(!icon.is_empty()) + name_width;
    let padding = (width as usize).saturating_sub(used + right_width + 1);
    if padding > 0 {
        spans.push(Span::raw(" ".repeat(padding)));
    }

    if !git_icon.is_empty() {
        let (r, g, b) = crate::icons::git_status_color(&entry.git_status);
        spans.push(Span::styled(
            git_icon,
            Style::default().fg(Color::Rgb(r, g, b)),
        ));
        if !size.is_empty() {
            spans.push(Span::raw(" "));
        }
    }

    if !size.is_empty() {
        spans.push(Span::styled(size, Style::default().fg(muted)));
    }

    spans.push(Span::raw(" "));

    ListItem::new(Line::from(spans))
}

fn name_color(entry: &FileEntry, colors: &ColorConfig) -> Color {
    if entry.is_dir() {
        parse_color(&colors.dir)
    } else if entry.is_symlink() {
        parse_color(&colors.symlink)
    } else if entry.permissions & 0o111 != 0 {
        parse_color(&colors.exec)
    } else if entry.is_hidden {
        parse_color(&colors.muted)
    } else {
        parse_color(&colors.file)
    }
}

fn pane_block(title: &str, colors: &ColorConfig) -> Block<'static> {
    Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(parse_color(&colors.fg))
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(parse_color(&colors.border)))
        .style(Style::default().bg(parse_color(&colors.bg)))
}

pub fn next(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(i) if i + 1 < len => i + 1,
        Some(_) | None => 0,
    };
    state.select(Some(i));
}

pub fn previous(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(0) | None => len - 1,
        Some(i) => i - 1,
    };
    state.select(Some(i));
}

pub fn page_down(state: &mut ListState, len: usize, page_size: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(i) => (i + page_size).min(len - 1),
        None => 0,
    };
    state.select(Some(i));
}

pub fn page_up(state: &mut ListState, len: usize, page_size: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let i = match state.selected() {
        Some(i) => i.saturating_sub(page_size),
        None => 0,
    };
    state.select(Some(i));
}

pub fn go_to_top(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    state.select(Some(0));
}

pub fn go_to_bottom(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        state.select(Some(len - 1));
    }
}
