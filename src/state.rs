use std::os::unix::ffi::OsStrExt;
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
            // The paths are written in a form that cannot break the line,
            // so a split on the separator is safe.
            let parts: Vec<&str> = rest.split('|').collect();
            if parts.len() < 3 {
                continue;
            }
            items.push(QueueItem {
                // Ids are handed out again when the app starts.
                id: 0,
                op: parts[0].to_string(),
                src: decode_path(parts[1]),
                dst: decode_path(parts[2]),
                // Work that was running when rfn stopped is done again.
                status: QueueStatus::Pending,
                // The last field says if the user allowed the destination
                // to be replaced. Older files do not have it.
                overwrite: parts.get(3).map(|value| *value == "1").unwrap_or(false),
                error: None,
            });
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
        // Finished work is not restored. Work that is running or that
        // failed is kept, so a restart does not silently lose it.
        if item.status == QueueStatus::Done {
            continue;
        }
        text.push_str(&format!(
            "item={}|{}|{}|{}\n",
            item.op,
            encode_path(&item.src),
            encode_path(&item.dst),
            if item.overwrite { "1" } else { "0" }
        ));
    }
    let _ = std::fs::write(queue_file(), text);
}

/// A path in a form that can be put in the queue file without breaking it.
///
/// A `|` or a line break in a file name would split the line in the wrong
/// place, and a name that is not valid UTF-8 would be changed on the way.
fn encode_path(path: &Path) -> String {
    let mut out = String::new();
    for byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(*byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The path that `encode_path` wrote.
fn decode_path(text: &str) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;

    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(value) = hex.and_then(|text| u8::from_str_radix(text, 16).ok()) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    PathBuf::from(std::ffi::OsStr::from_bytes(&out))
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
    /// Stable identifier. The worker reports its result with this id, so
    /// removing or reordering tasks while the queue runs cannot mix up the
    /// statuses.
    pub id: u64,
    pub op: String,
    pub src: std::path::PathBuf,
    pub dst: std::path::PathBuf,
    pub status: QueueStatus,
    pub overwrite: bool,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_with_a_pipe_survives_the_queue_file() {
        let path = PathBuf::from("/tmp/notes|2024.txt");
        let encoded = encode_path(&path);
        assert!(!encoded.contains('|'));
        assert_eq!(decode_path(&encoded), path);
    }

    #[test]
    fn a_path_with_a_line_break_survives_the_queue_file() {
        let path = PathBuf::from("/tmp/two\nlines.txt");
        let encoded = encode_path(&path);
        assert!(!encoded.contains('\n'));
        assert_eq!(decode_path(&encoded), path);
    }

    #[test]
    fn a_path_that_is_not_utf8_survives_the_queue_file() {
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(b"/tmp/f\x80oo"));
        let encoded = encode_path(&path);
        assert_eq!(decode_path(&encoded), path);
    }

    #[test]
    fn a_normal_path_stays_readable() {
        let path = PathBuf::from("/home/user/Documents/notes.txt");
        assert_eq!(encode_path(&path), "/home/user/Documents/notes.txt");
        assert_eq!(decode_path(&encode_path(&path)), path);
    }
}

