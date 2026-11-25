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
    SortName,
    SortSize,
    SortTime,
    SortExt,
    ReverseSort,
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
        Self::add_keys(&mut map, &config.sort_name, Action::SortName);
        Self::add_keys(&mut map, &config.sort_size, Action::SortSize);
        Self::add_keys(&mut map, &config.sort_time, Action::SortTime);
        Self::add_keys(&mut map, &config.sort_ext, Action::SortExt);
        Self::add_keys(&mut map, &config.reverse_sort, Action::ReverseSort);

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

fn parse_key(s: &str) -> Option<(KeyCode, KeyModifiers)> {
    let s = s.to_lowercase();
    let parts: Vec<&str> = s.split('+').collect();
    let mut mods = KeyModifiers::empty();
    let mut code = None;

    for part in &parts[..parts.len().saturating_sub(1)] {
        match *part {
            "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
            "shift" => mods |= KeyModifiers::SHIFT,
            "alt" | "meta" => mods |= KeyModifiers::ALT,
            _ => {}
        }
    }

    let key = parts.last().copied().unwrap_or("");
    code = match key {
        "up" => Some(KeyCode::Up),
        "down" => Some(KeyCode::Down),
        "left" => Some(KeyCode::Left),
        "right" => Some(KeyCode::Right),
        "enter" => Some(KeyCode::Enter),
        "backspace" => Some(KeyCode::Backspace),
        "delete" | "del" => Some(KeyCode::Delete),
        "esc" | "escape" => Some(KeyCode::Esc),
        "tab" => Some(KeyCode::Tab),
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
        c if c.len() == 1 => Some(KeyCode::Char(c.chars().next().unwrap())),
        _ => None,
    };

    code.map(|c| (c, mods))
}