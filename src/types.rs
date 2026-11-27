use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    Text,
    Image,
    Binary,
    None,
}

#[derive(Debug, Clone)]
pub struct PreviewContent {
    pub content: String,
    pub preview_type: PreviewType,
    pub image_path: Option<PathBuf>,
}

impl PreviewContent {
    pub fn text(content: String) -> Self {
        Self {
            content,
            preview_type: PreviewType::Text,
            image_path: None,
        }
    }

    pub fn image(path: PathBuf) -> Self {
        Self {
            content: String::new(),
            preview_type: PreviewType::Image,
            image_path: Some(path),
        }
    }

    pub fn binary() -> Self {
        Self {
            content: "[Binary file]".to_string(),
            preview_type: PreviewType::Binary,
            image_path: None,
        }
    }

    pub fn none() -> Self {
        Self {
            content: String::new(),
            preview_type: PreviewType::None,
            image_path: None,
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
    ConfirmOverwrite,
    Visual,
    TrashBrowser,
    OpenWith,
    QueueManager,
    ThemePicker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardOp {
    None,
    Copy,
    Cut,
}