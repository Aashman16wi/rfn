use crate::config::{ColorConfig, KeyConfig};
use crate::ui::parse_color;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

/// One line of the help: the keys on the left, what they do on the right.
struct Entry {
    keys: String,
    description: &'static str,
}

fn section(title: &'static str, entries: Vec<Entry>) -> (String, Vec<Entry>) {
    (title.to_string(), entries)
}

/// The keys of one action, as the user configured them.
fn keys(keys: &[String]) -> String {
    if keys.is_empty() {
        return "not bound".to_string();
    }
    keys.join(" / ")
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    key_config: &KeyConfig,
    colors: &ColorConfig,
    scroll: usize,
    mut on_scroll: impl FnMut(usize, usize),
) {
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
    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // The list of what the keys do is built from the keys of the user, so
    // the help never tells them something that is not true.
    let sections = vec![
        section(
            "Navigation",
            vec![
                Entry { keys: keys(&key_config.up), description: "Move up" },
                Entry { keys: keys(&key_config.down), description: "Move down" },
                Entry { keys: keys(&key_config.left), description: "Parent directory" },
                Entry { keys: keys(&key_config.right), description: "Enter directory / open file" },
                Entry { keys: keys(&key_config.enter), description: "Enter directory / open file" },
                Entry { keys: keys(&key_config.goto_parent), description: "Go to parent" },
                Entry { keys: keys(&key_config.goto_home), description: "Go to home" },
                Entry { keys: "Ctrl+u / Ctrl+d".into(), description: "Page up / down" },
            ],
        ),
        section(
            "File operations",
            vec![
                Entry { keys: keys(&key_config.new_file), description: "New file" },
                Entry { keys: keys(&key_config.new_dir), description: "New directory" },
                Entry { keys: keys(&key_config.rename), description: "Rename" },
                Entry { keys: keys(&key_config.bulk_rename), description: "Bulk rename in $EDITOR" },
                Entry { keys: keys(&key_config.delete), description: "Delete (to trash)" },
                Entry { keys: keys(&key_config.copy), description: "Copy (yank)" },
                Entry { keys: keys(&key_config.cut), description: "Cut" },
                Entry { keys: keys(&key_config.paste), description: "Paste" },
            ],
        ),
        section(
            "Selection",
            vec![
                Entry { keys: keys(&key_config.visual), description: "Visual select" },
                Entry { keys: keys(&key_config.mark), description: "Mark / unmark file" },
                Entry { keys: "Esc".into(), description: "Clear selection" },
            ],
        ),
        section(
            "Tabs",
            vec![
                Entry { keys: keys(&key_config.tab_new), description: "New tab" },
                Entry { keys: keys(&key_config.tab_close), description: "Close tab" },
                Entry { keys: keys(&key_config.tab_next), description: "Next tab" },
                Entry { keys: keys(&key_config.tab_prev), description: "Previous tab" },
                Entry { keys: "1 - 9".into(), description: "Jump to tab" },
            ],
        ),
        section(
            "Preview",
            vec![
                Entry { keys: keys(&key_config.preview_scroll_down), description: "Scroll preview down" },
                Entry { keys: keys(&key_config.preview_scroll_up), description: "Scroll preview up" },
                Entry { keys: keys(&key_config.preview_zoom_in), description: "Zoom in" },
                Entry { keys: keys(&key_config.preview_zoom_out), description: "Zoom out" },
                Entry { keys: keys(&key_config.preview_reset), description: "Reset preview" },
            ],
        ),
        section(
            "View",
            vec![
                Entry { keys: keys(&key_config.toggle_hidden), description: "Toggle hidden files" },
                Entry { keys: keys(&key_config.toggle_preview), description: "Toggle preview" },
                Entry { keys: keys(&key_config.toggle_git), description: "Toggle git status" },
                Entry { keys: keys(&key_config.refresh), description: "Refresh" },
                Entry { keys: keys(&key_config.sort), description: "Cycle sort field" },
                Entry { keys: keys(&key_config.reverse_sort), description: "Reverse order" },
            ],
        ),
        section(
            "Tools",
            vec![
                Entry { keys: keys(&key_config.queue), description: "Transfer queue" },
                Entry { keys: keys(&key_config.trash), description: "Trash browser" },
                Entry { keys: keys(&key_config.open_with), description: "Open with a command" },
                Entry { keys: keys(&key_config.bookmark_add), description: "Bookmark this directory" },
                Entry { keys: keys(&key_config.bookmark_jump), description: "Open the bookmark list" },
                Entry { keys: keys(&key_config.theme), description: "Change the theme" },
                Entry { keys: keys(&key_config.search), description: "Search" },
                Entry { keys: keys(&key_config.help), description: "Toggle this help" },
                Entry { keys: keys(&key_config.quit), description: "Quit" },
            ],
        ),
    ];

    // Every line of the help, so the two columns can be cut from the same
    // The width of the key column follows the longest key, plus a gap, so
    // nothing is pushed out of the window.
    let key_width = sections
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .map(|entry| entry.keys.width())
        .max()
        .unwrap_or(10)
        .saturating_add(2)
        .clamp(8, 20);

    // Every line of the help, so the columns can be cut from the same list
    // and scroll together.
    let mut lines: Vec<Line> = Vec::new();
    for (title, entries) in &sections {
        lines.push(Line::from(Span::styled(
            format!("  {title}"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )));
        for entry in entries {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("   {:<width$}", entry.keys, width = key_width),
                    Style::default().fg(fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.description, Style::default().fg(muted)),
            ]));
        }
        lines.push(Line::from(""));
    }

    // A narrow window gets one column, so the text is not cut off. A wide
    // one gets two columns, so everything fits without scrolling.
    let wide = area.width >= 100;
    let rows = inner.height.saturating_sub(1) as usize; // one row for the footer
    let total = lines.len();
    // The last page: every column that is filled.
    let per_page = if wide { 2 * rows } else { rows };
    let max_scroll = total.saturating_sub(per_page);
    let scroll = scroll.min(max_scroll);
    on_scroll(scroll, max_scroll);

    if wide {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        let first: Vec<Line> = lines.iter().skip(scroll).take(rows).cloned().collect();
        let second: Vec<Line> = lines
            .iter()
            .skip(scroll + rows)
            .take(rows)
            .cloned()
            .collect();

        frame.render_widget(Paragraph::new(first).style(Style::default().bg(bg)), columns[0]);
        frame.render_widget(Paragraph::new(second).style(Style::default().bg(bg)), columns[1]);
    } else {
        let shown: Vec<Line> = lines.iter().skip(scroll).take(rows).cloned().collect();
        frame.render_widget(Paragraph::new(shown).style(Style::default().bg(bg)), inner);
    }

    // Footer hint
    if inner.height > 1 {
        let hint_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
        let more = if scroll + per_page < total {
            " (more below) "
        } else {
            ""
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" j/k scroll   ? or Esc to close {more}"),
                Style::default().fg(muted),
            )))
            .style(Style::default().bg(bg)),
            hint_area,
        );
    }
}
