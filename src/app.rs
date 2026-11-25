use crate::config::{AppConfig, SortBy};
use crate::fs::{apply_git_status, find_git_root, get_git_status, read_dir, sort_entries};
use crate::keys::{Action, KeyMapper};
use crate::preview::generate_preview;
use crate::types::{AppMode, ClipboardOp, FileEntry, PreviewContent};
use crate::ui::file_list::{go_to_bottom, go_to_top, next, page_down, page_up, previous};
use crate::ui::{layout};
use crate::ui::help::render as render_help;
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::interval;

pub struct App {
    config: AppConfig,
    key_mapper: KeyMapper,

    current_path: PathBuf,
    entries: Vec<FileEntry>,
    list_state: ratatui::widgets::ListState,

    preview_content: PreviewContent,

    git_statuses: Vec<(PathBuf, crate::types::GitStatus)>,
    git_root: Option<PathBuf>,

    mode: AppMode,
    search_query: String,
    rename_buffer: String,
    new_name_buffer: String,

    clipboard: ClipboardOp,
    clipboard_paths: Vec<PathBuf>,

    show_hidden: bool,
    show_preview: bool,
    show_git: bool,
    sort_by: SortBy,
    reverse_sort: bool,

    help_visible: bool,

    should_quit: bool,
    last_tick: std::time::Instant,
}

impl App {
    pub async fn new(config: AppConfig, start_path: std::path::PathBuf) -> Result<Self> {
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
            current_path,
            entries: Vec::new(),
            list_state: ratatui::widgets::ListState::default(),
            preview_content: PreviewContent::none(),
            git_statuses: Vec::new(),
            git_root: None,
            mode: AppMode::Normal,
            search_query: String::new(),
            rename_buffer: String::new(),
            new_name_buffer: String::new(),
            clipboard: ClipboardOp::None,
            clipboard_paths: Vec::new(),
            show_hidden,
            show_preview,
            show_git,
            sort_by,
            reverse_sort,
            help_visible: false,
            should_quit: false,
            last_tick: std::time::Instant::now(),
        };

