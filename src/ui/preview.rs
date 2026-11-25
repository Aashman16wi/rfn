use crate::types::PreviewContent;
use crate::config::ColorConfig;
use crate::ui::parse_color;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    content: &PreviewContent,
    colors: &ColorConfig,
) {
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    let text = match content.preview_type {
        crate::types::PreviewType::Image => {
            vec![Line::from(Span::styled(
                "[Image preview - requires terminal support]",
                Style::default().fg(Color::DarkGray),
            ))]
        }
        crate::types::PreviewType::Binary => {
            vec![Line::from(Span::styled(
                "[Binary file]",
                Style::default().fg(Color::DarkGray),
            ))]
        }
        crate::types::PreviewType::Directory => {
            vec![Line::from(Span::styled(
                "[Directory]",
                Style::default().fg(Color::DarkGray),
            ))]
        }
        _ => content
            .content
            .lines()
            .map(|l| Line::from(Span::raw(l)))
            .collect(),
    };

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    frame.render_widget(paragraph, area);
}