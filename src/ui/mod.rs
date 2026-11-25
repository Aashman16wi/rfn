pub mod file_list;
pub mod preview;
pub mod status_bar;
pub mod help;

use crate::config::ColorConfig;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
};

pub fn layout(area: Rect, show_preview: bool, show_help: bool) -> (Rect, Rect, Rect) {
    if show_help {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(area);
        return (chunks[0], Rect::default(), Rect::default());
    }

    if show_preview {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(chunks[0]);

        let right_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(chunks[1]);

        (left_chunks[0], right_chunks[0], left_chunks[1])
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        (chunks[0], Rect::default(), chunks[1])
    }
}

pub fn parse_color(s: &str) -> Color {
    if s.starts_with('#') && s.len() == 7 {
        let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(255);
        let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(255);
        let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(255);
        Color::Rgb(r, g, b)
    } else {
        Color::White
    }
}

pub fn file_style(colors: &ColorConfig, git_status: &crate::types::GitStatus, selected: bool) -> Style {
    let mut style = Style::default();

    if selected {
        style = style.bg(parse_color(&colors.selected_bg));
    }

    let (r, g, b) = crate::icons::git_status_color(git_status);
    let git_color = Color::Rgb(r, g, b);

    if *git_status != crate::types::GitStatus::Unmodified {
        style = style.fg(git_color);
    } else {
        // Default file colors would go here based on file type
    }

    style
}