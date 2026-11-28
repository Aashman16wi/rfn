pub mod bookmarks;
pub mod file_list;
pub mod format;
pub mod help;
pub mod preview;
pub mod prompt;
pub mod queue;
pub mod status_bar;
pub mod theme_picker;
pub mod top_bar;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Color,
};

pub struct LayoutAreas {
    pub tabs: Option<Rect>,
    pub top: Rect,
    pub parent: Option<Rect>,
    pub current: Rect,
    pub preview: Option<Rect>,
    pub status: Rect,
}

pub fn layout(
    area: Rect,
    show_preview: bool,
    show_parent: bool,
    show_tabs: bool,
) -> LayoutAreas {
    let mut constraints = Vec::new();
    if show_tabs {
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Length(1));
    constraints.push(Constraint::Min(0));
    constraints.push(Constraint::Length(1));

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let (tabs, top, body, status) = if show_tabs {
        (Some(rows[0]), rows[1], rows[2], rows[3])
    } else {
        (None, rows[0], rows[1], rows[2])
    };
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
        tabs,
        top,
        parent,
        current,
        preview,
        status,
    }
}

pub fn parse_color(s: &str) -> Color {
    // A color comes from the config file, so it can be anything the user
    // typed. Slicing it at fixed byte offsets would panic on a name with a
    // multi byte character in it.
    let Some(hex) = s.strip_prefix('#') else {
        return Color::White;
    };
    if hex.len() != 6 || !hex.is_ascii() {
        return Color::White;
    }
    let Some(red) = u8::from_str_radix(&hex[0..2], 16).ok() else {
        return Color::White;
    };
    let Some(green) = u8::from_str_radix(&hex[2..4], 16).ok() else {
        return Color::White;
    };
    let Some(blue) = u8::from_str_radix(&hex[4..6], 16).ok() else {
        return Color::White;
    };
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_valid_color_is_parsed() {
        assert_eq!(parse_color("#7aa2f7"), Color::Rgb(0x7a, 0xa2, 0xf7));
    }

    #[test]
    fn a_color_with_a_multi_byte_character_does_not_panic() {
        // Seven bytes, but the second character takes two of them, so a
        // slice at byte 2 would cut it in half.
        let _ = parse_color("#aébcd");
        let _ = parse_color("#abcdé");
        let _ = parse_color("#é");
    }

    #[test]
    fn a_broken_color_falls_back_instead_of_panicking() {
        assert_eq!(parse_color(""), Color::White);
        assert_eq!(parse_color("7aa2f7"), Color::White);
        assert_eq!(parse_color("#7aa2f"), Color::White);
        assert_eq!(parse_color("#7aa2f77"), Color::White);
        assert_eq!(parse_color("#gggggg"), Color::White);
    }
}
