use crate::config::ColorConfig;
use crate::types::{PreviewContent, PreviewType};
use crate::ui::parse_color;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
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
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

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

    let text = match content.preview_type {
        PreviewType::Image => vec![Line::from(Span::styled(
            "[Image preview unavailable]",
            Style::default().fg(Color::DarkGray),
        ))],
        PreviewType::Binary => vec![Line::from(Span::styled(
            "[Binary file]",
            Style::default().fg(Color::DarkGray),
        ))],
        PreviewType::Directory => vec![Line::from(Span::styled(
            "[Directory]",
            Style::default().fg(Color::DarkGray),
        ))],
        PreviewType::None => vec![Line::from(Span::styled(
            "[No preview]",
            Style::default().fg(Color::DarkGray),
        ))],
        PreviewType::Text => content
            .content
            .lines()
            .map(|line| Line::from(Span::raw(line.to_string())))
            .collect(),
    };

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    frame.render_widget(paragraph, inner);
}
