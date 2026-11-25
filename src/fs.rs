use crate::types::{FileEntry, FileType};
use anyhow::{Context, Result};
use walkdir::WalkDir;
use mime_guess::from_path;
use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::task;

pub async fn read_dir(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>> {
    let path = path.to_path_buf();
    let show_hidden = show_hidden;

    task::spawn_blocking(move || {
        let mut entries = Vec::new();

        for entry in fs::read_dir(&path).with_context(|| format!("Failed to read dir: {}", path.display()))? {
            let entry = entry?;
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();

            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let entry_path = entry.path();
            let metadata = entry.metadata()?;
            let file_type = metadata.file_type();
            let size = metadata.len();
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let permissions = metadata.mode() & 0o777;

            let ft = if file_type.is_dir() {
                FileType::Directory
            } else if file_type.is_symlink() {
                FileType::Symlink
            } else if file_type.is_file() {
                FileType::File
            } else if FileTypeExt::is_block_device(&file_type) {
                FileType::BlockDevice
            } else if FileTypeExt::is_char_device(&file_type) {
                FileType::CharDevice
            } else if FileTypeExt::is_fifo(&file_type) {
                FileType::Fifo
            } else if FileTypeExt::is_socket(&file_type) {
                FileType::Socket
            } else {
                FileType::Unknown
            };

            let mime_type = from_path(&entry_path).first_raw().map(|s| s.to_string());

            entries.push(FileEntry {
                path: entry_path,
                name: name.clone(),
                file_type: ft,
                size,
                modified,
                permissions,
                git_status: Default::default(),
                mime_type,
                is_hidden: name.starts_with('.'),
            });
        }

        Ok(entries)
    })
    .await?
}

pub async fn read_dir_recursive(path: &Path, show_hidden: bool) -> Result<Vec<FileEntry>> {
    let path = path.to_path_buf();
    let show_hidden = show_hidden;

    task::spawn_blocking(move || {
        let mut entries = Vec::new();

        let walker = WalkDir::new(&path)
            .follow_links(false)
            .into_iter();

        for entry in walker.filter_map(Result::ok) {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy().to_string();

            if !show_hidden && name.starts_with('.') {
                continue;
            }

            let entry_path = entry.path();
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let file_type = metadata.file_type();
            let size = metadata.len();
            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let permissions = metadata.mode() & 0o777;

            let ft = if file_type.is_dir() {
                FileType::Directory
            } else if file_type.is_symlink() {
                FileType::Symlink
            } else if file_type.is_file() {
                FileType::File
            } else if FileTypeExt::is_block_device(&file_type) {
                FileType::BlockDevice
            } else if FileTypeExt::is_char_device(&file_type) {
                FileType::CharDevice
            } else if FileTypeExt::is_fifo(&file_type) {
                FileType::Fifo
            } else if FileTypeExt::is_socket(&file_type) {
                FileType::Socket
            } else {
                FileType::Unknown
            };

            let mime_type = from_path(&entry_path).first_raw().map(|s| s.to_string());

            entries.push(FileEntry {
                path: entry_path.to_path_buf(),
                name,
                file_type: ft,
                size,
                modified,
                permissions,
                git_status: Default::default(),
                mime_type,
                is_hidden: name.starts_with('.'),
            });
        }

        Ok(entries)
    })
    .await?
}

pub fn sort_entries(entries: &mut [FileEntry], sort_by: crate::config::SortBy, dirs_first: bool, reverse: bool) {
    entries.sort_by(|a, b| {
        let dir_cmp = if dirs_first {
            b.is_dir().cmp(&a.is_dir())
        } else {
            std::cmp::Ordering::Equal
        };

        if dir_cmp != std::cmp::Ordering::Equal {
            return dir_cmp;
        }

        let cmp = match sort_by {
            crate::config::SortBy::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            crate::config::SortBy::Size => a.size.cmp(&b.size),
            crate::config::SortBy::Modified => a.modified.cmp(&b.modified),
            crate::config::SortBy::Extension => {
                a.extension().unwrap_or("").cmp(&b.extension().unwrap_or(""))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            }
        };

        if reverse {
            cmp.reverse()
        } else {
            cmp
        }
    });
}

pub async fn get_git_status(repo_path: &Path) -> Result<Vec<(PathBuf, crate::types::GitStatus)>> {
    let repo_path = repo_path.to_path_buf();
    task::spawn_blocking(move || {
        let mut statuses = Vec::new();
        let repo = match git2::Repository::open(&repo_path) {
            Ok(r) => r,
            Err(_) => return Ok(statuses),
        };

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);
        opts.include_ignored(false);
        opts.include_unmodified(false);

        let statuses_list = match repo.statuses(Some(&mut opts)) {
            Ok(s) => s,
            Err(_) => return Ok(statuses),
        };

        for entry in statuses_list.iter() {
            let path = entry.path().map(|p| repo_path.join(p));
            if let Some(path) = path {
                let status = entry.status();
                let gs = if status.contains(git2::Status::INDEX_NEW) {
                    crate::types::GitStatus::Added
                } else if status.contains(git2::Status::INDEX_MODIFIED) {
                    crate::types::GitStatus::Modified
                } else if status.contains(git2::Status::INDEX_DELETED) {
                    crate::types::GitStatus::Deleted
                } else if status.contains(git2::Status::INDEX_RENAMED) {
                    crate::types::GitStatus::Renamed
                } else if status.contains(git2::Status::INDEX_TYPECHANGE) {
                    crate::types::GitStatus::Modified
                } else if status.contains(git2::Status::WT_NEW) {
                    crate::types::GitStatus::Untracked
                } else if status.contains(git2::Status::WT_MODIFIED) {
                    crate::types::GitStatus::Modified
                } else if status.contains(git2::Status::WT_DELETED) {
                    crate::types::GitStatus::Deleted
                } else if status.contains(git2::Status::WT_RENAMED) {
                    crate::types::GitStatus::Renamed
                } else if status.contains(git2::Status::WT_TYPECHANGE) {
                    crate::types::GitStatus::Modified
                } else if status.contains(git2::Status::IGNORED) {
                    crate::types::GitStatus::Ignored
                } else if status.contains(git2::Status::CONFLICTED) {
                    crate::types::GitStatus::Conflicted
                } else {
                    crate::types::GitStatus::Unmodified
                };
                statuses.push((path, gs));
            }
        }

        Ok(statuses)
    })
    .await?
}

pub fn apply_git_status(entries: &mut [FileEntry], statuses: &[(PathBuf, crate::types::GitStatus)]) {
    for (path, gs) in statuses {
        for entry in entries.iter_mut() {
            if &entry.path == path {
                entry.git_status = *gs;
            }
        }
    }
}

pub fn find_git_root(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            break;
        }
    }
    None
}