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

    // The label takes the first columns, the text gets the rest.
    let prefix = label.width() + 3;
    let available = area.width.saturating_sub(prefix as u16).max(1) as usize;
    // Long text scrolls to the left, so the caret stays on the last
    // character that is visible.
    let visible = tail_by_width(buffer, available);

    let line = Line::from(vec![
        Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(visible.to_string(), Style::default().fg(Color::White)),
    ]);

    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(parse_color(&colors.preview_bg)));

    frame.render_widget(paragraph, area);

    let x = area
        .x
        .saturating_add(prefix as u16)
        .saturating_add(visible.width() as u16)
        .min(area.x + area.width.saturating_sub(1));
    frame.set_cursor_position((x, area.y));
}

/// The end of the text that fits into `columns` columns.
fn tail_by_width(text: &str, columns: usize) -> &str {
    use unicode_width::UnicodeWidthChar;

    let mut width = 0;
    let mut cut = text.len();
    for (index, character) in text.char_indices().rev() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > columns {
            break;
        }
        width += character_width;
        cut = index;
    }
    &text[cut..]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_stays_as_it_is() {
        assert_eq!(tail_by_width("hello", 10), "hello");
    }

    #[test]
    fn long_text_keeps_its_end() {
        assert_eq!(tail_by_width("hello world", 5), "world");
    }

    #[test]
    fn a_multi_byte_character_is_never_cut_in_half() {
        // Every letter is one column wide, so two of them fit.
        let text = "äöü";
        let tail = tail_by_width(text, 2);
        assert_eq!(tail, "öü");
    }

    #[test]
    fn the_width_of_the_tail_is_never_bigger_than_the_room() {
        for length in 0..40 {
            let text = "ä".repeat(length);
            let tail = tail_by_width(&text, 7);
            assert!(tail.width() <= 7, "{tail:?} is wider than 7");
        }
    }
}

pub fn render_confirm(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    colors: &ColorConfig,
) {

    // The popup has to stay inside the terminal. A popup that reaches past
    // the edge makes ratatui panic while it clears the area.
    let width = 56.min(area.width.saturating_sub(4)).max(24).min(area.width);
    let height = 5.min(area.height);
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
            message.to_string(),
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
