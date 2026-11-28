use crate::config::KeyConfig;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Quit,
    Up,
    Down,
    Left,
    Right,
    Enter,
    Backspace,
    Delete,
    Rename,
    NewFile,
    NewDir,
    Copy,
    Paste,
    Cut,
    Search,
    ToggleHidden,
    TogglePreview,
    ToggleGit,
    Help,
    Refresh,
    GotoParent,
    GotoHome,
    SortCycle,
    ReverseSort,
    NewTab,
    CloseTab,
    NextTab,
    PrevTab,
    ToggleVisual,
    ToggleMark,
    BulkRename,
    PreviewScrollUp,
    PreviewScrollDown,
    PreviewZoomIn,
    PreviewZoomOut,
    PreviewReset,
    BookmarkAdd,
    BookmarkJump,
    OpenTrash,
    OpenWith,
    QueueManager,
    ThemePicker,
}

pub struct KeyMapper {
    map: HashMap<(KeyCode, KeyModifiers), Action>,
}

impl KeyMapper {
    pub fn new(config: &KeyConfig) -> Self {
        let mut map = HashMap::new();

        Self::add_keys(&mut map, &config.quit, Action::Quit);
        Self::add_keys(&mut map, &config.up, Action::Up);
        Self::add_keys(&mut map, &config.down, Action::Down);
        Self::add_keys(&mut map, &config.left, Action::Left);
        Self::add_keys(&mut map, &config.right, Action::Right);
        Self::add_keys(&mut map, &config.enter, Action::Enter);
        Self::add_keys(&mut map, &config.backspace, Action::Backspace);
        Self::add_keys(&mut map, &config.delete, Action::Delete);
        Self::add_keys(&mut map, &config.rename, Action::Rename);
        Self::add_keys(&mut map, &config.new_file, Action::NewFile);
        Self::add_keys(&mut map, &config.new_dir, Action::NewDir);
        Self::add_keys(&mut map, &config.copy, Action::Copy);
        Self::add_keys(&mut map, &config.paste, Action::Paste);
        Self::add_keys(&mut map, &config.cut, Action::Cut);
        Self::add_keys(&mut map, &config.search, Action::Search);
        Self::add_keys(&mut map, &config.toggle_hidden, Action::ToggleHidden);
        Self::add_keys(&mut map, &config.toggle_preview, Action::TogglePreview);
        Self::add_keys(&mut map, &config.toggle_git, Action::ToggleGit);
        Self::add_keys(&mut map, &config.help, Action::Help);
        Self::add_keys(&mut map, &config.refresh, Action::Refresh);
        Self::add_keys(&mut map, &config.goto_parent, Action::GotoParent);
        Self::add_keys(&mut map, &config.goto_home, Action::GotoHome);
        Self::add_keys(&mut map, &config.sort, Action::SortCycle);
        Self::add_keys(&mut map, &config.reverse_sort, Action::ReverseSort);
        Self::add_keys(&mut map, &config.tab_new, Action::NewTab);
        Self::add_keys(&mut map, &config.tab_close, Action::CloseTab);
        Self::add_keys(&mut map, &config.tab_next, Action::NextTab);
        Self::add_keys(&mut map, &config.tab_prev, Action::PrevTab);
        Self::add_keys(&mut map, &config.visual, Action::ToggleVisual);
        Self::add_keys(&mut map, &config.mark, Action::ToggleMark);
        Self::add_keys(&mut map, &config.bulk_rename, Action::BulkRename);
        Self::add_keys(&mut map, &config.preview_scroll_up, Action::PreviewScrollUp);
        Self::add_keys(&mut map, &config.preview_scroll_down, Action::PreviewScrollDown);
        Self::add_keys(&mut map, &config.preview_zoom_in, Action::PreviewZoomIn);
        Self::add_keys(&mut map, &config.preview_zoom_out, Action::PreviewZoomOut);
        Self::add_keys(&mut map, &config.preview_reset, Action::PreviewReset);
        Self::add_keys(&mut map, &config.bookmark_add, Action::BookmarkAdd);
        Self::add_keys(&mut map, &config.bookmark_jump, Action::BookmarkJump);
        Self::add_keys(&mut map, &config.trash, Action::OpenTrash);
        Self::add_keys(&mut map, &config.open_with, Action::OpenWith);
        Self::add_keys(&mut map, &config.queue, Action::QueueManager);
        Self::add_keys(&mut map, &config.theme, Action::ThemePicker);

        Self { map }
    }

