use crate::config::ColorConfig;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

/// Small overlay that lets the user switch theme without restarting.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    selected: usize,
    active_theme: &str,
    colors: &ColorConfig,
) {
    let bg = crate::ui::parse_color(&colors.bg);
    let fg = crate::ui::parse_color(&colors.fg);
    let muted = crate::ui::parse_color(&colors.muted);
    let accent = crate::ui::parse_color(&colors.accent);

    // The popup has to stay inside the terminal. A popup that reaches past
    // the edge makes ratatui panic while it clears the area.
    let width = area.width.saturating_sub(4).clamp(20, 60).min(area.width);
    let height = (ColorConfig::THEMES.len() as u16 + 4).clamp(5, 20).min(area.height);
    let popup = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    let block = Block::default()
        .title(Line::from(Span::styled(
            " Theme ",
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

    let items: Vec<ListItem> = ColorConfig::THEMES
        .iter()
        .map(|name| {
            let preset = ColorConfig::preset(name);
            let active = *name == active_theme;

            let mut spans = vec![Span::styled(
                format!(" {name:<18}"),
                Style::default().fg(fg),
            )];

            if let Some(preset) = preset {
                // A tiny swatch row: background, foreground, accent, dir.
                spans.push(Span::styled(
                    "███",
                    Style::default().bg(crate::ui::parse_color(&preset.bg)),
                ));
                spans.push(Span::styled(
                    "███",
                    Style::default().fg(crate::ui::parse_color(&preset.fg)),
                ));
                spans.push(Span::styled(
                    "███",
                    Style::default().fg(crate::ui::parse_color(&preset.accent)),
                ));
                spans.push(Span::styled(
                    "███",
                    Style::default().fg(crate::ui::parse_color(&preset.dir)),
                ));
            }

            if active {
                spans.push(Span::styled(
                    "  (active)",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ));
            }

            ListItem::new(Line::from(spans))
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

    let mut state = ListState::default();
    state.select(Some(selected.min(ColorConfig::THEMES.len().saturating_sub(1))));
    frame.render_stateful_widget(list, chunks[0], &mut state);

    let footer = Paragraph::new(Line::from(Span::styled(
        " j/k select   Enter apply   Esc cancel ",
        Style::default().fg(muted),
    )))
    .style(Style::default().bg(bg))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[1]);
}
