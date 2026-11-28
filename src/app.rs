use crate::config::{AppConfig, SortBy};
use crate::fs::{apply_git_status, find_git_root, get_git_status, read_dir, sort_entries};
use crate::keys::{Action, KeyMapper};
use crate::preview::generate_preview;
use crate::state::{QueueItem, QueueStatus, load_queue, save_queue};


use crate::types::{AppMode, ClipboardOp, FileEntry, PreviewContent, PreviewType};
use crate::ui::file_list::{
    go_to_bottom, go_to_top, next, page_down, page_up, previous,
};
use crate::ui::help::render as render_help;
use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::interval;
use tokio::sync::mpsc;

/// Progress messages sent from the background queue worker back to the UI.
#[derive(Debug)]
enum QueueEvent {
    /// The task id (not the index) and the outcome of the work.
    Finished(u64, std::result::Result<(), String>),
    AllDone,
}

/// Results produced by the background directory loader. Every result carries
/// the generation it was requested with, so a slow scan of an old directory can
/// never overwrite a newer one.
#[derive(Debug)]
enum LoadResult {
    Entries {
        generation: u64,
        path: PathBuf,
        entries: Vec<FileEntry>,
        parent_entries: Vec<FileEntry>,
    },
    Git {
        generation: u64,
        path: PathBuf,
        root: PathBuf,
        statuses: Vec<(PathBuf, crate::types::GitStatus)>,
    },
    Error {
        generation: u64,
        path: PathBuf,
        message: String,
    },
}

/// What woke the event loop.
enum InputEvent {
    Term(Event),
    Queue(QueueEvent),
    Load(LoadResult),
}

/// Build an image picker without poking at stdio. `Picker::from_query_stdio`
/// spawns a thread that can disable raw mode behind our back when the terminal
/// does not answer its queries, which breaks all keyboard input.
pub fn create_picker(config: &AppConfig) -> Picker {
    let protocol = match config.preview.image_backend {
        crate::config::ImageBackend::Kitty => ProtocolType::Kitty,
        crate::config::ImageBackend::Iterm => ProtocolType::Iterm2,
        crate::config::ImageBackend::Sixel => ProtocolType::Sixel,
        crate::config::ImageBackend::Chafa => ProtocolType::Halfblocks,
        crate::config::ImageBackend::Auto => detect_protocol(),
    };

    let mut picker = Picker::from_fontsize((10, 20));
    picker.set_protocol_type(protocol);
    picker
}

fn detect_protocol() -> ProtocolType {
    let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    if std::env::var_os("KITTY_WINDOW_ID").is_some()
        || term.contains("kitty")
        || term.contains("ghostty")
        || term_program == "WezTerm"
    {
        ProtocolType::Kitty
    } else if term_program == "iTerm.app" {
        ProtocolType::Iterm2
    } else if term.contains("sixel") || term.contains("foot") || term.contains("mlterm") {
        ProtocolType::Sixel
    } else {
        ProtocolType::Halfblocks
    }
}

#[derive(Clone)]
struct TabState {
    current_path: PathBuf,
    entries: Vec<FileEntry>,
    display: Vec<FileEntry>,
    parent_entries: Vec<FileEntry>,
    list_state: ratatui::widgets::ListState,
    search_query: String,
    preview_scroll: usize,
    preview_zoom: f32,
}

pub struct App {
    config: AppConfig,
    key_mapper: KeyMapper,
    picker: Picker,

    current_path: PathBuf,
    entries: Vec<FileEntry>,
    display: Vec<FileEntry>,
    parent_entries: Vec<FileEntry>,
    list_state: ratatui::widgets::ListState,
    list_inner: ratatui::layout::Rect,
    tabs: Vec<TabState>,
    active_tab: usize,
    bookmarks: Vec<PathBuf>,
    trash_entries: Vec<FileEntry>,
    /// The real trash items, in the same order as `trash_entries`.
    trash_items: Vec<trash::TrashItem>,
    trash_list_state: ratatui::widgets::ListState,
    open_with_buffer: String,

    preview_content: PreviewContent,
    preview_protocol: Option<StatefulProtocol>,
    preview_scroll: usize,
    preview_zoom: f32,

    git_statuses: Vec<(PathBuf, crate::types::GitStatus)>,
    git_root: Option<PathBuf>,

    mode: AppMode,
    overwrite_count: usize,
    theme_index: usize,
    bookmark_state: ratatui::widgets::ListState,
    search_query: String,
    rename_buffer: String,
    new_name_buffer: String,

    clipboard: ClipboardOp,
    clipboard_paths: Vec<PathBuf>,
    queue: Vec<QueueItem>,
    /// Counter for the stable task ids.
    next_queue_id: u64,
    queue_state: ratatui::widgets::ListState,
    queue_rx: Option<mpsc::UnboundedReceiver<QueueEvent>>,
    queue_running: bool,
    marked: HashSet<PathBuf>,

    show_hidden: bool,
    show_preview: bool,
    show_git: bool,
    sort_by: SortBy,
    reverse_sort: bool,

    // Background directory loading
    load_generation: u64,
    load_tx: Option<mpsc::UnboundedSender<LoadResult>>,
    load_rx: Option<mpsc::UnboundedReceiver<LoadResult>>,
    loading: bool,
    git_loading: bool,

    help_visible: bool,
    notice: Option<String>,
    pending_bulk_rename: bool,

    should_quit: bool,
    last_tick: std::time::Instant,
}

impl App {
    pub async fn new(
        config: AppConfig,
        start_path: std::path::PathBuf,
        picker: Picker,
        restore_tabs: bool,
    ) -> Result<Self> {
        let current_path = if start_path.is_absolute() {
            start_path
        } else {
            std::env::current_dir()?.join(start_path)
        };
        let show_hidden = config.ui.show_hidden;
        let show_preview = config.preview.enabled;
        let show_git = config.git.enabled;
        let sort_by = config.behavior.sort_by;
        let reverse_sort = config.behavior.reverse_sort;

        let key_mapper = KeyMapper::new(&config.keys);
        let (load_tx, load_rx) = mpsc::unbounded_channel();

        let mut app = Self {
            config,
            key_mapper,
            picker,
            current_path,
            entries: Vec::new(),
            display: Vec::new(),
            parent_entries: Vec::new(),
            list_state: ratatui::widgets::ListState::default(),
            list_inner: ratatui::layout::Rect::default(),
            tabs: Vec::new(),
            active_tab: 0,
            bookmarks: Vec::new(),
            trash_entries: Vec::new(),
            trash_items: Vec::new(),
            trash_list_state: ratatui::widgets::ListState::default(),
            open_with_buffer: String::new(),
            preview_content: PreviewContent::none(),
            preview_protocol: None,
            preview_scroll: 0,
            preview_zoom: 1.0,
            git_statuses: Vec::new(),
            git_root: None,
            mode: AppMode::Normal,
            overwrite_count: 0,
            theme_index: 0,
            bookmark_state: ratatui::widgets::ListState::default(),
            search_query: String::new(),
            rename_buffer: String::new(),
            new_name_buffer: String::new(),
            clipboard: ClipboardOp::None,
            clipboard_paths: Vec::new(),
            queue: load_queue(),
            next_queue_id: 0,
            queue_state: ratatui::widgets::ListState::default(),
            queue_rx: None,
            queue_running: false,
            marked: HashSet::new(),
            show_hidden,
            show_preview,
            show_git,
            sort_by,
            reverse_sort,
            load_generation: 0,
            load_tx: Some(load_tx),
            load_rx: Some(load_rx),
            loading: false,
            git_loading: false,
            help_visible: false,
            notice: None,
            pending_bulk_rename: false,
            should_quit: false,
            last_tick: std::time::Instant::now(),
        };

        // Restored tasks need fresh ids, they are unique per session.
        for item in app.queue.iter_mut() {
            item.id = app.next_queue_id;
            app.next_queue_id += 1;
        }

        // The first listing is awaited so the very first frame is complete;
        // every later refresh happens in the background.
        app.refresh();
        app.wait_for_entries().await;
        app.tabs.push(app.snapshot());

        if restore_tabs {
            if let Some(saved) = crate::state::load() {
                let mut restored = Vec::new();
                for path in &saved.tabs {
                    if path.is_dir() {
                        app.current_path = path.clone();
                        app.refresh();
                        app.wait_for_entries().await;
                        restored.push(app.snapshot());
                    }
                }
                if !restored.is_empty() {
                    app.tabs = restored;
                    app.active_tab = saved.active.min(app.tabs.len() - 1);
                    app.restore_active_tab();
                    app.refresh();
                    app.wait_for_entries().await;
                }
                app.bookmarks = saved.bookmarks;
            }
        }

        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let mut tick_interval = interval(Duration::from_secs(2));
        terminal.draw(|f| self.render(f))?;

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if self.tick().await? {
                        terminal.draw(|f| self.render(f))?;
                    }
                }
                event = self.read_event() => {
                    if let Some(event) = event? {
                        match event {
                            InputEvent::Term(event) => {
                                self.handle_event(event).await?;
                                if self.pending_bulk_rename {
                                    self.pending_bulk_rename = false;
                                    self.bulk_rename(terminal).await?;
                                }
                            }
                            InputEvent::Queue(event) => {
                                self.handle_queue_event(event).await?;
                            }
                            InputEvent::Load(result) => {
                                self.apply_load_result(result).await;
                            }
                        }
                        terminal.draw(|f| self.render(f))?;
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        let paths: Vec<PathBuf> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                if index == self.active_tab {
                    self.current_path.clone()
                } else {
                    tab.current_path.clone()
                }
            })
            .collect();
        let paths: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        crate::state::save(&paths, self.active_tab, &self.bookmarks);
        save_queue(&self.queue);