    fn add_keys(map: &mut HashMap<(KeyCode, KeyModifiers), Action>, keys: &[String], action: Action) {
        for key in keys {
            if let Some((code, mods)) = parse_key(key) {
                map.insert((code, mods), action);
            }
        }
    }

    pub fn get_action(&self, event: KeyEvent) -> Option<Action> {
        self.map.get(&(event.code, event.modifiers)).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_tab_is_the_key_that_crossterm_sends() {
        // Crossterm reports shift+tab as BackTab with the shift modifier.
        assert_eq!(
            parse_key("backtab"),
            Some((KeyCode::BackTab, KeyModifiers::SHIFT))
        );
        assert_eq!(
            parse_key("shift+tab"),
            Some((KeyCode::BackTab, KeyModifiers::SHIFT))
        );
        assert_eq!(parse_key("tab"), Some((KeyCode::Tab, KeyModifiers::empty())));
    }

    #[test]
    fn the_map_finds_the_action_of_shift_tab() {
        let mapper = KeyMapper::new(&KeyConfig::default());
        let event = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        assert_eq!(mapper.get_action(event), Some(Action::PrevTab));
    }

    #[test]
    fn a_letter_is_the_key_that_crossterm_sends() {
        assert_eq!(
            parse_key("P"),
            Some((KeyCode::Char('P'), KeyModifiers::SHIFT))
        );
        assert_eq!(
            parse_key("ctrl+t"),
            Some((KeyCode::Char('t'), KeyModifiers::CONTROL))
        );
    }
}

fn parse_key(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let parts: Vec<&str> = s.split('+').collect();
    let mut mods = KeyModifiers::empty();

    for part in &parts[..parts.len().saturating_sub(1)] {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
            "shift" => mods |= KeyModifiers::SHIFT,
            "alt" | "meta" => mods |= KeyModifiers::ALT,
            _ => {}
        }
    }

    let key = parts.last().copied().unwrap_or("");
    let lower = key.to_lowercase();

    let code = match lower.as_str() {
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "enter" => Some(KeyCode::Enter),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "esc" | "escape" => Some(KeyCode::Esc),
        "tab" => Some(if mods.contains(KeyModifiers::SHIFT) {
            // Crossterm sends shift+tab as BackTab, never as Tab with the
            // shift modifier.
            KeyCode::BackTab
        } else {
            KeyCode::Tab
        }),
        "backtab" => {
            // Crossterm reports shift+tab as BackTab with the shift
            // modifier, so the map needs both.
            mods |= KeyModifiers::SHIFT;
            Some(KeyCode::BackTab)
        }
        "space" => Some(KeyCode::Char(' ')),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "pageup" => Some(KeyCode::PageUp),
        "pagedown" => Some(KeyCode::PageDown),
        "f1" => Some(KeyCode::F(1)),
        "f2" => Some(KeyCode::F(2)),
        "f3" => Some(KeyCode::F(3)),
        "f4" => Some(KeyCode::F(4)),
        "f5" => Some(KeyCode::F(5)),
        "f6" => Some(KeyCode::F(6)),
        "f7" => Some(KeyCode::F(7)),
        "f8" => Some(KeyCode::F(8)),
        "f9" => Some(KeyCode::F(9)),
        "f10" => Some(KeyCode::F(10)),
        "f11" => Some(KeyCode::F(11)),
        "f12" => Some(KeyCode::F(12)),
        _ => {
            let mut chars = key.chars();
            let mut c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            // Crossterm reports shifted letters as uppercase + SHIFT.
            if c.is_ascii_uppercase() {
                mods |= KeyModifiers::SHIFT;
            } else if mods.contains(KeyModifiers::SHIFT) && c.is_ascii_lowercase() {
                c = c.to_ascii_uppercase();
            }
            Some(KeyCode::Char(c))
        }
    };

    code.map(|c| (c, mods))
}