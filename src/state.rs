use std::path::{Path, PathBuf};

pub struct SessionState {
    pub tabs: Vec<PathBuf>,
    pub active: usize,
    pub bookmarks: Vec<PathBuf>,
}

pub fn state_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/state")))
        .unwrap_or_else(|| std::env::temp_dir())
        .join("rfn")
}

fn state_file() -> PathBuf {
    state_dir().join("session")
}

pub fn load() -> Option<SessionState> {
    let text = std::fs::read_to_string(state_file()).ok()?;
    let mut tabs = Vec::new();
    let mut bookmarks = Vec::new();
    let mut active = 0;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("active=") {
            active = value.parse().unwrap_or(0);
        } else if let Some(value) = line.strip_prefix("tab=") {
            let path = PathBuf::from(value);
            if path.is_dir() {
                tabs.push(path);
            }
        } else if let Some(value) = line.strip_prefix("bookmark=") {
            let path = PathBuf::from(value);
            if path.is_dir() {
                bookmarks.push(path);
            }
        }
    }

    Some(SessionState {
        tabs,
        active,
        bookmarks,
    })
}

pub fn save(tabs: &[&Path], active: usize, bookmarks: &[PathBuf]) {
    let mut text = format!("active={active}\n");
    for tab in tabs {
        if let Some(tab) = tab.to_str() {
            if !tab.contains('\n') {
                text.push_str("tab=");
                text.push_str(tab);
                text.push('\n');
            }
        }
    }
    for bookmark in bookmarks {
        if let Some(bookmark) = bookmark.to_str() {
            if !bookmark.contains('\n') {
                text.push_str("bookmark=");
                text.push_str(bookmark);
                text.push('\n');
            }
        }
    }

    let dir = state_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let _ = std::fs::write(dir.join("session"), text);
}
