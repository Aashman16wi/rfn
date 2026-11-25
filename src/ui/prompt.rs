use crate::config::ColorConfig;
use crate::types::AppMode;
use crate::ui::parse_color;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub fn render_input(
    frame: &mut Frame,
    area: Rect,
    mode: AppMode,
    buffer: &str,
    colors: &ColorConfig,
) {
    let (label, color) = match mode {
        AppMode::Search => ("Search", Color::Cyan),
        AppMode::Rename => ("Rename", Color::Yellow),
        AppMode::NewFile => ("New file", Color::Green),
        AppMode::NewDir => ("New dir", Color::Green),
        _ => return,
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(buffer.to_string(), Style::default().fg(Color::White)),
    ]);

    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    frame.render_widget(paragraph, area);

    let prefix = label.width() + 3;
    let x = area.x + prefix as u16 + buffer.width() as u16;
    let x = x.min(area.x + area.width.saturating_sub(1));
    frame.set_cursor_position((x, area.y));
}

pub fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    mode: AppMode,
    name: &str,
    colors: &ColorConfig,
) {
    let (title, message) = match mode {
        AppMode::ConfirmDelete => (" Delete ", format!("Delete \"{name}\"?")),
        AppMode::ConfirmQuit => (" Quit ", "Quit rfn?".to_string()),
        _ => return,
    };

    let width = 56.min(area.width.saturating_sub(4)).max(24);
    let height = 5;
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let text = vec![
        Line::from(Span::styled(
            message,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " y ",
                Style::default()
                    .bg(Color::Red)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" confirm    "),
            Span::styled(
                " esc ",
                Style::default().bg(Color::DarkGray).fg(Color::White),
            ),
            Span::raw(" cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner);
}
