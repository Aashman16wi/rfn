use crate::types::FileEntry;
use crate::config::ColorConfig;
use crate::ui::{parse_color, file_style};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    entries: &[FileEntry],
    state: &mut ListState,
    colors: &ColorConfig,
    show_icons: bool,
    show_git: bool,
) {
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let selected = state.selected() == Some(i);
            let style = file_style(colors, &entry.git_status, selected);

            let icon = if show_icons {
                crate::icons::get_icon(&entry.path, entry.file_type.clone(), true)
            } else {
                ""
            };

            let git_icon = if show_git && entry.git_status != crate::types::GitStatus::Unmodified {
                crate::icons::git_status_icon(&entry.git_status)
            } else {
                ""
            };

            let name = if entry.is_dir() {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };

            let mut spans = Vec::new();

            if !icon.is_empty() {
                spans.push(Span::styled(format!("{} ", icon), style));
            }

            spans.push(Span::styled(name, style));

            if !git_icon.is_empty() {
                spans.push(Span::styled(format!(" {}", git_icon), style));
            }

            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(parse_color(&colors.selected_bg))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("► ");

    frame.render_stateful_widget(list, area, state);
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

pub fn page_up(state: &mut ListState, page_size: usize) {
    let i = match state.selected() {
        Some(i) => i.saturating_sub(page_size),
        None => 0,
    };
    state.select(Some(i));
}

pub fn go_to_top(state: &mut ListState) {
    state.select(Some(0));
}

pub fn go_to_bottom(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        state.select(Some(len - 1));
    }
}