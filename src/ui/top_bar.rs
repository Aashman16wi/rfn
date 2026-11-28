use crate::config::ColorConfig;
use crate::ui::parse_color;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

pub fn render_tabs(
    frame: &mut Frame,
    area: Rect,
    tabs: &[(String, bool)],
    colors: &ColorConfig,
) {
    let bg = parse_color(&colors.topbar_bg);
    let muted = parse_color(&colors.muted);
    let accent = parse_color(&colors.accent);

    frame.render_widget(Block::default().style(Style::default().bg(bg)), area);

    let mut spans = Vec::new();
    let mut used = 0usize;

    for (index, (title, active)) in tabs.iter().enumerate() {
        let label = format!(" {} {} ", index + 1, title);
        let width = label.width();
        if used + width > area.width as usize {
            break;
        }
        used += width + 1;

        spans.push(Span::styled(
            label,
            if *active {
                Style::default()
                    .bg(accent)
                    .fg(parse_color(&colors.bg))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .bg(parse_color(&colors.border))
                    .fg(muted)
            },
        ));
        spans.push(Span::raw(" "));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    current_path: &Path,
    item_count: usize,
    colors: &ColorConfig,
    title: Option<&str>,
) {
    let bg = parse_color(&colors.topbar_bg);
    let muted = parse_color(&colors.muted);
    let fg = parse_color(&colors.fg);
    let accent = parse_color(&colors.accent);

    frame.render_widget(Block::default().style(Style::default().bg(bg)), area);

    let right = format!(" {item_count} items ");
    let right_width = right.width();

    let mut spans = vec![
        Span::styled(
            " rfn ",
            Style::default()
                .bg(accent)
                .fg(parse_color(&colors.bg))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    if let Some(title) = title {
        spans.push(Span::styled(
            title.to_string(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    } else {
        let path_text = display_path(current_path);
        let mut segments: Vec<String> = path_text
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect();

        if path_text.starts_with('/') {
            segments.insert(0, "/".to_string());
        }

        if segments.len() > 4 {
            let keep = segments.split_off(segments.len() - 3);
            segments = vec!["…".to_string()];
            segments.extend(keep);
        }

        let available = area.width.saturating_sub(right_width as u16 + 2) as usize;
        let mut used = 6; // " rfn " + space

        for (index, segment) in segments.iter().enumerate() {
            let is_last = index == segments.len() - 1;

            // The separator goes in front of every segment but the first.
            // Its width has to be counted, otherwise the path grows longer
            // than the room that was measured for it and it runs into the
            // item count on the right.
            let separator = if index > 0 { 3 } else { 0 };
            let piece_width = separator + segment.width();

            if used + piece_width > available {
                break;
            }
            used += piece_width;

            if index > 0 {
                spans.push(Span::styled(" › ", Style::default().fg(muted)));
            }

            spans.push(Span::styled(
                segment.clone(),
                if is_last {
                    Style::default().fg(accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg)
                },
            ));
        }
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
        area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(right, Style::default().fg(muted))))
            .alignment(Alignment::Right)
            .style(Style::default().bg(bg)),
        area,
    );
}

fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if path == home {
            return "~".to_string();
        }
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}
