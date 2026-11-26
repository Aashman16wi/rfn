use crate::config::{AppConfig, SortBy};
use crate::fs::{apply_git_status, find_git_root, get_git_status, read_dir, sort_entries};
use crate::keys::{Action, KeyMapper};
use crate::preview::generate_preview;
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
    search_query: String,
    rename_buffer: String,
    new_name_buffer: String,

    clipboard: ClipboardOp,
    clipboard_paths: Vec<PathBuf>,
    marked: HashSet<PathBuf>,

    show_hidden: bool,
    show_preview: bool,
    show_git: bool,
    sort_by: SortBy,
    reverse_sort: bool,

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
            search_query: String::new(),
            rename_buffer: String::new(),
            new_name_buffer: String::new(),
            clipboard: ClipboardOp::None,
            clipboard_paths: Vec::new(),
            marked: HashSet::new(),
            show_hidden,
            show_preview,
            show_git,
            sort_by,
            reverse_sort,
            help_visible: false,
            notice: None,
            pending_bulk_rename: false,
            should_quit: false,
            last_tick: std::time::Instant::now(),
        };

        app.refresh().await?;
        app.tabs.push(app.snapshot());

        if restore_tabs {
            if let Some(saved) = crate::state::load() {
                let mut restored = Vec::new();
                for path in &saved.tabs {
                    if path.is_dir() {
                        app.current_path = path.clone();
                        app.refresh().await?;
                        restored.push(app.snapshot());
                    }
                }
                if !restored.is_empty() {
                    app.tabs = restored;
                    app.active_tab = saved.active.min(app.tabs.len() - 1);
                    app.restore_active_tab();
                    app.refresh().await?;
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
                        self.handle_event(event).await?;
                        if self.pending_bulk_rename {
                            self.pending_bulk_rename = false;
                            self.bulk_rename(terminal).await?;
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

        Ok(())
    }

    async fn read_event(&self) -> Result<Option<Event>> {
        loop {
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
            return Ok(Some(event));
        }
    }

    async fn tick(&mut self) -> Result<bool> {
        if self.last_tick.elapsed() < Duration::from_secs(2) {
            return Ok(false);
        }

        let before: Vec<_> = self.display.iter().map(|entry| entry.git_status).collect();
        self.refresh_git().await?;
        self.rebuild_display();
        let after: Vec<_> = self.display.iter().map(|entry| entry.git_status).collect();
        self.last_tick = std::time::Instant::now();

        Ok(before != after)
    }

    async fn refresh(&mut self) -> Result<()> {
        if self.mode == AppMode::TrashBrowser {
            self.refresh_trash().await?;
            return Ok(());
        }
        match read_dir(&self.current_path, self.show_hidden).await {
            Ok(entries) => self.entries = entries,
            Err(error) => {
                self.notice = Some(format!(
                    "Cannot read {}: {error}",
                    self.current_path.display()
                ));
                self.entries.clear();
            }
        }
        sort_entries(&mut self.entries, self.sort_by, self.config.behavior.sort_dirs_first, self.reverse_sort);

        self.parent_entries = match self.current_path.parent() {
            Some(parent) => {
                let mut entries = read_dir(parent, self.show_hidden).await.unwrap_or_default();
                sort_entries(
                    &mut entries,
                    self.sort_by,
                    self.config.behavior.sort_dirs_first,
                    self.reverse_sort,
                );
                entries
            }
            None => Vec::new(),
        };

        self.refresh_git().await?;
        self.rebuild_display();
        self.update_preview().await;

        Ok(())
    }

    async fn refresh_trash(&mut self) -> Result<()> {
        use trash::os_limited::{list, restore_all};
        self.trash_entries.clear();
        if let Ok(items) = list() {
            for item in items {
                let original_path = item.original_path();
                if let Ok(meta) = std::fs::metadata(&original_path) {
                    let name = original_path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.trash_entries.push(FileEntry {
                        path: original_path,
                        name,
                        file_type: if meta.is_dir() { crate::types::FileType::Directory } else { crate::types::FileType::File },
                        size: meta.len(),
                        modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                        permissions: 0,
                        git_status: crate::types::GitStatus::Unmodified,
                        mime_type: None,
                        is_hidden: false,
                    });
                }
            }
        }
        sort_entries(&mut self.trash_entries, SortBy::Modified, false, true);
        self.trash_list_state.select(Some(0));
        Ok(())
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

    async fn refresh_git(&mut self) -> Result<()> {
        if !self.show_git {
            return Ok(());
        }

        if let Some(root) = find_git_root(&self.current_path) {
            if self.git_root.as_ref() != Some(&root) {
                self.git_root = Some(root.clone());
                self.git_statuses = get_git_status(&root).await?;
            }
            apply_git_status(&mut self.entries, &self.git_statuses);
        } else {
            self.git_root = None;
            self.git_statuses.clear();
            for entry in &mut self.entries {
                entry.git_status = Default::default();
            }
        }
        Ok(())
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

        self.preview_content = match generate_preview(&entry, max_size, show_images).await {
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
        self.refresh().await?;
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
        self.refresh().await?;
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
        let colors = &self.config.ui.colors;

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
        let areas = crate::ui::layout(
            area,
            self.show_preview,
            self.config.ui.show_parent,
            show_tabs,
        );

        let current_title = dir_title(&self.current_path);

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
            self.display.len(),
            colors,
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
        );

        // Preview
        if let Some(preview_area) = areas.preview {
            crate::ui::preview::render(
                frame,
                preview_area,
                &self.preview_content,
                self.preview_protocol.as_mut(),
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
            colors,
        );

        // Typing prompt replaces the status bar; confirmations float on top.
        match self.mode {
            AppMode::Search => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.search_query,
                &self.config.ui.colors,
            ),
            AppMode::Rename => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.rename_buffer,
                &self.config.ui.colors,
            ),
            AppMode::NewFile | AppMode::NewDir => crate::ui::prompt::render_input(
                frame,
                status_area,
                self.mode,
                &self.new_name_buffer,
                &self.config.ui.colors,
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
                    &self.config.ui.colors,
                );
            }
            AppMode::ConfirmQuit => {
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    " Quit ",
                    "Quit rfn?",
                    &self.config.ui.colors,
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
                    &self.config.ui.colors,
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
                    AppMode::Visual => self.handle_normal(key).await?,
                    AppMode::TrashBrowser => self.handle_trash(key).await?,
                    AppMode::OpenWith => self.handle_open_with(key).await?,
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
                    self.refresh().await?;
                }
                Action::TogglePreview => {
                    self.show_preview = !self.show_preview;
                    self.update_preview().await;
                }
                Action::ToggleGit => {
                    self.show_git = !self.show_git;
                    if self.show_git {
                        self.refresh_git().await?;
                        self.rebuild_display();
                    }
                }
                Action::Help => self.help_visible = !self.help_visible,
                Action::Refresh => self.refresh().await?,
                Action::GotoParent => self.go_parent().await?,
                Action::GotoHome => self.go_home().await?,
                Action::SortCycle => {
                    self.sort_by = match self.sort_by {
                        SortBy::Name => SortBy::Size,
                        SortBy::Size => SortBy::Modified,
                        SortBy::Modified => SortBy::Extension,
                        SortBy::Extension => SortBy::Name,
                    };
                    self.refresh().await?;
                }
                Action::ReverseSort => {
                    self.reverse_sort = !self.reverse_sort;
                    self.refresh().await?;
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
                    self.preview_zoom = (self.preview_zoom * 1.2).min(4.0);
                }
                Action::PreviewZoomOut => {
                    self.preview_zoom = (self.preview_zoom / 1.2).max(0.25);
                }
                Action::PreviewReset => {
                    self.preview_scroll = 0;
                    self.preview_zoom = 1.0;
                }
                Action::BookmarkAdd => {
                    if !self.bookmarks.contains(&self.current_path) {
                        self.bookmarks.push(self.current_path.clone());
                        self.notice = Some("Bookmark added".to_string());
                    }
                }
                Action::BookmarkJump => {
                    if !self.bookmarks.is_empty() {
                        self.current_path = self.bookmarks[0].clone();
                        self.refresh().await?;
                    }
                }
                Action::OpenTrash => self.mode = AppMode::TrashBrowser,
                Action::OpenWith => self.mode = AppMode::OpenWith,
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

    async fn enter(&mut self) -> Result<()> {
        if let Some(entry) = self.selected_entry().cloned() {
            // Follow symlinks to directories when entering.
            if entry.path.is_dir() {
                self.clear_marks();
                self.current_path = entry.path;
                self.search_query.clear();
                self.list_state.select(Some(0));
                *self.list_state.offset_mut() = 0;
                self.refresh().await?;
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
            self.list_state.select(Some(0));
            *self.list_state.offset_mut() = 0;
            self.refresh().await?;
        }
        Ok(())
    }

    async fn go_home(&mut self) -> Result<()> {
        if let Some(home) = dirs::home_dir() {
            self.clear_marks();
            self.current_path = home;
            self.search_query.clear();
            self.list_state.select(Some(0));
            *self.list_state.offset_mut() = 0;
            self.refresh().await?;
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
            let new_path = entry.path.with_file_name(new_name);
            if let Err(error) = tokio::fs::rename(&entry.path, &new_path).await {
                self.notice = Some(format!("Rename failed: {error}"));
                return Ok(());
            }
            self.refresh().await?;
        }
        Ok(())
    }

    async fn create_file(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        if let Err(error) = tokio::fs::write(&path, "").await {
            self.notice = Some(format!("Could not create file: {error}"));
            return Ok(());
        }
        self.refresh().await?;
        Ok(())
    }

    async fn create_dir(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        if let Err(error) = tokio::fs::create_dir(&path).await {
            self.notice = Some(format!("Could not create directory: {error}"));
            return Ok(());
        }
        self.refresh().await?;
        Ok(())
    }

    fn copy_selected(&mut self) {
        let paths = self.target_paths();
        if !paths.is_empty() {
            self.clipboard = ClipboardOp::Copy;
            self.clipboard_paths = paths;
        }
        self.clear_marks();
    }

    fn cut_selected(&mut self) {
        let paths = self.target_paths();
        if !paths.is_empty() {
            self.clipboard = ClipboardOp::Cut;
            self.clipboard_paths = paths;
        }
        self.clear_marks();
    }

    async fn start_paste(&mut self) -> Result<()> {
        if self.clipboard == ClipboardOp::None || self.clipboard_paths.is_empty() {
            return Ok(());
        }

        let mut conflicts = 0;
        for src in &self.clipboard_paths {
            let Some(file_name) = src.file_name() else {
                continue;
            };
            let dst = self.current_path.join(file_name);
            if dst == *src {
                self.notice =
                    Some("Source and destination are the same file".to_string());
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

    async fn paste(&mut self, overwrite: bool) -> Result<()> {
        let mut failed = false;
        for src in &self.clipboard_paths {
            let Some(file_name) = src.file_name() else {
                continue;
            };
            let dst = self.current_path.join(file_name);

            if dst == *src {
                self.notice =
                    Some("Source and destination are the same file".to_string());
                failed = true;
                break;
            }

            if overwrite && dst.exists() {
                let removal = if dst.is_dir() {
                    tokio::fs::remove_dir_all(&dst).await
                } else {
                    tokio::fs::remove_file(&dst).await
                };
                if let Err(error) = removal {
                    self.notice = Some(format!("Overwrite failed: {error}"));
                    failed = true;
                    break;
                }
            }

            let result = match self.clipboard {
                ClipboardOp::Copy => {
                    if src.is_dir() {
                        copy_dir(src, &dst).await
                    } else {
                        tokio::fs::copy(src, &dst)
                            .await
                            .map(|_| ())
                            .map_err(anyhow::Error::from)
                    }
                }
                ClipboardOp::Cut => tokio::fs::rename(src, &dst)
                    .await
                    .map_err(anyhow::Error::from),
                ClipboardOp::None => Ok(()),
            };

            if let Err(error) = result {
                self.notice = Some(format!("Paste failed: {error}"));
                failed = true;
                break;
            }
        }

        if !failed {
            self.clipboard = ClipboardOp::None;
            self.clipboard_paths.clear();
            self.marked.clear();
        }
        self.refresh().await?;
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
                self.refresh().await?;
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
        self.refresh().await?;
        Ok(())
    }

    async fn handle_trash(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};
        use trash::os_limited::{list, restore_all};

        if key.code == KeyCode::Esc {
            self.mode = AppMode::Normal;
            self.trash_entries.clear();
            return Ok(());
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                previous(&mut self.trash_list_state, self.trash_entries.len());
            }
            KeyCode::Down | KeyCode::Char('j') => {
                next(&mut self.trash_list_state, self.trash_entries.len());
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if let Some(idx) = self.trash_list_state.selected() {
                    if let Some(entry) = self.trash_entries.get(idx) {
                        if entry.path.is_dir() {
                            self.current_path = entry.path.clone();
                            self.mode = AppMode::Normal;
                            self.trash_entries.clear();
                            self.refresh().await?;
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Some(idx) = self.trash_list_state.selected() {
                    if let Some(entry) = self.trash_entries.get(idx) {
                        // Need to get the TrashItem from the list to restore
                        if let Ok(items) = list() {
                            if let Some(trash_item) = items.get(idx) {
                                if let Err(e) = restore_all(std::iter::once(trash_item.clone())) {
                                    self.notice = Some(format!("Restore failed: {e}"));
                                } else {
                                    self.notice = Some("Restored".to_string());
                                    self.refresh_trash().await?;
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Delete | KeyCode::Char('d') => {
                if let Some(idx) = self.trash_list_state.selected() {
                    if let Some(entry) = self.trash_entries.get(idx) {
                        if entry.path.is_dir() {
                            let _ = tokio::fs::remove_dir_all(&entry.path).await;
                        } else {
                            let _ = tokio::fs::remove_file(&entry.path).await;
                        }
                        self.refresh_trash().await?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_open_with(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        use crossterm::event::{KeyCode, KeyModifiers};

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
        self.refresh().await?;
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