        app.refresh().await?;
        Ok(app)
    }

    pub async fn run(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let mut tick_interval = interval(Duration::from_millis(100));

        loop {
            terminal.draw(|f| self.render(f))?;

            tokio::select! {
                _ = tick_interval.tick() => {
                    self.tick().await?;
                }
                event = self.read_event() => {
                    if let Some(event) = event? {
                        self.handle_event(event).await?;
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    async fn read_event(&self) -> Result<Option<Event>> {
        if crossterm::event::poll(Duration::from_millis(50))? {
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    }

    async fn tick(&mut self) -> Result<()> {
        if self.last_tick.elapsed() > Duration::from_secs(2) {
            self.refresh_git().await?;
            self.last_tick = std::time::Instant::now();
        }
        Ok(())
    }

    async fn refresh(&mut self) -> Result<()> {
        self.entries = read_dir(&self.current_path, self.show_hidden).await?;
        sort_entries(&mut self.entries, self.sort_by, self.config.behavior.sort_dirs_first, self.reverse_sort);

        if self.entries.is_empty() {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() || self.list_state.selected().unwrap() >= self.entries.len() {
            self.list_state.select(Some(0));
        }

        self.refresh_git().await?;
        self.update_preview().await;

        Ok(())
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
        if !self.show_preview {
            self.preview_content = PreviewContent::none();
            return;
        }

        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                let entry = entry.clone();
                let max_size = self.config.preview.max_size;
                let show_images = self.config.preview.show_images;

                match generate_preview(&entry, max_size, show_images).await {
                    Ok(content) => self.preview_content = content,
                    Err(_) => self.preview_content = PreviewContent::none(),
                }
            }
        } else {
            self.preview_content = PreviewContent::none();
        }
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.size();

        if self.help_visible {
            render_help(frame, area, &self.config.keys);
            return;
        }

        let (list_area, preview_area, status_area) =
            crate::ui::layout(area, self.show_preview, false);

        // Filter entries for search
        let filtered_entries: Vec<FileEntry> = if self.search_query.is_empty() {
            self.entries.clone()
        } else {
            let query = self.search_query.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&query))
                .cloned()
                .collect()
        };

        // File list
        crate::ui::file_list::render(
            frame,
            list_area,
            &filtered_entries,
            &mut self.list_state,
            &self.config.ui.colors,
            self.config.ui.show_icons,
            self.show_git,
        );

        // Preview
        if self.show_preview {
            crate::ui::preview::render(
                frame,
                preview_area,
                &self.preview_content,
                &self.config.ui.colors,
            );
        }

        // Status bar
        let filtered_entries: Vec<&FileEntry> = if self.search_query.is_empty() {
            self.entries.iter().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&query))
                .collect()
        };
        let selected_entry = self.list_state.selected().and_then(|i| filtered_entries.get(i).copied());
        crate::ui::status_bar::render(
            frame,
            status_area,
            self.mode,
            &self.current_path,
            selected_entry,
            self.clipboard,
            self.clipboard_paths.len(),
            self.show_hidden,
            self.show_preview,
            self.show_git,
            self.sort_by,
            self.reverse_sort,
            &self.config.ui.colors,
        );

        
    }

    async fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
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
                    AppMode::Help => self.handle_help(key).await?,
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
                // Click to select item
                // This would need the render area to calculate which item was clicked
            }
            MouseEventKind::ScrollUp => {
                previous(&mut self.list_state, self.entries.len());
            }
            MouseEventKind::ScrollDown => {
                next(&mut self.list_state, self.entries.len());
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_normal(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if let Some(action) = self.key_mapper.get_action(key) {
            match action {
                Action::Quit => {
                    if self.config.behavior.confirm_quit {
                        self.mode = AppMode::ConfirmQuit;
                    } else {
                        self.should_quit = true;
                    }
                }
                Action::Up => previous(&mut self.list_state, self.entries.len()),
                Action::Down => next(&mut self.list_state, self.entries.len()),
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
                Action::Paste => self.paste().await?,
                Action::Search => self.mode = AppMode::Search,
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
                    }
                }
                Action::Help => self.help_visible = true,
                Action::Refresh => self.refresh().await?,
                Action::GotoParent => self.go_parent().await?,
                Action::GotoHome => self.go_home().await?,
                Action::SortName => { self.sort_by = SortBy::Name; self.refresh().await?; }
                Action::SortSize => { self.sort_by = SortBy::Size; self.refresh().await?; }
                Action::SortTime => { self.sort_by = SortBy::Modified; self.refresh().await?; }
                Action::SortExt => { self.sort_by = SortBy::Extension; self.refresh().await?; }
                Action::ReverseSort => { self.reverse_sort = !self.reverse_sort; self.refresh().await?; }
            }
        }
        Ok(())
    }

    async fn handle_search(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Esc => {
                self.mode = AppMode::Normal;
                self.search_query.clear();
            }
            crossterm::event::KeyCode::Enter => {
                self.mode = AppMode::Normal;
            }
            crossterm::event::KeyCode::Backspace => {
                self.search_query.pop();
                self.filter_entries();
            }
            crossterm::event::KeyCode::Char(c) => {
                self.search_query.push(c);
                self.filter_entries();
            }
            _ => {}
        }
        Ok(())
    }

    fn filter_entries(&mut self) {
        let query = self.search_query.to_lowercase();
        if query.is_empty() {
            return;
        }
        // Filter is applied in render by checking the search query
        // We keep all entries but the list rendering will need to filter
    }

    fn get_filtered_entries(&self) -> Vec<&FileEntry> {
        if self.search_query.is_empty() {
            self.entries.iter().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&query))
                .collect()
        }
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

    async fn handle_help(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            crossterm::event::KeyCode::Char('?') | crossterm::event::KeyCode::Esc => {
                self.help_visible = false;
            }
            _ => {}
        }
        Ok(())
    }

    async fn enter(&mut self) -> Result<()> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                if entry.is_dir() {
                    self.current_path = entry.path.clone();
                    self.refresh().await?;
                } else {
                    // Open file with default application
                    let _ = opener::open(&entry.path);
                }
            }
        }
        Ok(())
    }

    async fn go_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.refresh().await?;
        }
        Ok(())
    }

    async fn go_home(&mut self) -> Result<()> {
        if let Some(home) = dirs::home_dir() {
            self.current_path = home;
            self.refresh().await?;
        }
        Ok(())
    }

    fn start_rename(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                self.rename_buffer = entry.name.clone();
                self.mode = AppMode::Rename;
            }
        }
    }

    async fn rename_selected(&mut self, new_name: &str) -> Result<()> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                let new_path = entry.path.with_file_name(new_name);
                tokio::fs::rename(&entry.path, &new_path).await?;
                self.refresh().await?;
            }
        }
        Ok(())
    }

    async fn create_file(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        tokio::fs::write(&path, "").await?;
        self.refresh().await?;
        Ok(())
    }

    async fn create_dir(&mut self, name: &str) -> Result<()> {
        let path = self.current_path.join(name);
        tokio::fs::create_dir(&path).await?;
        self.refresh().await?;
        Ok(())
    }

    fn copy_selected(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                self.clipboard = ClipboardOp::Copy;
                self.clipboard_paths = vec![entry.path.clone()];
            }
        }
    }

    fn cut_selected(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                self.clipboard = ClipboardOp::Cut;
                self.clipboard_paths = vec![entry.path.clone()];
            }
        }
    }

    async fn paste(&mut self) -> Result<()> {
        for src in &self.clipboard_paths {
            let file_name = src.file_name().unwrap();
            let dst = self.current_path.join(file_name);

            match self.clipboard {
                ClipboardOp::Copy => {
                    if src.is_dir() {
                        copy_dir(src, &dst).await?;
                    } else {
                        tokio::fs::copy(src, &dst).await?;
                    }
                }
                ClipboardOp::Cut => {
                    tokio::fs::rename(src, &dst).await?;
                }
                _ => {}
            }
        }
        self.clipboard = ClipboardOp::None;
        self.clipboard_paths.clear();
        self.refresh().await?;
        Ok(())
    }

    async fn delete_selected(&mut self) -> Result<()> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(entry) = self.entries.get(selected) {
                if entry.is_dir() {
                    tokio::fs::remove_dir_all(&entry.path).await?;
                } else {
                    tokio::fs::remove_file(&entry.path).await?;
                }
                self.refresh().await?;
            }
        }
        Ok(())
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
