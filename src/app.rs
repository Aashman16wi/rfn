use crate::config::{AppConfig, SortBy};
use crate::fs::{apply_git_status, find_git_root, get_git_status, read_dir, sort_entries};
use crate::keys::{Action, KeyMapper};
use crate::preview::generate_preview;
use crate::types::{AppMode, ClipboardOp, FileEntry, PreviewContent, PreviewType};
use crate::ui::file_list::{next, previous};
use crate::ui::help::render as render_help;
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
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

pub struct App {
    config: AppConfig,
    key_mapper: KeyMapper,
    picker: Picker,

    current_path: PathBuf,
    entries: Vec<FileEntry>,
    display: Vec<FileEntry>,
    list_state: ratatui::widgets::ListState,

    preview_content: PreviewContent,
    preview_protocol: Option<StatefulProtocol>,

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
    pub async fn new(
        config: AppConfig,
        start_path: std::path::PathBuf,
        picker: Picker,
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
            list_state: ratatui::widgets::ListState::default(),
            preview_content: PreviewContent::none(),
            preview_protocol: None,
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
                        terminal.draw(|f| self.render(f))?;
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
        self.entries = read_dir(&self.current_path, self.show_hidden).await?;
        sort_entries(&mut self.entries, self.sort_by, self.config.behavior.sort_dirs_first, self.reverse_sort);

        self.refresh_git().await?;
        self.rebuild_display();
        self.update_preview().await;

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

    fn selected_entry(&self) -> Option<&FileEntry> {
        self.list_state
            .selected()
            .and_then(|index| self.display.get(index))
    }

    fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.size();

        if self.help_visible {
            render_help(frame, area, &self.config.keys);
            return;
        }

        let (list_area, preview_area, status_area) =
            crate::ui::layout(area, self.show_preview, false);

        // File list
        crate::ui::file_list::render(
            frame,
            list_area,
            &self.display,
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
                self.preview_protocol.as_mut(),
                &self.config.ui.colors,
            );
        }

        // Status bar
        let selected_entry = self.selected_entry();
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
            AppMode::ConfirmDelete | AppMode::ConfirmQuit => {
                let name = self
                    .selected_entry()
                    .map(|entry| entry.name.clone())
                    .unwrap_or_default();
                crate::ui::prompt::render_confirm(
                    frame,
                    area,
                    self.mode,
                    &name,
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
        if key.code == crossterm::event::KeyCode::Esc && !self.search_query.is_empty() {
            self.search_query.clear();
            self.rebuild_display();
            self.update_preview().await;
            return Ok(());
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
                    self.update_preview().await;
                }
                Action::Down => {
                    next(&mut self.list_state, self.display.len());
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
                Action::Paste => self.paste().await?,
                Action::Search => {
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
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.is_dir() {
                self.current_path = entry.path;
                self.search_query.clear();
                self.list_state.select(Some(0));
                *self.list_state.offset_mut() = 0;
                self.refresh().await?;
            } else {
                // Open file with default application
                let _ = opener::open(&entry.path);
            }
        }
        Ok(())
    }

    async fn go_parent(&mut self) -> Result<()> {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.search_query.clear();
            self.list_state.select(Some(0));
            *self.list_state.offset_mut() = 0;
            self.refresh().await?;
        }
        Ok(())
    }

    async fn go_home(&mut self) -> Result<()> {
        if let Some(home) = dirs::home_dir() {
            self.current_path = home;
            self.search_query.clear();
            self.list_state.select(Some(0));
            *self.list_state.offset_mut() = 0;
            self.refresh().await?;
        }
        Ok(())
    }

    fn start_rename(&mut self) {
        if let Some(entry) = self.selected_entry() {
            self.rename_buffer = entry.name.clone();
            self.mode = AppMode::Rename;
        }
    }

    async fn rename_selected(&mut self, new_name: &str) -> Result<()> {
        if let Some(entry) = self.selected_entry().cloned() {
            let new_path = entry.path.with_file_name(new_name);
            tokio::fs::rename(&entry.path, &new_path).await?;
            self.refresh().await?;
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
        if let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) {
            self.clipboard = ClipboardOp::Copy;
            self.clipboard_paths = vec![path];
        }
    }

    fn cut_selected(&mut self) {
        if let Some(path) = self.selected_entry().map(|entry| entry.path.clone()) {
            self.clipboard = ClipboardOp::Cut;
            self.clipboard_paths = vec![path];
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
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.is_dir() {
                tokio::fs::remove_dir_all(&entry.path).await?;
            } else {
                tokio::fs::remove_file(&entry.path).await?;
            }
            self.refresh().await?;
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
