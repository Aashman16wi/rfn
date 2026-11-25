use crate::config::KeyConfig;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, _keys: &KeyConfig) {
    let block = Block::default()
        .title(" Help - Press ? to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = vec![
        ("Navigation", vec![
            ("j / ↓", "Move down"),
            ("k / ↑", "Move up"),
            ("h / ←", "Parent directory"),
            ("l / → / Enter", "Enter directory / open file"),
            ("H / Ctrl+h", "Go to parent"),
            ("~", "Go to home"),
            ("gg", "Go to top"),
            ("G", "Go to bottom"),
            ("Ctrl+u / Ctrl+d", "Page up / down"),
        ]),
        ("File Operations", vec![
            ("n", "New file"),
            ("N / Shift+n", "New directory"),
            ("r", "Rename"),
            ("d / Delete", "Delete"),
            ("y", "Copy (yank)"),
            ("x", "Cut"),
            ("p", "Paste"),
        ]),
        ("View", vec![
            (".", "Toggle hidden files"),
            ("P", "Toggle preview"),
            ("g", "Toggle git status"),
            ("R / Ctrl+r", "Refresh"),
        ]),
        ("Sorting", vec![
            ("sn", "Sort by name"),
            ("ss", "Sort by size"),
            ("st", "Sort by time"),
            ("se", "Sort by extension"),
            ("sr", "Reverse sort"),
        ]),
        ("Other", vec![
            ("/", "Search"),
            ("?", "Toggle help"),
            ("q / Ctrl+c", "Quit"),
        ]),
    ];

    let mut lines = Vec::new();
    for (title, items) in sections {
        lines.push(Line::from(Span::styled(
            title,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in items {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<18}", key),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(desc, Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));

    frame.render_widget(paragraph, inner);
}