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

    if content.preview_type == PreviewType::Image {
        if let Some(protocol) = protocol {
            frame.render_stateful_widget(
                StatefulImage::new().resize(Resize::Fit(None)),
                inner,
                protocol,
            );
            return;
        }
    }

    let muted = parse_color(&colors.muted);
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
        PreviewType::Text => content
            .content
            .lines()
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(parse_color(&colors.fg)),
                ))
            })
            .collect(),
    };

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(parse_color(&colors.bg)));

    frame.render_widget(paragraph, inner);
}
