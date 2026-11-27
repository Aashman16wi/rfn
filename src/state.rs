use std::path::{Path, PathBuf};

pub struct SessionState {
    pub tabs: Vec<PathBuf>,
    pub active: usize,
    pub bookmarks: Vec<PathBuf>,
}


pub fn queue_dir() -> PathBuf {
    state_dir().join("queue")
}

fn queue_file() -> PathBuf {
    queue_dir().join("queue")
}

pub fn load_queue() -> Vec<QueueItem> {
    let text = match std::fs::read_to_string(queue_file()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut items = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("item=") {
            let parts: Vec<&str> = rest.splitn(3, '|').collect();
            if parts.len() == 3 {
                items.push(QueueItem {
                    op: parts[0].to_string(),
                    src: PathBuf::from(parts[1]),
                    dst: PathBuf::from(parts[2]),
                    status: QueueStatus::Pending,
                    overwrite: false,
                    error: None,
                });
            }
        }
    }
    items
}

pub fn save_queue(items: &[QueueItem]) {
    let dir = queue_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let mut text = String::new();
    for item in items {
        // Completed work is not restored; failed and pending tasks are kept
        // so a restart does not silently lose them.
        if !matches!(item.status, QueueStatus::Pending | QueueStatus::Failed) {
            continue;
        }
        text.push_str(&format!("item={}|{}|{}\n", item.op, item.src.display(), item.dst.display()));
    }
    let _ = std::fs::write(queue_file(), text);
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


pub fn _trash_dir() -> PathBuf {
    dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".local/share")))
        .unwrap_or_else(|| std::env::temp_dir())
        .join("Trash")
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


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub op: String,
    pub src: std::path::PathBuf,
    pub dst: std::path::PathBuf,
    pub status: QueueStatus,
    pub overwrite: bool,
    pub error: Option<String>,
}

