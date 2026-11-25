use crate::config::{ColorConfig, KeyConfig};
use crate::ui::parse_color;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn render(frame: &mut Frame, area: Rect, _keys: &KeyConfig, colors: &ColorConfig) {
    let bg = parse_color(&colors.bg);
    let fg = parse_color(&colors.fg);
    let muted = parse_color(&colors.muted);
    let accent = parse_color(&colors.accent);

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                " rfn ",
                Style::default()
                    .bg(accent)
                    .fg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" help ", Style::default().fg(muted)),
        ]))
        .title_alignment(ratatui::layout::Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(parse_color(&colors.border)))
        .style(Style::default().bg(bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let left_sections = vec![
        (
            "Navigation",
            vec![
                ("j / ↓", "Move down"),
                ("k / ↑", "Move up"),
                ("h / ←", "Parent directory"),
                ("l / → / Enter", "Enter directory / open file"),
                ("H / Ctrl+h", "Go to parent"),
                ("~", "Go to home"),
                ("G / End", "Go to bottom"),
                ("Home", "Go to top"),
                ("Ctrl+d / Ctrl+u", "Page down / up"),
            ],
        ),
        (
            "File operations",
            vec![
                ("n", "New file"),
                ("N", "New directory"),
                ("r", "Rename"),
                ("d / Delete", "Delete"),
                ("y", "Copy (yank)"),
                ("x", "Cut"),
                ("p", "Paste"),
            ],
        ),
    ];

    let right_sections = vec![
        (
            "View",
            vec![
                (".", "Toggle hidden files"),
                ("P", "Toggle preview"),
                ("g", "Toggle git status"),
                ("R / Ctrl+r", "Refresh"),
            ],
        ),
        (
            "Sorting",
            vec![("s", "Cycle name → size → time → ext"), ("S", "Reverse order")],
        ),
        (
            "Other",
            vec![
                ("/", "Search"),
                ("Esc", "Clear search"),
                ("?", "Toggle help"),
                ("q / Ctrl+c", "Quit"),
            ],
        ),
    ];

    let mut left_lines = Vec::new();
    for (title, items) in left_sections {
        push_section(&mut left_lines, title, items, accent, fg, muted);
    }

    let mut right_lines = Vec::new();
    for (title, items) in right_sections {
        push_section(&mut right_lines, title, items, accent, fg, muted);
    }

    frame.render_widget(
        Paragraph::new(left_lines).style(Style::default().bg(bg)),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(right_lines).style(Style::default().bg(bg)),
        columns[1],
    );

    // Footer hint
    if inner.height > 2 {
        let hint_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " press ? or Esc to close ",
                Style::default().fg(muted),
            )))
            .style(Style::default().bg(bg)),
            hint_area,
        );
    }
}

fn push_section<'a>(
    lines: &mut Vec<Line<'a>>,
    title: &'a str,
    items: Vec<(&'a str, &'a str)>,
    accent: ratatui::style::Color,
    fg: ratatui::style::Color,
    muted: ratatui::style::Color,
) {
    lines.push(Line::from(Span::styled(
        format!("  {title}"),
        Style::default().fg(accent).add_modifier(Modifier::BOLD),
    )));
    for (key, desc) in items {
        lines.push(Line::from(vec![
            Span::styled(
                format!("   {:<16}", key),
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(muted)),
        ]));
    }
    lines.push(Line::from(""));
}
