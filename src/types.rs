use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    Directory,
    File,
    Symlink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitStatus {
    Unmodified,
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicted,
}

impl Default for GitStatus {
    fn default() -> Self {
        GitStatus::Unmodified
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub file_type: FileType,
    pub size: u64,
    pub modified: SystemTime,
    pub permissions: u32,
    pub git_status: GitStatus,
    pub mime_type: Option<String>,
    pub is_hidden: bool,
}

impl FileEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.file_type, FileType::Directory)
    }

    pub fn is_symlink(&self) -> bool {
        matches!(self.file_type, FileType::Symlink)
    }

    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|s| s.to_str())
    }

    pub fn display_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    Text,
    Image,
    Binary,
    Directory,
    None,
}

#[derive(Debug, Clone)]
pub struct PreviewContent {
    pub content: String,
    pub preview_type: PreviewType,
    pub truncated: bool,
}

impl PreviewContent {
    pub fn text(content: String) -> Self {
        Self {
            content,
            preview_type: PreviewType::Text,
            truncated: false,
        }
    }

    pub fn image() -> Self {
        Self {
            content: String::new(),
            preview_type: PreviewType::Image,
            truncated: false,
        }
    }

    pub fn binary() -> Self {
        Self {
            content: "[Binary file]".to_string(),
            preview_type: PreviewType::Binary,
            truncated: false,
        }
    }

    pub fn directory() -> Self {
        Self {
            content: "[Directory]".to_string(),
            preview_type: PreviewType::Directory,
            truncated: false,
        }
    }

    pub fn none() -> Self {
        Self {
            content: String::new(),
            preview_type: PreviewType::None,
            truncated: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,
    Rename,
    NewFile,
    NewDir,
    ConfirmDelete,
    ConfirmQuit,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    None,
    Copy,
    Cut,
}