        Ok(())
    }

    async fn read_event(&mut self) -> Result<Option<InputEvent>> {
        loop {
            // Drain queue progress before waiting on the terminal so a busy
            // copy does not make the interface feel frozen.
            if let Some(rx) = self.queue_rx.as_mut() {
                match rx.try_recv() {
                    Ok(event) => return Ok(Some(InputEvent::Queue(event))),
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.queue_rx = None;
                        self.queue_running = false;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
                }
            }

            // Directory listings arrive in the background; apply them as soon
            // as they show up so the window never feels stuck.
            if let Some(rx) = self.load_rx.as_mut() {
                match rx.try_recv() {
                    Ok(result) => return Ok(Some(InputEvent::Load(result))),
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.load_rx = None;
                        self.loading = false;
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {}
                }
            }

            if !crossterm::event::poll(Duration::from_millis(50))? {
                return Ok(None);
            }
            let event = crossterm::event::read()?;
            // Mouse-move tracking is on so clicks work, but moves should not
            // force a full redraw (and an image retransmit).
            if matches!(
                event,
                Event::Mouse(mouse)
                    if mouse.kind == crossterm::event::MouseEventKind::Moved
            ) {
                continue;
            }
            return Ok(Some(InputEvent::Term(event)));
        }
    }

    async fn tick(&mut self) -> Result<bool> {
        if self.last_tick.elapsed() < Duration::from_secs(2) {
            return Ok(false);
        }
        self.last_tick = std::time::Instant::now();

        // Git status is re-read in the background; the result is applied when
        // it arrives, so the interface never blocks on a big repository.
        if self.show_git {
            self.request_git_refresh();
        }

        Ok(false)
    }

    /// Ask the background loader for a fresh listing of the current directory.
    /// The UI stays responsive: the result is applied when it arrives, and
    /// results from an older generation are dropped.
    fn refresh(&mut self) {
        if self.mode == AppMode::TrashBrowser {
            // The trash browser has its own listing; it is loaded when the
            // mode is opened and when an item is restored.
            return;
        }

        self.load_generation = self.load_generation.wrapping_add(1);
        let generation = self.load_generation;

        let path = self.current_path.clone();
        let show_hidden = self.show_hidden;
        let sort_by = self.sort_by;
        let reverse_sort = self.reverse_sort;
        let dirs_first = self.config.behavior.sort_dirs_first;
        let git_enabled = self.show_git;

        // One channel for every background result. Results are told apart by
        // their generation, so a slow scan of an old directory can never
        // overwrite a newer one and nothing is ever lost to a dropped
        // receiver.
        let Some(tx) = self.load_tx.clone() else {
            return;
        };
        self.loading = true;

        tokio::spawn(async move {
            let mut entries = match read_dir(&path, show_hidden).await {
                Ok(entries) => entries,
                Err(error) => {
                    let _ = tx.send(LoadResult::Error {
                        generation,
                        path: path.clone(),
                        message: format!("Cannot read {}: {error}", path.display()),
                    });
                    return;
                }
            };
            sort_entries(&mut entries, sort_by, dirs_first, reverse_sort);

            let parent_entries = match path.parent() {
                Some(parent) => {
                    let mut entries = read_dir(parent, show_hidden).await.unwrap_or_default();
                    sort_entries(&mut entries, sort_by, dirs_first, reverse_sort);
                    entries
                }
                None => Vec::new(),
            };

            if tx
                .send(LoadResult::Entries {
                    generation,
                    path: path.clone(),
                    entries,
                    parent_entries,
                })
                .is_err()
            {
                return;
            }

            if git_enabled {
                if let Some(root) = find_git_root(&path) {
                    let statuses = get_git_status(&root).await.unwrap_or_default();
                    let _ = tx.send(LoadResult::Git {
                        generation,
                        path,
                        root,
                        statuses,
                    });
                }
            }
        });
    }

    /// Wait for the first listing to arrive. Only used at startup so the
    /// first frame is complete.
    async fn wait_for_entries(&mut self) {
        loop {
            let received = match self.load_rx.as_mut() {
                Some(rx) => match rx.try_recv() {
                    Ok(result) => {
                        self.apply_load_result(result).await;
                        !self.loading
                    }
                    Err(mpsc::error::TryRecvError::Empty) => false,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.loading = false;
                        true
                    }
                },
                None => true,
            };
            if received {
                return;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    /// Apply a background result, ignoring anything that is out of date.
    async fn apply_load_result(&mut self, result: LoadResult) {
        match result {
            LoadResult::Entries {
                generation,
                path,
                entries,
                parent_entries,
            } => {
                if generation != self.load_generation || path != self.current_path {
                    return;
                }
                self.entries = entries;
                self.parent_entries = parent_entries;
                self.loading = false;
                self.rebuild_display();
                self.update_preview().await;
            }
            LoadResult::Git {
                generation,
                path,
                root,
                statuses,
            } => {
                // Always clear the in-flight flag, even for a stale result,
                // otherwise git polling would stop for good.
                self.git_loading = false;
                if generation != self.load_generation || path != self.current_path {
                    return;
                }
                self.git_root = Some(root);
                self.git_statuses = statuses;
                apply_git_status(&mut self.entries, &self.git_statuses);
                self.rebuild_display();
            }
            LoadResult::Error {
                generation,
                path,
                message,
            } => {
                if generation != self.load_generation || path != self.current_path {
                    return;
                }
                self.loading = false;
                self.entries.clear();
                self.parent_entries.clear();
                self.rebuild_display();
                self.notice = Some(message);
            }
        }
    }

    /// Re-read the git status of the current repository without blocking the
    /// interface. Used by the periodic tick so changes show up live.
    fn request_git_refresh(&mut self) {
        if !self.show_git || self.git_loading {
            return;
        }
        let Some(tx) = self.load_tx.clone() else {
            return;
        };
        let Some(root) = find_git_root(&self.current_path) else {
            self.git_root = None;
            self.git_statuses.clear();
            for entry in &mut self.entries {
                entry.git_status = Default::default();
            }
            return;
        };

        let generation = self.load_generation;
        let path = self.current_path.clone();
        self.git_loading = true;

        tokio::spawn(async move {
            let statuses = get_git_status(&root).await.unwrap_or_default();
            let _ = tx.send(LoadResult::Git {
                generation,
                path,
                root,
                statuses,
            });
        });
    }

    async fn refresh_trash(&mut self) -> Result<()> {
        use trash::os_limited::{list, metadata};

        self.trash_entries.clear();
        self.trash_items.clear();

        let items = match list() {
            Ok(items) => items,
            Err(error) => {
                self.notice = Some(format!("Cannot read the trash: {error}"));
                return Ok(());
            }
        };

        // The trash item keeps the pointer to the real trashed file, the
        // FileEntry is what gets drawn. They are built together so both
        // lists always describe the same item at the same index.
        let mut rows: Vec<(trash::TrashItem, FileEntry)> = Vec::new();

        for item in items {
            let original_path = item.original_path();
            let name = item
                .name
                .to_str()
                .map(str::to_string)
                .unwrap_or_else(|| original_path.display().to_string());

            let (size, is_dir) = match metadata(&item) {
                Ok(info) => {
                    let is_dir = matches!(info.size, trash::TrashItemSize::Entries(_));
                    (info.size.size().unwrap_or(0), is_dir)
                }
                Err(_) => (0, false),
            };

            let modified = if item.time_deleted >= 0 {
                std::time::SystemTime::UNIX_EPOCH
                    + std::time::Duration::from_secs(item.time_deleted as u64)
            } else {
                std::time::SystemTime::UNIX_EPOCH
            };

            rows.push((
                item,
                FileEntry {
                    path: original_path,
                    name,
                    file_type: if is_dir {
                        crate::types::FileType::Directory
                    } else {
                        crate::types::FileType::File
                    },
                    size,
                    modified,
                    permissions: 0,
                    git_status: crate::types::GitStatus::Unmodified,
                    mime_type: None,
                    is_hidden: false,
                },
            ));
        }

        // Newest first.
        rows.sort_by(|left, right| right.0.time_deleted.cmp(&left.0.time_deleted));

        for (item, entry) in rows {
            self.trash_items.push(item);
            self.trash_entries.push(entry);
        }

        if self.trash_entries.is_empty() {
            self.trash_list_state.select(None);
            *self.trash_list_state.offset_mut() = 0;
        } else {
            self.trash_list_state.select(Some(0));
        }
        Ok(())
    }

    /// The trash item that is currently highlighted.
    fn selected_trash_item(&self) -> Option<&trash::TrashItem> {
        self.trash_list_state
            .selected()
            .and_then(|index| self.trash_items.get(index))
    }

    fn rebuild_display(&mut self) {
        let query = self.search_query.to_lowercase();
        self.display = if query.is_empty() {
            self.entries.clone()
        } else {
            self.entries
                .iter()
                .filter(|entry| entry.name.to_lowercase().contains(&query))
                .cloned()
                .collect()
        };

        if self.display.is_empty() {
            self.list_state.select(None);
            *self.list_state.offset_mut() = 0;
        } else {
            let selected = self
                .list_state
                .selected()
                .unwrap_or(0)
                .min(self.display.len() - 1);
            self.list_state.select(Some(selected));
            let offset = self.list_state.offset().min(self.display.len() - 1);
            *self.list_state.offset_mut() = offset;
        }
    }

    async fn update_preview(&mut self) {
        self.preview_protocol = None;

        if !self.show_preview {
            self.preview_content = PreviewContent::none();
            return;
        }

        let entry = match self.selected_entry().cloned() {
            Some(entry) => entry,
            None => {
                self.preview_content = PreviewContent::none();
                return;
            }
        };

        let max_size = self.config.preview.max_size;
        let show_images = self.config.preview.show_images;
        let show_icons = self.config.ui.show_icons;

        self.preview_content = match generate_preview(&entry, max_size, show_images, show_icons).await {
            Ok(content) => content,
            Err(_) => PreviewContent::none(),
        };

        if self.preview_content.preview_type == PreviewType::Image {
            if let Some(path) = self.preview_content.image_path.clone() {
                let decoded: anyhow::Result<image::DynamicImage> = (|| {
                    let reader = image::ImageReader::open(&path)?.with_guessed_format()?;
                    Ok(reader.decode()?)
                })();

                match decoded {
                    Ok(img) => {
                        let zoom = self.preview_zoom.clamp(0.25, 4.0);
                        let img = zoom_image(img, zoom);
                        self.preview_protocol = Some(self.picker.new_resize_protocol(img));
                    }
                    Err(e) => {
                        self.preview_content =
                            PreviewContent::text(format!("Cannot decode image\n\n{e}"));
                    }
                }
            }
        }
    }

    fn snapshot(&self) -> TabState {
        TabState {
            current_path: self.current_path.clone(),
            entries: self.entries.clone(),
            display: self.display.clone(),
            parent_entries: self.parent_entries.clone(),
            list_state: self.list_state.clone(),
            search_query: self.search_query.clone(),
            preview_scroll: self.preview_scroll,
            preview_zoom: self.preview_zoom,
        }
    }

    fn save_active_tab(&mut self) {
        let snapshot = self.snapshot();
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            *tab = snapshot;
        }
    }

    fn restore_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab).cloned() {
            self.current_path = tab.current_path;
            self.entries = tab.entries;
            self.display = tab.display;
            self.parent_entries = tab.parent_entries;
            self.list_state = tab.list_state;
            self.search_query = tab.search_query;
            self.preview_scroll = tab.preview_scroll;
            self.preview_zoom = tab.preview_zoom;
        }
    }

    async fn switch_tab(&mut self, index: usize) -> Result<()> {
        if index >= self.tabs.len() || index == self.active_tab {
            return Ok(());
        }
        self.save_active_tab();
        self.marked.clear();
        self.mode = AppMode::Normal;
        self.active_tab = index;
        self.restore_active_tab();
        self.refresh();
        Ok(())
    }

    async fn new_tab(&mut self) {
        self.save_active_tab();
        let snapshot = self.snapshot();
        self.tabs.push(snapshot);
        self.active_tab = self.tabs.len() - 1;
    }

    async fn close_tab(&mut self) -> Result<()> {
        if self.tabs.len() <= 1 {
            self.notice = Some("Cannot close the last tab".to_string());
            return Ok(());
        }

        self.marked.clear();
        self.mode = AppMode::Normal;
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.restore_active_tab();
        self.refresh();
        Ok(())
    }

    fn target_paths(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            return self
                .selected_entry()
                .map(|entry| vec![entry.path.clone()])
                .unwrap_or_default();
        }

        self.entries
            .iter()
            .filter(|entry| self.marked.contains(&entry.path))
            .map(|entry| entry.path.clone())
            .collect()
    }

    fn mark_current(&mut self) {
        if let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) {
            self.marked.insert(path);
        }
    }

    fn toggle_mark(&mut self) {
        if let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) {
            if !self.marked.insert(path.clone()) {
                self.marked.remove(&path);
            }
        }
    }

    fn clear_marks(&mut self) {
        self.marked.clear();
        if self.mode == AppMode::Visual {
            self.mode = AppMode::Normal;
        }
    }

    fn selected_entry(&self) -> Option<&FileEntry> {
        self.list_state
            .selected()
            .and_then(|index| self.display.get(index))
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        let colors = self.config.ui.active_colors();

        frame.render_widget(
            ratatui::widgets::Block::default()
                .style(ratatui::style::Style::default().bg(crate::ui::parse_color(&colors.bg))),
            area,
        );

        if self.help_visible {
            render_help(frame, area, &self.config.keys, colors);
            return;
        }

        let show_tabs = self.tabs.len() > 1;
        let in_trash = self.mode == AppMode::TrashBrowser;
        let areas = crate::ui::layout(
            area,
            self.show_preview,
            self.config.ui.show_parent && !in_trash,
            show_tabs,
        );

        let current_title = if in_trash {
            "Trash".to_string()
        } else {
            dir_title(&self.current_path)
        };

        // Tab bar (only when there is more than one tab)
        if let Some(tabs_area) = areas.tabs {
            let tabs: Vec<(String, bool)> = self
                .tabs
                .iter()
                .enumerate()
                .map(|(index, tab)| {
                    let path = if index == self.active_tab {
                        &self.current_path
                    } else {
                        &tab.current_path
                    };
                    (dir_title(path), index == self.active_tab)
                })
                .collect();
            crate::ui::top_bar::render_tabs(frame, tabs_area, &tabs, colors);
        }

        // Breadcrumb bar
        crate::ui::top_bar::render(
            frame,
            areas.top,
            &self.current_path,
            if in_trash {
                self.trash_entries.len()
            } else {
                self.display.len()
            },
            colors,
            if in_trash { Some("Trash") } else { None },
        );

        // Parent column (Miller style)
        if let Some(parent_area) = areas.parent {
            let current_name = self
                .current_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            let parent_title = self
                .current_path
                .parent()
                .map(dir_title)
                .unwrap_or_else(|| "/".to_string());
            crate::ui::file_list::render_parent(
                frame,
                parent_area,
                &self.parent_entries,
                current_name,
                colors,
                self.config.ui.show_icons,
                &parent_title,
            );
        }

        // File list
        if in_trash {
            self.list_inner = crate::ui::file_list::render(
                frame,
                areas.current,
                &self.trash_entries,
                &mut self.trash_list_state,
                colors,
                self.config.ui.show_icons,
                false,
                &self.marked,
                &current_title,
                false,
            );
        } else {
            self.list_inner = crate::ui::file_list::render(
                frame,
                areas.current,
                &self.display,
                &mut self.list_state,
                colors,
                self.config.ui.show_icons,
                self.show_git,
                &self.marked,
                &current_title,
                self.loading,
            );
        }

        // Preview
        if let Some(preview_area) = areas.preview {
            // In the trash the file itself is gone, so show where it came from.
            let trash_preview = if in_trash {
                self.trash_list_state
                    .selected()
                    .and_then(|index| self.trash_entries.get(index))
                    .map(|entry| {
                        PreviewContent::text(format!(
                            "Deleted item\n\nName: {}\nOriginal path: {}\n\nPress r to restore it,\nd to delete it forever.",
                            entry.name,
                            entry.path.display()
                        ))
                    })
            } else {
                None
            };

            let content = trash_preview.as_ref().unwrap_or(&self.preview_content);
            let protocol = if in_trash {
                None
            } else {
                self.preview_protocol.as_mut()
            };

            crate::ui::preview::render(
                frame,
                preview_area,
                content,
                protocol,
                colors,
                self.preview_scroll,
                self.preview_zoom,
            );
        }

        // Transient error notice above the status bar.
        if let Some(notice) = self.notice.clone() {
            let notice_area = ratatui::layout::Rect::new(
                area.x,
                area.y + area.height.saturating_sub(2),
                area.width,
                1,
            );
            frame.render_widget(ratatui::widgets::Clear, notice_area);
            frame.render_widget(
                ratatui::widgets::Paragraph::new(ratatui::text::Line::from(
                    ratatui::text::Span::styled(
                        format!(" {notice} "),
                        ratatui::style::Style::default()
                            .bg(ratatui::style::Color::Rgb(247, 118, 142))
                            .fg(ratatui::style::Color::Rgb(26, 27, 38))
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                ))
                .style(
                    ratatui::style::Style::default()
                        .bg(ratatui::style::Color::Rgb(247, 118, 142)),
                ),
                notice_area,
            );
        }

        // Status bar
        let status_area = areas.status;
        let selected_entry = match self.mode {
            AppMode::TrashBrowser => self.trash_list_state.selected().and_then(|i| self.trash_entries.get(i)),
            _ => self.selected_entry(),
        };
        crate::ui::status_bar::render(
            frame,
            status_area,
            self.mode,
            selected_entry,
            self.clipboard,
            self.clipboard_paths.len(),
            self.marked.len(),
            self.show_hidden,
            self.show_preview,
            self.show_git,
            self.sort_by,
            self.reverse_sort,
            &self.search_query,
            &self.config.ui.theme,
            colors,
        );

        // Typing prompt replaces the status bar; confirmations float on top.
        match self.mode {
            AppMode::Search => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.search_query,
                self.config.ui.active_colors(),
            ),
            AppMode::Rename => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.rename_buffer,
                self.config.ui.active_colors(),
            ),
            AppMode::NewFile | AppMode::NewDir => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.new_name_buffer,
                self.config.ui.active_colors(),
            ),
            AppMode::ConfirmDelete => {
                let name = self
                    .selected_entry()
                    .map(|entry| entry.name.clone())
                    .unwrap_or_default();
                let (title, message) = if self.config.behavior.use_trash {
                    (" Trash ", format!("Move \"{name}\" to trash?"))
                } else {
                    (" Delete ", format!("Delete \"{name}\" permanently?"))
                };
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    title,
                    &message,
                    self.config.ui.active_colors(),
                );
            }
            AppMode::ConfirmQuit => {
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    " Quit ",
                    "Quit rfn?",
                    self.config.ui.active_colors(),
                );
            }
            AppMode::ConfirmOverwrite => {
                let message = format!(
                    "{} existing item{} will be replaced. Continue?",
                    self.overwrite_count,
                    if self.overwrite_count == 1 { "" } else { "s" }
                );
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    " Overwrite ",
                    &message,
                    self.config.ui.active_colors(),
                );
            }
            AppMode::ConfirmPurge => {
                let name = self
                    .trash_list_state
                    .selected()
                    .and_then(|index| self.trash_entries.get(index))
                    .map(|entry| entry.name.clone())
                    .unwrap_or_default();
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    " Delete forever ",
                    &format!("Delete \"{name}\" permanently? This cannot be undone."),
                    self.config.ui.active_colors(),
                );
            }
            AppMode::QueueManager => {
                crate::ui::queue::render(
                    frame,
                    area,
                    &self.queue,
                    &mut self.queue_state,
                    self.queue_running,
                    self.notice.as_deref(),
                    self.config.ui.active_colors(),
                );
            }
            AppMode::ThemePicker => {
                crate::ui::theme_picker::render(
                    frame,
                    area,
                    self.theme_index,
                    &self.config.ui.theme,
                    self.config.ui.active_colors(),
                    &self.config.ui.colors,
                );
            }
            AppMode::BookmarkPicker => {
                crate::ui::bookmarks::render(
                    frame,
                    area,
                    &self.bookmarks,
                    &mut self.bookmark_state,
                    self.config.ui.active_colors(),
                );
            }
            _ => {}
        }
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                self.notice = None;

                if self.help_visible {
                    self.handle_help(key).await?;
                    return Ok(());
                }

                match self.mode {
                    AppMode::Normal => self.handle_normal(key).await?,
                    AppMode::Search => self.handle_search(key).await?,
                    AppMode::Rename => self.handle_rename(key).await?,
                    AppMode::NewFile => self.handle_new_file(key).await?,
                    AppMode::NewDir => self.handle_new_dir(key).await?,
                    AppMode::ConfirmDelete => self.handle_confirm_delete(key).await?,
                    AppMode::ConfirmQuit => self.handle_confirm_quit(key).await?,
                    AppMode::ConfirmOverwrite => self.handle_confirm_overwrite(key).await?,
                    AppMode::ConfirmPurge => self.handle_confirm_purge(key).await?,
                    AppMode::Visual => self.handle_normal(key).await?,
                    AppMode::TrashBrowser => self.handle_trash(key).await?,
                    AppMode::OpenWith => self.handle_open_with(key).await?,
                    AppMode::QueueManager => self.handle_queue(key).await?,
                    AppMode::ThemePicker => self.handle_theme_picker(key),
                    AppMode::BookmarkPicker => self.handle_bookmark_picker(key).await?,
                }
            }
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse).await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Result<()> {
        use crossterm::event::{MouseEventKind, MouseButton};

        // The trash browser has its own list, so clicks and scrolling have to
        // work on that list instead of the current directory.
        if self.mode == AppMode::TrashBrowser {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    let inner = self.list_inner;
                    if mouse.row >= inner.y
                        && mouse.row < inner.y + inner.height
                        && mouse.column >= inner.x
                        && mouse.column < inner.x + inner.width
                    {
                        let index = self.trash_list_state.offset() + (mouse.row - inner.y) as usize;
                        if index < self.trash_entries.len() {
                            self.trash_list_state.select(Some(index));
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    previous(&mut self.trash_list_state, self.trash_entries.len());
                }
                MouseEventKind::ScrollDown => {
                    next(&mut self.trash_list_state, self.trash_entries.len());
                }
                _ => {}
            }
            return Ok(());
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inner = self.list_inner;
                let inside = mouse.row >= inner.y
                    && mouse.row < inner.y + inner.height
                    && mouse.column >= inner.x
                    && mouse.column < inner.x + inner.width;

                if inside {
                    let index = self.list_state.offset() + (mouse.row - inner.y) as usize;
                    if index < self.display.len() {
                        self.list_state.select(Some(index));
                        if self.mode == AppMode::Visual {
                            self.mark_current();
                        }
                        self.update_preview().await;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                previous(&mut self.list_state, self.display.len());
                self.update_preview().await;
            }
            MouseEventKind::ScrollDown => {
                next(&mut self.list_state, self.display.len());
                self.update_preview().await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_normal(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};

        if key.code == KeyCode::Esc {
            if self.mode == AppMode::Visual || !self.marked.is_empty() {
                self.clear_marks();
                return Ok(());
            }
            if !self.search_query.is_empty() {
                self.search_query.clear();
                self.rebuild_display();
                self.update_preview().await;
                return Ok(());
            }
        }

        // Vim-style jump keys that do not need a config entry.
        let page = page_size();
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('d') => {
                    page_down(&mut self.list_state, self.display.len(), page);
                    if self.mode == AppMode::Visual {
                        self.mark_current();
                    }
                    self.update_preview().await;
                    return Ok(());
                }
                KeyCode::Char('u') => {
                    page_up(&mut self.list_state, page);
                    if self.mode == AppMode::Visual {
                        self.mark_current();
                    }
                    self.update_preview().await;
                    return Ok(());
                }
                _ => {}
            }
        }
        match key.code {
            KeyCode::PageDown => {
                page_down(&mut self.list_state, self.display.len(), page);
                if self.mode == AppMode::Visual {
                    self.mark_current();
                }
                self.update_preview().await;
                return Ok(());
            }
            KeyCode::PageUp => {
                page_up(&mut self.list_state, page);
                if self.mode == AppMode::Visual {
                    self.mark_current();
                }
                self.update_preview().await;
                return Ok(());
            }
            KeyCode::Home => {
                go_to_top(&mut self.list_state);
                if self.mode == AppMode::Visual {
                    self.mark_current();
                }
                self.update_preview().await;
                return Ok(());
            }
            KeyCode::End | KeyCode::Char('G') => {
                go_to_bottom(&mut self.list_state, self.display.len());
                if self.mode == AppMode::Visual {
                    self.mark_current();
                }
                self.update_preview().await;
                return Ok(());
            }
            KeyCode::Char(c @ '1'..='9') => {
                let index = (c as u8 - b'1') as usize;
                if index < self.tabs.len() {
                    self.switch_tab(index).await?;
                    return Ok(());
                }
            }
            _ => {}
        }

        if let Some(action) = self.key_mapper.get_action(key) {
            match action {
                Action::Quit => {
                    if self.config.behavior.confirm_quit {
                        self.mode = AppMode::ConfirmQuit;
                    } else {
                        self.should_quit = true;
                    }
                }
                Action::Up => {
                    previous(&mut self.list_state, self.display.len());
                    if self.mode == AppMode::Visual {
                        self.mark_current();
                    }
                    self.update_preview().await;
                }
                Action::Down => {
                    next(&mut self.list_state, self.display.len());
                    if self.mode == AppMode::Visual {
                        self.mark_current();
                    }
                    self.update_preview().await;
                }
                Action::Left => self.go_parent().await?,
                Action::Right | Action::Enter => self.enter().await?,
                Action::Backspace => self.go_parent().await?,
                Action::Delete => {
                    if self.config.behavior.confirm_delete {
                        self.mode = AppMode::ConfirmDelete;
                    } else {
                        self.delete_selected().await?;
                    }
                }
                Action::Rename => self.start_rename(),
                Action::NewFile => self.mode = AppMode::NewFile,
                Action::NewDir => self.mode = AppMode::NewDir,
                Action::Copy => self.copy_selected(),
                Action::Cut => self.cut_selected(),
                Action::Paste => self.start_paste().await?,
                Action::Search => {
                    self.marked.clear();
                    self.search_query.clear();
                    self.rebuild_display();
                    self.mode = AppMode::Search;
                }
                Action::ToggleHidden => {
                    self.show_hidden = !self.show_hidden;
                    self.refresh();
                }
                Action::TogglePreview => {
                    self.show_preview = !self.show_preview;
                    self.update_preview().await;
                }
                Action::ToggleGit => {
                    self.show_git = !self.show_git;
                    if self.show_git {
                        self.request_git_refresh();
                    } else {
                        self.git_root = None;
                        self.git_statuses.clear();
                        for entry in &mut self.entries {
                            entry.git_status = Default::default();
                        }
                        self.rebuild_display();
                    }
                }
                Action::Help => self.help_visible = !self.help_visible,
                Action::Refresh => self.refresh(),
                Action::GotoParent => self.go_parent().await?,
                Action::GotoHome => self.go_home().await?,
                Action::SortCycle => {
                    self.sort_by = match self.sort_by {
                        SortBy::Name => SortBy::Size,
                        SortBy::Size => SortBy::Modified,
                        SortBy::Modified => SortBy::Extension,
                        SortBy::Extension => SortBy::Name,
                    };
                    self.refresh();
                }
                Action::ReverseSort => {
                    self.reverse_sort = !self.reverse_sort;
                    self.refresh();
                }
                Action::NewTab => self.new_tab().await,
                Action::CloseTab => self.close_tab().await?,
                Action::NextTab => {
                    if self.tabs.len() > 1 {
                        let next = (self.active_tab + 1) % self.tabs.len();
                        self.switch_tab(next).await?;
                    }
                }
                Action::PrevTab => {
                    if self.tabs.len() > 1 {
                        let prev = if self.active_tab == 0 {
                            self.tabs.len() - 1
                        } else {
                            self.active_tab - 1
                        };
                        self.switch_tab(prev).await?;
                    }
                }
                Action::ToggleVisual => {
                    if self.mode == AppMode::Visual {
                        self.mode = AppMode::Normal;
                    } else {
                        self.mode = AppMode::Visual;
                        self.mark_current();
                    }
                }
                Action::ToggleMark => self.toggle_mark(),
                Action::BulkRename => self.pending_bulk_rename = true,
                Action::PreviewScrollUp => {
                    if self.preview_scroll > 0 {
                        self.preview_scroll = self.preview_scroll.saturating_sub(5);
                    }
                }
                Action::PreviewScrollDown => {
                    self.preview_scroll += 5;
                }
                Action::PreviewZoomIn => {
                    self.preview_zoom = (self.preview_zoom * 1.25).min(4.0);
                    self.update_preview().await;
                }
                Action::PreviewZoomOut => {
                    self.preview_zoom = (self.preview_zoom / 1.25).max(0.25);
                    self.update_preview().await;
                }
                Action::PreviewReset => {
                    self.preview_scroll = 0;
                    self.preview_zoom = 1.0;
                    self.update_preview().await;
                }
                Action::BookmarkAdd => {
                    if !self.bookmarks.contains(&self.current_path) {
                        self.bookmarks.push(self.current_path.clone());
                        self.notice = Some("Bookmark added".to_string());
                    }
                }
                Action::BookmarkJump => {
                    self.mode = AppMode::BookmarkPicker;
                    if self.bookmarks.is_empty() {
                        self.bookmark_state.select(None);
                    } else {
                        self.bookmark_state.select(Some(0));
                    }
                }
                Action::OpenTrash => {
                    self.mode = AppMode::TrashBrowser;
                    self.refresh_trash().await?;
                }
                Action::OpenWith => self.mode = AppMode::OpenWith,
                Action::QueueManager => {
                    self.mode = AppMode::QueueManager;
                    if self.queue.is_empty() {
                        self.queue_state.select(None);
                    } else {
                        self.queue_state.select(Some(0));
                    }
                }
                Action::ThemePicker => {
                    self.theme_index = crate::config::ColorConfig::picker_list()
                        .iter()
                        .position(|name| *name == self.config.ui.theme)
                        .unwrap_or(0);
                    self.mode = AppMode::ThemePicker;
                }
            }
        }
        Ok(())
    }

    async fn handle_search(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.search_query.clear();
                self.rebuild_display();
                self.update_preview().await;
            }
            crossterm::event::KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            crossterm::event::KeyCode::Backspace => {
                self.search_query.pop();
                self.rebuild_display();
                self.update_preview().await;
            }
            crossterm::event::KeyCode::Up => {
                previous(&mut self.list_state, self.display.len());
                self.update_preview().await;
            }
            crossterm::event::KeyCode::Down => {
                next(&mut self.list_state, self.display.len());
                self.update_preview().await;
            }
            crossterm::event::KeyCode::Char(c) => {
                self.search_query.push(c);
                self.rebuild_display();
                self.update_preview().await;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_rename(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.rename_buffer.clear();
            }
            crossterm::event::KeyCode::Enter => {
                if !self.rename_buffer.is_empty() {
                    self.rename_selected(&self.rename_buffer.clone()).await?;
                }
                self.mode = AppMode::Normal;
                self.rename_buffer.clear();
            }
            crossterm::event::KeyCode::Backspace => {
                self.rename_buffer.pop();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.rename_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_new_file(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.new_name_buffer.clear();
            }
            crossterm::event::KeyCode::Enter => {
                if !self.new_name_buffer.is_empty() {
                    self.create_file(&self.new_name_buffer.clone()).await?;
                }
                self.mode = AppMode::Normal;
                self.new_name_buffer.clear();
            }
            crossterm::event::KeyCode::Backspace => {
                self.new_name_buffer.pop();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.new_name_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_new_dir(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.new_name_buffer.clear();
            }
            crossterm::event::KeyCode::Enter => {
                if !self.new_name_buffer.is_empty() {
                    self.create_dir(&self.new_name_buffer.clone()).await?;
                }
                self.mode = AppMode::Normal;
                self.new_name_buffer.clear();
            }
            crossterm::event::KeyCode::Backspace => {
                self.new_name_buffer.pop();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.new_name_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_confirm_delete(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                self.delete_selected().await?;
                self.mode = AppMode::Normal;
            }
            _ => {
                self.mode = AppMode::Normal;
            }
        }
        Ok(())
    }

    async fn handle_confirm_quit(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                self.should_quit = true;
            }
            _ => {
                self.mode = AppMode::Normal;
            }
        }
        Ok(())
    }

    async fn handle_confirm_overwrite(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                self.mode = AppMode::Normal;
                self.paste(true).await?;
            }
            _ => {
                self.mode = AppMode::Normal;
            }
        }
        Ok(())
    }

    async fn handle_help(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Char('?') | crossterm::event::KeyCode::Esc => {
                self.help_visible = false;
            }
            crossterm::event::KeyCode::Char('q') => {
                self.should_quit = true;
            }
            _ => {}
        }
        Ok(())
    }

    /// Drop the current listing while a new directory is being read, so the
    /// old files are never shown under the new path.
    fn clear_listing(&mut self) {
        self.entries.clear();
        self.display.clear();
        self.parent_entries.clear();
        self.list_state.select(None);
        *self.list_state.offset_mut() = 0;
        self.preview_content = PreviewContent::none();
        self.preview_protocol = None;
    }

    async fn enter(&mut self) -> Result<()> {
        if let Some(entry) = self.selected_entry().cloned() {
            // Follow symlinks to directories when entering.
            if entry.path.is_dir() {
                self.clear_marks();
                self.current_path = entry.path;
                self.search_query.clear();
                self.clear_listing();
                self.refresh();
            } else if let Err(error) = opener::open(&entry.path) {
                self.notice = Some(format!("Could not open file: {error}"));
            }
        }
        Ok(())
    }

    async fn go_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.current_path.parent().map(Path::to_path_buf) {
            self.clear_marks();
            self.current_path = parent;
            self.search_query.clear();
            self.clear_listing();
            self.refresh();
        }
        Ok(())
    }

    async fn go_home(&mut self) -> Result<()> {
        if let Some(home) = dirs::home_dir() {
            self.clear_marks();
            self.current_path = home;
            self.search_query.clear();
            self.clear_listing();
            self.refresh();
        }
        Ok(())
    }

    fn start_rename(&mut self) {
        let entry = self.selected_entry().cloned();
                if let Some(entry) = entry {
            self.rename_buffer = entry.name.clone();
            self.mode = AppMode::Rename;
        }
    }

    async fn rename_selected(&mut self, new_name: &str) -> Result<()> {
        if let Some(entry) = self.selected_entry().cloned() {
            if new_name.is_empty() || new_name.contains('/') {
                self.notice = Some("That is not a valid name".to_string());
                return Ok(());
            }
            let new_path = entry.path.with_file_name(new_name);
            // Renaming over an existing file would throw its content away,
            // so it is refused instead.
            if new_path != entry.path && new_path.exists() {
                self.notice = Some(format!("\"{new_name}\" already exists"));
                return Ok(());
            }
            if let Err(error) = tokio::fs::rename(&entry.path, &new_path).await {
                self.notice = Some(format!("Rename failed: {error}"));
                return Ok(());
            }
            self.refresh();
        }
        Ok(())
    }

    async fn create_file(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        // create_new never truncates an existing file.
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(_) => {}
            Err(error) => {
                self.notice = Some(if error.kind() == std::io::ErrorKind::AlreadyExists {
                    format!("\"{name}\" already exists")
                } else {
                    format!("Could not create file: {error}")
                });
                return Ok(());
            }
        }
        self.refresh();
        Ok(())
    }

    async fn create_dir(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        if let Err(error) = tokio::fs::create_dir(&path).await {
            self.notice = Some(format!("Could not create directory: {error}"));
            return Ok(());
        }
        self.refresh();
        Ok(())
    }

    fn copy_selected(&mut self) {
        let paths = self.target_paths();
        if paths.is_empty() {
            return;
        }
        self.clipboard = ClipboardOp::Copy;
        self.clipboard_paths = paths;
        self.notice = Some(format!(
            "Yanked {} item(s) — p to paste, Q to view the queue",
            self.clipboard_paths.len()
        ));
        self.clear_marks();
    }

    fn cut_selected(&mut self) {
        let paths = self.target_paths();
        if paths.is_empty() {
            return;
        }
        self.clipboard = ClipboardOp::Cut;
        self.clipboard_paths = paths;
        self.notice = Some(format!(
            "Cut {} item(s) — p to move, Q to view the queue",
            self.clipboard_paths.len()
        ));
        self.clear_marks();
    }

    async fn start_paste(&mut self) -> Result<()> {
        if self.clipboard == ClipboardOp::None || self.clipboard_paths.is_empty() {
            self.notice = Some("Nothing to paste — yank with y or x first".to_string());
            return Ok(());
        }

        let mut conflicts = 0;
        for src in &self.clipboard_paths {
            let Some(file_name) = src.file_name() else {
                continue;
            };
            let dst = self.current_path.join(file_name);
            if dst == *src {
                self.notice = Some("Source and destination are the same file".to_string());
                return Ok(());
            }
            if dst.exists() {
                conflicts += 1;
            }
        }

        if conflicts > 0 {
            self.overwrite_count = conflicts;
            self.mode = AppMode::ConfirmOverwrite;
        } else {
            self.paste(false).await?;
        }
        Ok(())
    }

    /// Turn the clipboard into real transfer tasks. The tasks are queued and a
    /// worker starts immediately, so pasting a huge directory no longer freezes
    /// the interface. `Q` shows live progress.
    async fn paste(&mut self, overwrite: bool) -> Result<()> {
        let op = match self.clipboard {
            ClipboardOp::Copy => "copy",
            ClipboardOp::Cut => "move",
            ClipboardOp::None => return Ok(()),
        };

        let mut added = 0usize;
        for src in &self.clipboard_paths {
            let Some(file_name) = src.file_name() else {
                continue;
            };
            let dst = self.current_path.join(file_name);
            if dst == *src {
                self.notice = Some("Source and destination are the same file".to_string());
                continue;
            }
            self.queue.push(QueueItem {
                id: self.next_queue_id,
                op: op.to_string(),
                src: src.clone(),
                dst,
                status: QueueStatus::Pending,
                overwrite,
                error: None,
            });
            self.next_queue_id += 1;
            added += 1;
        }

        if added > 0 {
            self.notice = Some(format!(
                "Queued {added} {} task(s) — Q to watch progress",
                if op == "copy" { "copy" } else { "move" }
            ));
            self.clipboard = ClipboardOp::None;
            self.clipboard_paths.clear();
            self.marked.clear();
            self.start_queue();
        }
        save_queue(&self.queue);
        Ok(())
    }

    /// Spawn the background worker for every pending or failed task.
    fn start_queue(&mut self) {
        if self.queue_running {
            return;
        }

        let tasks: Vec<(u64, QueueItem)> = self
            .queue
            .iter()
            .filter(|item| matches!(item.status, QueueStatus::Pending | QueueStatus::Failed))
            .map(|item| (item.id, item.clone()))
            .collect();
        if tasks.is_empty() {
            return;
        }

        for (id, _) in &tasks {
            if let Some(item) = self.queue.iter_mut().find(|item| item.id == *id) {
                item.status = QueueStatus::Running;
                item.error = None;
            }
        }

        let (tx, rx) = mpsc::unbounded_channel();
        self.queue_rx = Some(rx);
        self.queue_running = true;

        tokio::spawn(async move {
            for (id, item) in tasks {
                let result = execute_queue_item(&item).await;
                let _ = tx.send(QueueEvent::Finished(id, result));
            }
            let _ = tx.send(QueueEvent::AllDone);
        });
    }

    async fn handle_queue_event(&mut self, event: QueueEvent) -> Result<()> {
        match event {
            QueueEvent::Finished(id, result) => {
                // Look the task up by id: the list may have been reordered or
                // shortened while the worker was busy.
                if let Some(item) = self.queue.iter_mut().find(|item| item.id == id) {
                    match result {
                        Ok(()) => {
                            item.status = QueueStatus::Done;
                            item.error = None;
                        }
                        Err(error) => {
                            item.status = QueueStatus::Failed;
                            item.error = Some(error);
                        }
                    }
                }
            }
            QueueEvent::AllDone => {
                self.queue_running = false;
                self.queue_rx = None;
                let done = self
                    .queue
                    .iter()
                    .filter(|item| item.status == QueueStatus::Done)
                    .count();
                let failed = self
                    .queue
                    .iter()
                    .filter(|item| item.status == QueueStatus::Failed)
                    .count();
                self.notice = Some(if failed == 0 {
                    format!("Queue finished: {done} task(s) completed")
                } else {
                    format!("Queue finished: {done} done, {failed} failed — Q to inspect")
                });
                save_queue(&self.queue);
                self.refresh();
            }
        }
        Ok(())
    }

    /// Hand every marked file name to $EDITOR, then apply the edited lines as
    /// renames. The terminal is restored to the shell while the editor runs.
    async fn bulk_rename(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let paths = self.target_paths();
        if paths.is_empty() {
            return Ok(());
        }

        let mut names = Vec::with_capacity(paths.len());
        for path in &paths {
            match path.file_name().and_then(|name| name.to_str()) {
                Some(name) if !name.contains('\n') => names.push(name.to_string()),
                _ => {
                    self.notice = Some("Bulk rename: unsupported file name".to_string());
                    return Ok(());
                }
            }
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let temp_path = std::env::temp_dir().join(format!(
            "rfn-rename-{}-{stamp}.txt",
            std::process::id()
        ));

        if let Err(error) = tokio::fs::write(&temp_path, format!("{}\n", names.join("\n"))).await
        {
            self.notice = Some(format!("Bulk rename: {error}"));
            return Ok(());
        }

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());
        let status = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!("exec {editor} \"$1\""))
            .arg("sh")
            .arg(&temp_path)
            .status()
            .await;

        enable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture
        )?;
        terminal.clear()?;

        let status = match status {
            Ok(status) => status,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                self.notice = Some(format!("Bulk rename: could not start editor: {error}"));
                return Ok(());
            }
        };

        if !status.success() {
            let _ = tokio::fs::remove_file(&temp_path).await;
            self.notice = Some(format!("Bulk rename: editor exited with {status}"));
            return Ok(());
        }

        let edited = match tokio::fs::read_to_string(&temp_path).await {
            Ok(text) => text,
            Err(error) => {
                self.notice = Some(format!("Bulk rename: {error}"));
                return Ok(());
            }
        };
        let _ = tokio::fs::remove_file(&temp_path).await;

        let new_names: Vec<&str> = edited.lines().collect();
        if new_names.len() != paths.len() {
            self.notice = Some(format!(
                "Bulk rename: expected {} lines, got {}",
                paths.len(),
                new_names.len()
            ));
            return Ok(());
        }

        for name in &new_names {
            if name.is_empty() || *name == "." || *name == ".." || name.contains('/') {
                self.notice = Some(format!("Bulk rename: invalid name \"{name}\""));
                return Ok(());
            }
        }

        let mut finals = HashSet::new();
        for name in &new_names {
            if !finals.insert(*name) {
                self.notice = Some(format!("Bulk rename: duplicate name \"{name}\""));
                return Ok(());
            }
        }

        let sources: HashSet<&Path> = paths.iter().map(PathBuf::as_path).collect();
        let mut changes: Vec<(PathBuf, PathBuf)> = Vec::new();
        for (source, new_name) in paths.iter().zip(&new_names) {
            let current_name = source.file_name().and_then(|name| name.to_str());
            if current_name == Some(*new_name) {
                continue;
            }
            let destination = source.with_file_name(new_name);
            if destination.exists() && !sources.contains(destination.as_path()) {
                self.notice = Some(format!("Bulk rename: \"{new_name}\" already exists"));
                return Ok(());
            }
            changes.push((source.clone(), destination));
        }

        if changes.is_empty() {
            self.notice = Some("Bulk rename: no names changed".to_string());
            return Ok(());
        }

        // Stage every source under a temporary name first so swaps and
        // rotations (a -> b, b -> a) do not collide.
        let mut staged: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::new();
        for (index, (source, destination)) in changes.iter().enumerate() {
            let temporary =
                source.with_file_name(format!(".rfn-rename-{}-{index}", std::process::id()));
            if let Err(error) = tokio::fs::rename(source, &temporary).await {
                for (temporary, source, _) in staged.iter().rev() {
                    let _ = tokio::fs::rename(temporary, source).await;
                }
                self.notice = Some(format!("Bulk rename failed: {error}"));
                self.refresh();
                return Ok(());
            }
            staged.push((temporary, source.clone(), destination.clone()));
        }

        let mut failure = None;
        for (index, (temporary, source, destination)) in staged.iter().enumerate() {
            if let Err(error) = tokio::fs::rename(temporary, destination).await {
                let _ = tokio::fs::rename(temporary, source).await;
                for (other_index, (temporary, source, destination)) in
                    staged.iter().enumerate().rev()
                {
                    if other_index < index {
                        let _ = tokio::fs::rename(destination, source).await;
                    } else if other_index > index {
                        let _ = tokio::fs::rename(temporary, source).await;
                    }
                }
                failure = Some(error);
                break;
            }
        }

        if let Some(error) = failure {
            self.notice = Some(format!("Bulk rename failed: {error}"));
        }

        self.clear_marks();
        self.refresh();
        Ok(())
    }

    async fn handle_trash(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        if key.code == KeyCode::Esc {
            self.mode = AppMode::Normal;
            self.trash_entries.clear();
            self.trash_items.clear();
            return Ok(());
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                previous(&mut self.trash_list_state, self.trash_entries.len());
            }
            KeyCode::Down | KeyCode::Char('j') => {
                next(&mut self.trash_list_state, self.trash_entries.len());
            }
            KeyCode::Home => {
                self.trash_list_state.select(Some(0));
                *self.trash_list_state.offset_mut() = 0;
            }
            KeyCode::End => {
                if !self.trash_entries.is_empty() {
                    self.trash_list_state.select(Some(self.trash_entries.len() - 1));
                }
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                // Jump to the folder the file was deleted from.
                if let Some(entry) = self
                    .trash_list_state
                    .selected()
                    .and_then(|index| self.trash_entries.get(index))
                {
                    let original = entry.path.clone();
                    match original.parent() {
                        Some(parent) if parent.is_dir() => {
                            self.mode = AppMode::Normal;
                            self.trash_entries.clear();
                            self.trash_items.clear();
                            self.clear_marks();
                            self.current_path = parent.to_path_buf();
                            self.search_query.clear();
                            self.clear_listing();
                            self.refresh();
                            return Ok(());
                        }
                        _ => {
                            self.notice = Some(format!(
                                "Deleted from {} (the folder is gone)",
                                original.display()
                            ));
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                let Some(item) = self.selected_trash_item().cloned() else {
                    return Ok(());
                };
                use trash::os_limited::restore_all;
                match restore_all(std::iter::once(item)) {
                    Ok(()) => {
                        self.notice = Some("Restored".to_string());
                        self.refresh_trash().await?;
                    }
                    Err(error) => {
                        self.notice = Some(format!("Restore failed: {error}"));
                    }
                }
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if self.selected_trash_item().is_some() {
                    self.mode = AppMode::ConfirmPurge;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Permanent deletion of a trash item. This cannot be undone, so it asks
    /// first.
    async fn handle_confirm_purge(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;
        use trash::os_limited::purge_all;

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.mode = AppMode::TrashBrowser;
                if let Some(item) = self.selected_trash_item().cloned() {
                    match purge_all(std::iter::once(item)) {
                        Ok(()) => {
                            self.notice = Some("Deleted permanently".to_string());
                        }
                        Err(error) => {
                            self.notice = Some(format!("Could not delete: {error}"));
                        }
                    }
                    self.refresh_trash().await?;
                }
            }
            _ => {
                self.mode = AppMode::TrashBrowser;
            }
        }
        Ok(())
    }

    async fn handle_open_with(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        if key.code == KeyCode::Esc {
            self.mode = AppMode::Normal;
            self.open_with_buffer.clear();
            return Ok(());
        }

        match key.code {
            KeyCode::Char(c) => {
                self.open_with_buffer.push(c);
            }
            KeyCode::Backspace => {
                self.open_with_buffer.pop();
            }
            KeyCode::Enter => {
                let entry = self.selected_entry().cloned();
                if let Some(entry) = entry {
                    let cmd = self.open_with_buffer.clone();
                    self.open_with_buffer.clear();
                    self.mode = AppMode::Normal;
                    disable_raw_mode()?;
                    execute!(
                        std::io::stdout(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    let status = tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!("exec {} \"{}\"", cmd, entry.path.display()))
                        .status()
                        .await;
                    enable_raw_mode()?;
                    execute!(
                        std::io::stdout(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;
                    if let Err(e) = status {
                        self.notice = Some(format!("Open with failed: {e}"));
                    }
                } else {
                    self.open_with_buffer.clear();
                    self.mode = AppMode::Normal;
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_queue(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        let len = self.queue.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let next = self
                        .queue_state
                        .selected()
                        .map(|index| (index + 1).min(len - 1))
                        .unwrap_or(0);
                    self.queue_state.select(Some(next));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
                    let previous = self
                        .queue_state
                        .selected()
                        .map(|index| index.saturating_sub(1))
                        .unwrap_or(0);
                    self.queue_state.select(Some(previous));
                }
            }
            KeyCode::Char('g') => {
                if len > 0 {
                    self.queue_state.select(Some(0));
                }
            }
            KeyCode::Char('G') => {
                if len > 0 {
                    self.queue_state.select(Some(len - 1));
                }
            }
            KeyCode::Char('r') => {
                if self.queue_running {
                    self.notice = Some("Queue is already running".to_string());
                } else if self
                    .queue
                    .iter()
                    .any(|item| matches!(item.status, QueueStatus::Pending | QueueStatus::Failed))
                {
                    self.start_queue();
                    self.notice = Some("Queue started".to_string());
                } else {
                    self.notice = Some("No pending tasks".to_string());
                }
            }
            KeyCode::Char('c') => {
                let before = self.queue.len();
                self.queue.retain(|item| item.status != QueueStatus::Done);
                self.notice = Some(format!(
                    "Cleared {} finished task(s)",
                    before - self.queue.len()
                ));
                self.fix_queue_selection();
                save_queue(&self.queue);
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.queue_state.selected() {
                    match self.queue.get(index) {
                        Some(item) if item.status == QueueStatus::Running => {
                            self.notice = Some("Cannot remove a running task".to_string());
                        }
                        Some(_) => {
                            self.queue.remove(index);
                            self.fix_queue_selection();
                            save_queue(&self.queue);
                        }
                        None => {}
                    }
                }
            }
            KeyCode::Char('u') => {
                if let Some(index) = self.queue_state.selected() {
                    if index > 0 && index < self.queue.len() {
                        self.queue.swap(index, index - 1);
                        self.queue_state.select(Some(index - 1));
                        save_queue(&self.queue);
                    }
                }
            }
            KeyCode::Char('o') => {
                if let Some(index) = self.queue_state.selected() {
                    if index + 1 < self.queue.len() {
                        self.queue.swap(index, index + 1);
                        self.queue_state.select(Some(index + 1));
                        save_queue(&self.queue);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn fix_queue_selection(&mut self) {
        if self.queue.is_empty() {
            self.queue_state.select(None);
            *self.queue_state.offset_mut() = 0;
        } else {
            let selected = self
                .queue_state
                .selected()
                .unwrap_or(0)
                .min(self.queue.len() - 1);
            self.queue_state.select(Some(selected));
        }
    }

    /// Bookmark picker: j/k to move, Enter to jump, a to add, d to remove.
    async fn handle_bookmark_picker(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::KeyCode;

        let len = self.bookmarks.len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let next = self
                        .bookmark_state
                        .selected()
                        .map(|index| (index + 1).min(len - 1))
                        .unwrap_or(0);
                    self.bookmark_state.select(Some(next));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
                    let previous = self
                        .bookmark_state
                        .selected()
                        .map(|index| index.saturating_sub(1))
                        .unwrap_or(0);
                    self.bookmark_state.select(Some(previous));
                }
            }
            KeyCode::Home => {
                if len > 0 {
                    self.bookmark_state.select(Some(0));
                }
            }
            KeyCode::End => {
                if len > 0 {
                    self.bookmark_state.select(Some(len - 1));
                }
            }
            KeyCode::Char('a') => {
                if !self.bookmarks.contains(&self.current_path) {
                    self.bookmarks.push(self.current_path.clone());
                    self.notice = Some("Bookmark added".to_string());
                } else {
                    self.notice = Some("Already bookmarked".to_string());
                }
                self.fix_bookmark_selection();
            }
            KeyCode::Char('d') => {
                if let Some(index) = self.bookmark_state.selected() {
                    if index < self.bookmarks.len() {
                        self.bookmarks.remove(index);
                        self.notice = Some("Bookmark removed".to_string());
                    }
                }
                self.fix_bookmark_selection();
            }
            KeyCode::Enter => {
                if let Some(index) = self.bookmark_state.selected() {
                    if let Some(path) = self.bookmarks.get(index).cloned() {
                        self.mode = AppMode::Normal;
                        self.clear_marks();
                        self.current_path = path;
                        self.search_query.clear();
                        self.clear_listing();
                        self.refresh();
                        return Ok(());
                    }
                }
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        Ok(())
    }

    fn fix_bookmark_selection(&mut self) {
        if self.bookmarks.is_empty() {
            self.bookmark_state.select(None);
            *self.bookmark_state.offset_mut() = 0;
        } else {
            let selected = self
                .bookmark_state
                .selected()
                .unwrap_or(0)
                .min(self.bookmarks.len() - 1);
            self.bookmark_state.select(Some(selected));
        }
    }

    /// Theme picker: j/k to move, Enter to apply, Esc to cancel.
    fn handle_theme_picker(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        let count = crate::config::ColorConfig::picker_list().len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
            KeyCode::Up | KeyCode::Char('k') => {
                self.theme_index = self.theme_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.theme_index = (self.theme_index + 1).min(count.saturating_sub(1));
            }
            KeyCode::Home => self.theme_index = 0,
            KeyCode::End => self.theme_index = count.saturating_sub(1),
            KeyCode::Enter => {
                if let Some(name) = crate::config::ColorConfig::picker_list()
                    .get(self.theme_index)
                {
                    self.apply_theme(name);
                }
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
    }

    /// Switch theme immediately and remember the choice for next launch.
    fn apply_theme(&mut self, name: &str) {
        if name == crate::config::ColorConfig::CUSTOM {
            // The colors of the user stay as they are.
            self.config.ui.theme_colors = None;
        } else {
            let Some(colors) = crate::config::ColorConfig::preset(name) else {
                return;
            };
            self.config.ui.theme_colors = Some(colors);
        }
        self.config.ui.theme = name.to_string();

        match crate::config::AppConfig::save_theme(name) {
            Ok(path) => self.notice = Some(format!(
                "Theme: {name} (saved to {})",
                path.display()
            )),
            Err(error) => self.notice = Some(format!("Theme: {name} (not saved: {error})")),
        }
    }

    async fn delete_selected(&mut self) -> Result<()> {
        let paths = self.target_paths();
        if paths.is_empty() {
            return Ok(());
        }

        let mut failures = 0;
        for path in &paths {
            let result = if self.config.behavior.use_trash {
                trash::delete(path).map_err(anyhow::Error::from)
            } else if path.is_dir() {
                tokio::fs::remove_dir_all(path)
                    .await
                    .map_err(anyhow::Error::from)
            } else {
                tokio::fs::remove_file(path)
                    .await
                    .map_err(anyhow::Error::from)
            };
            if result.is_err() {
                failures += 1;
            }
        }

        if failures > 0 {
            self.notice = Some(format!(
                "{failures} of {} item(s) could not be deleted",
                paths.len()
            ));
        }

        self.clear_marks();
        self.refresh();
        Ok(())
    }
}

fn page_size() -> usize {
    crossterm::terminal::size()
        .map(|(_, height)| height.saturating_sub(5) as usize)
        .unwrap_or(15)
        .max(1)
}

fn dir_title(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if path == home {
            return "~".to_string();
        }
    }
    if path.parent().is_none() {
        return "/".to_string();
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// Scale an image for the preview. Zooming in makes the picture bigger so
/// details are visible; zooming out fits the whole picture in the pane.
fn zoom_image(img: image::DynamicImage, zoom: f32) -> image::DynamicImage {
    if (zoom - 1.0).abs() < 0.01 {
        return img;
    }

    const MAX_DIMENSION: u32 = 4096;
    let width = ((img.width() as f32 * zoom).round() as u32).max(1);
    let height = ((img.height() as f32 * zoom).round() as u32).max(1);
    let (width, height) = if width > MAX_DIMENSION || height > MAX_DIMENSION {
        let scale = MAX_DIMENSION as f32 / width.max(height) as f32;
        (
            ((width as f32 * scale) as u32).max(1),
            ((height as f32 * scale) as u32).max(1),
        )
    } else {
        (width, height)
    };

    img.resize(width, height, image::imageops::FilterType::Triangle)
}

async fn execute_queue_item(item: &QueueItem) -> std::result::Result<(), String> {
    if item.src == item.dst {
        return Err("source and destination are identical".to_string());
    }
    if !item.src.exists() {
        return Err("source no longer exists".to_string());
    }
    if item.dst.exists() && !item.overwrite {
        return Err(format!(
            "destination already exists: {}",
            item.dst.display()
        ));
    }

    match item.op.as_str() {
        "copy" => copy_over(item).await,
        "move" => move_over(item).await,
        other => Err(format!("unknown operation: {other}")),
    }
}

/// Copy to a scratch name first and only then put it in place. A copy that
/// fails halfway through can therefore never destroy the file it replaces.
async fn copy_over(item: &QueueItem) -> std::result::Result<(), String> {
    let temp = temp_destination(&item.dst);

    if let Err(error) = copy_path(&item.src, &temp).await {
        let _ = remove_path(&temp).await;
        return Err(error);
    }

    if item.dst.exists() {
        remove_path(&item.dst)
            .await
            .map_err(|error| format!("cannot replace destination: {error}"))?;
    }

    if let Err(error) = tokio::fs::rename(&temp, &item.dst).await {
        let _ = remove_path(&temp).await;
        return Err(format!("cannot move the copy into place: {error}"));
    }
    Ok(())
}

/// Move the source. Renaming replaces a file in one step. A directory has to
/// be swapped out, so the new one is built next to it first and only then put
/// in place. Across filesystems the move falls back to copy + delete.
async fn move_over(item: &QueueItem) -> std::result::Result<(), String> {
    let destination_is_dir = item.dst.exists() && item.dst.is_dir();

    if destination_is_dir {
        // A rename cannot replace a directory, so the new folder is built
        // under a scratch name first. The old one stays untouched until the
        // copy is complete.
        let temp = temp_destination(&item.dst);
        if let Err(error) = copy_path(&item.src, &temp).await {
            let _ = remove_path(&temp).await;
            return Err(error);
        }
        remove_path(&item.dst)
            .await
            .map_err(|error| format!("cannot replace destination: {error}"))?;
        if let Err(error) = tokio::fs::rename(&temp, &item.dst).await {
            let _ = remove_path(&temp).await;
            return Err(format!("cannot move the copy into place: {error}"));
        }
        remove_path(&item.src)
            .await
            .map_err(|error| format!("copied but could not remove source: {error}"))
    } else {
        match tokio::fs::rename(&item.src, &item.dst).await {
            // Renaming over a file replaces it in one step.
            Ok(()) => Ok(()),
            Err(_) => {
                // `rename` fails across filesystems (EXDEV). Fall back to a
                // real copy followed by removing the source.
                let temp = temp_destination(&item.dst);
                if let Err(error) = copy_path(&item.src, &temp).await {
                    let _ = remove_path(&temp).await;
                    return Err(error);
                }
                if item.dst.exists() {
                    remove_path(&item.dst)
                        .await
                        .map_err(|error| format!("cannot replace destination: {error}"))?;
                }
                if let Err(error) = tokio::fs::rename(&temp, &item.dst).await {
                    let _ = remove_path(&temp).await;
                    return Err(format!("cannot move the copy into place: {error}"));
                }
                remove_path(&item.src)
                    .await
                    .map_err(|error| format!("copied but could not remove source: {error}"))
            }
        }
    }
}

/// A scratch name next to the destination, used while a copy is in flight.
fn temp_destination(dst: &Path) -> PathBuf {
    let name = dst
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "task".to_string());
    let temp_name = format!(".{name}.rfn-part");
    match dst.parent() {
        Some(parent) => parent.join(temp_name),
        None => PathBuf::from(temp_name),
    }
}

/// Remove a file, a directory or a symlink (symlinks are never followed).
async fn remove_path(path: &Path) -> std::io::Result<()> {
    let meta = tokio::fs::symlink_metadata(path).await?;
    if meta.is_dir() {
        tokio::fs::remove_dir_all(path).await
    } else {
        tokio::fs::remove_file(path).await
    }
}

async fn copy_path(src: &Path, dst: &Path) -> std::result::Result<(), String> {
    if src.is_dir() {
        copy_dir(src, dst).await.map_err(|error| error.to_string())
    } else {
        tokio::fs::copy(src, dst)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}

async fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    use std::collections::VecDeque;
    let mut queue = VecDeque::new();
    queue.push_back((src.to_path_buf(), dst.to_path_buf()));

    while let Some((src, dst)) = queue.pop_front() {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(&src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            let ft = entry.file_type().await?;
            if ft.is_dir() {
                queue.push_back((src_path, dst_path));
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
    }
    Ok(())
}
