use crate::config::ColorConfig;
use crate::types::{PreviewContent, PreviewType};
use crate::ui::parse_color;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{protocol::StatefulProtocol, Resize, StatefulImage};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    content: &PreviewContent,
    protocol: Option<&mut StatefulProtocol>,
    colors: &ColorConfig,
    scroll: usize,
    zoom: f32,
) {
    let block = Block::default()
        .title(Line::from(Span::styled(
            " Preview ",
            Style::default()
                .fg(parse_color(&colors.fg))
                .add_modifier(Modifier::BOLD),
        )))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(parse_color(&colors.border)))
        .style(Style::default().bg(parse_color(&colors.bg)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let muted = parse_color(&colors.muted);

    if content.preview_type == PreviewType::Image {
        if let Some(protocol) = protocol {
            // Zoomed in: crop the picture so the detail is bigger than the
            // pane. Zoomed out: fit the whole picture.
            let resize = if zoom >= 1.05 {
                Resize::Crop(None)
            } else {
                Resize::Fit(None)
            };
            frame.render_stateful_widget(StatefulImage::new().resize(resize), inner, protocol);
            return;
        }
    }

    // Only text previews scroll
    let text = match content.preview_type {
        PreviewType::Image => vec![Line::from(Span::styled(
            "[Image preview unavailable]",
            Style::default().fg(muted),
        ))],
        PreviewType::Binary => vec![Line::from(Span::styled(
            "[Binary file]",
            Style::default().fg(muted),
        ))],
        PreviewType::None => vec![Line::from(Span::styled(
            "[No preview]",
            Style::default().fg(muted),
        ))],
        PreviewType::Text => {
            let lines: Vec<&str> = content.content.lines().collect();
            // Scrolling past the end of the file must not empty the pane,
            // and a shorter file must not stay scrolled away.
            let scroll = scroll.min(lines.len().saturating_sub(1));
            lines
                .into_iter()
                .skip(scroll)
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(parse_color(&colors.fg)),
                    ))
                })
                .collect()
        }
    };

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(parse_color(&colors.bg)));

    frame.render_widget(paragraph, inner);
}