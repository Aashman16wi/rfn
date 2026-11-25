pub mod file_list;
pub mod format;
pub mod help;
pub mod preview;
pub mod prompt;
pub mod status_bar;
pub mod top_bar;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

pub struct LayoutAreas {
    pub top: Rect,
    pub parent: Option<Rect>,
    pub current: Rect,
    pub preview: Option<Rect>,
    pub status: Rect,
}

pub fn layout(area: Rect, show_preview: bool, show_parent: bool) -> LayoutAreas {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let body = rows[1];
    let show_parent = show_parent && body.width >= 100;
    let show_preview = show_preview && body.width >= 70;
    let gap = 1;

    let mut parent = None;
    let mut preview = None;
    let current;

    if show_parent && show_preview {
        let usable = body.width.saturating_sub(gap * 2) as u32;
        let parent_width = (usable * 22 / 100) as u16;
        let current_width = (usable * 38 / 100) as u16;
        let preview_width = usable as u16 - parent_width - current_width;

        parent = Some(Rect::new(body.x, body.y, parent_width, body.height));
        current = Rect::new(
            body.x + parent_width + gap,
            body.y,
            current_width,
            body.height,
        );
        preview = Some(Rect::new(
            body.x + parent_width + gap + current_width + gap,
            body.y,
            preview_width,
            body.height,
        ));
    } else if show_preview {
        let usable = body.width.saturating_sub(gap);
        let current_width = usable * 55 / 100;
        current = Rect::new(body.x, body.y, current_width, body.height);
        preview = Some(Rect::new(
            body.x + current_width + gap,
            body.y,
            usable - current_width,
            body.height,
        ));
    } else {
        current = body;
    }

    LayoutAreas {
        top: rows[0],
        parent,
        current,
        preview,
        status: rows[2],
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
