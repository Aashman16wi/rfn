use crate::config::ColorConfig;
use crate::state::{QueueItem, QueueStatus};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    queue: &[QueueItem],
    state: &mut ListState,
    running: bool,
    notice: Option<&str>,
    colors: &ColorConfig,
) {
    let bg = crate::ui::parse_color(&colors.bg);
    let fg = crate::ui::parse_color(&colors.fg);
    let muted = crate::ui::parse_color(&colors.muted);
    let accent = crate::ui::parse_color(&colors.accent);
    let border = crate::ui::parse_color(&colors.border);
    let selected_bg = crate::ui::parse_color(&colors.selected_bg);
    let selected_fg = crate::ui::parse_color(&colors.selected_fg);
    let ok = crate::ui::parse_color(&colors.git_added);
    let bad = crate::ui::parse_color(&colors.git_deleted);

    let pending = queue
        .iter()
        .filter(|item| matches!(item.status, QueueStatus::Pending | QueueStatus::Running))
        .count();
    let failed = queue
        .iter()
        .filter(|item| item.status == QueueStatus::Failed)
        .count();

    let mut title = vec![Span::styled(
        " Queue ",
        Style::default().fg(fg).add_modifier(Modifier::BOLD),
    )];
    if pending > 0 {
        title.push(Span::styled(
            format!(" {pending} pending "),
            Style::default().fg(accent),
        ));
    }
    if failed > 0 {
        title.push(Span::styled(
            format!(" {failed} failed "),
            Style::default().fg(bad),
        ));
    }
    if running {
        title.push(Span::styled(" running… ", Style::default().fg(accent)));
    }

    let block = Block::default()
        .title(Line::from(title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(bg));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(
            " #  STATUS  OP    SOURCE → DESTINATION",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
    ]))
    .style(Style::default().bg(bg));
    frame.render_widget(header, chunks[0]);

    if queue.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(Span::styled(
                "Queue is empty",
                Style::default().fg(fg).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "yank files with y / x, then paste with p",
                Style::default().fg(muted),
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(bg));
        frame.render_widget(empty, chunks[1]);
    } else {
        let items: Vec<ListItem> = queue
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let (status, status_color) = match item.status {
                    QueueStatus::Pending => ("PEND", muted),
                    QueueStatus::Running => ("RUN", accent),
                    QueueStatus::Done => ("DONE", ok),
                    QueueStatus::Failed => ("FAIL", bad),
                };

                let src = item
                    .src
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("?");
                let dst = display_path(item.dst.parent().unwrap_or(&item.dst));

                let mut spans = vec![
                    Span::styled(
                        format!("{:2} ", index + 1),
                        Style::default().fg(muted),
                    ),
                    Span::styled(
                        format!("{status:<4}  "),
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<4}  ", item.op.to_uppercase()),
                        Style::default().fg(accent),
                    ),
                    Span::styled(src.to_string(), Style::default().fg(fg)),
                    Span::styled(" → ", Style::default().fg(muted)),
                    Span::styled(dst, Style::default().fg(fg)),
                ];

                if let Some(error) = &item.error {
                    spans.push(Span::styled(
                        format!("  ! {error}"),
                        Style::default().fg(bad),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .style(Style::default().bg(bg))
            .highlight_style(
                Style::default()
                    .bg(selected_bg)
                    .fg(selected_fg)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, chunks[1], state);
    }

    let footer_text = match notice {
        Some(message) => message.to_string(),
        None => "r: run pending/failed  c: clear finished  d: remove  u/o: reorder  Esc: close"
            .to_string(),
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        footer_text,
        Style::default().fg(muted),
    )))
    .style(Style::default().bg(bg))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[2]);
}

fn display_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}
