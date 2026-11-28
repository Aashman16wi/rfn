use crate::config::ColorConfig;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::path::PathBuf;

/// Overlay listing the saved bookmarks.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    bookmarks: &[PathBuf],
    state: &mut ListState,
    colors: &ColorConfig,
) {
    let bg = crate::ui::parse_color(&colors.bg);
    let fg = crate::ui::parse_color(&colors.fg);
    let muted = crate::ui::parse_color(&colors.muted);
    let accent = crate::ui::parse_color(&colors.accent);

    let width = area.width.saturating_sub(8).max(34);
    let height = (bookmarks.len() as u16 + 4).min(area.height).max(5);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Bookmarks ",
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(bg));

    let inner = block.inner(popup);
    frame.render_widget(ratatui::widgets::Clear, popup);
    frame.render_widget(block, popup);
    if inner.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    if bookmarks.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(Span::styled(
                "No bookmarks yet",
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "press a to bookmark this directory",
                Style::default().fg(muted),
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(bg));
        frame.render_widget(empty, chunks[0]);
    } else {
        let items: Vec<ListItem> = bookmarks
            .iter()
            .map(|path| {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("/")
                    .to_string();
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {name:<20}"),
                        Style::default().fg(fg).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" {}", path.display()),
                        Style::default().fg(muted),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .style(Style::default().bg(bg))
            .highlight_style(
                Style::default()
                    .bg(crate::ui::parse_color(&colors.selected_bg))
                    .fg(crate::ui::parse_color(&colors.selected_fg))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[0], state);
    }

    let footer = Paragraph::new(Line::from(Span::styled(
        " j/k move   Enter jump   a add   d remove   Esc close ",
        Style::default().fg(muted),
    )))
    .style(Style::default().bg(bg))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
