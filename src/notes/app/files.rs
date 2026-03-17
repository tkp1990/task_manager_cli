use chrono::Local;
use std::{
    error::Error,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use super::{
    char_to_byte_idx, copy_path_recursive, extract_note_references, fuzzy_matches, line_count,
    load_custom_templates, parse_file_metadata, render_preview_content, save_file_shortcuts,
    shell_quote, App, FileEntry, FileMetadata, FileShortcutKind, FileTemplate, InputMode,
    NoteReference, RelatedFileLink, SavedFileShortcut, TemplateDefinition, AUTOSAVE_IDLE_DELAY,
};

impl App {
    pub fn load_file_entries(&mut self) -> Result<(), Box<dyn Error>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.current_dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let metadata = entry.metadata()?;
            entries.push(FileEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                path,
                is_dir: file_type.is_dir(),
                size_bytes: metadata.len(),
                modified_at: metadata
                    .modified()
                    .ok()
                    .map(|time| chrono::DateTime::<chrono::Local>::from(time))
                    .map(|time| time.format("%Y-%m-%d %H:%M").to_string()),
            });
        }

        entries.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });

        self.file_entries = entries;
        self.ensure_file_selection_visible();
        self.refresh_file_search_results()?;
        self.sync_file_preview()?;
        Ok(())
    }

    pub fn refresh_file_browser(&mut self) -> Result<(), Box<dyn Error>> {
        self.load_file_entries()
    }

    pub fn move_file_selection_down(&mut self) {
        if self.file_selected + 1 < self.visible_file_entries().len() {
            self.file_selected += 1;
            let _ = self.sync_file_preview();
        }
    }

    pub fn move_file_selection_up(&mut self) {
        if self.file_selected > 0 {
            self.file_selected -= 1;
            let _ = self.sync_file_preview();
        }
    }

    pub fn has_file_search(&self) -> bool {
        !self.file_search_query.trim().is_empty()
    }

    pub fn begin_file_search(&mut self) {
        self.input_mode = InputMode::SearchingFiles;
    }

    pub fn begin_file_shortcuts(&mut self) {
        self.file_shortcut_selected = 0;
        self.input_mode = InputMode::FileShortcuts;
    }

    pub fn begin_file_links(&mut self) {
        self.file_link_selected = 0;
        self.input_mode = InputMode::FileLinks;
    }

    pub fn append_file_search_char(&mut self, c: char) {
        self.file_search_query.push(c);
        let _ = self.refresh_file_search_results();
        self.ensure_file_selection_visible();
        let _ = self.sync_file_preview();
    }

    pub fn pop_file_search_char(&mut self) {
        self.file_search_query.pop();
        let _ = self.refresh_file_search_results();
        self.ensure_file_selection_visible();
        let _ = self.sync_file_preview();
    }

    pub fn clear_file_search(&mut self) {
        self.file_search_query.clear();
        self.file_search_results.clear();
        self.file_selected = 0;
        let _ = self.sync_file_preview();
    }

    pub fn set_file_search_query(&mut self, query: &str) -> Result<(), Box<dyn Error>> {
        self.file_search_query = query.to_string();
        self.refresh_file_search_results()?;
        self.ensure_file_selection_visible();
        self.sync_file_preview()?;
        Ok(())
    }

    pub fn visible_file_entries(&self) -> &[FileEntry] {
        if self.has_file_search() {
            &self.file_search_results
        } else {
            &self.file_entries
        }
    }

    pub fn ensure_file_selection_visible(&mut self) {
        let len = self.visible_file_entries().len();
        if len == 0 {
            self.file_selected = 0;
        } else if self.file_selected >= len {
            self.file_selected = len - 1;
        }
    }

    pub fn selected_file_entry(&self) -> Option<&FileEntry> {
        self.visible_file_entries().get(self.file_selected)
    }

    pub fn begin_inline_file_edit(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file selected").into());
        };
        if entry.is_dir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Directories cannot be edited inline",
            )
            .into());
        }

        self.viewed_file_path = Some(entry.path.clone());
        self.file_edit_content = fs::read_to_string(&entry.path)?;
        self.file_edit_message = None;
        self.file_edit_cursor_row = self.editor_lines().len().saturating_sub(1);
        self.file_edit_cursor_col = self
            .editor_lines()
            .last()
            .map(|line| line.chars().count())
            .unwrap_or(0);
        self.file_edit_preferred_col = self.file_edit_cursor_col;
        self.file_edit_scroll = self.file_edit_cursor_row.saturating_sub(3);
        self.file_edit_scroll_x = self.file_edit_cursor_col.saturating_sub(20);
        self.file_edit_dirty = false;
        self.file_edit_last_change_at = None;
        self.input_mode = InputMode::EditingFile;
        Ok(())
    }

    pub fn save_inline_file_edit(&mut self) -> Result<(), Box<dyn Error>> {
        self.persist_inline_file_edit(false)
    }

    pub fn cancel_inline_file_edit(&mut self) {
        self.file_edit_content.clear();
        self.file_edit_message = None;
        self.file_edit_cursor_row = 0;
        self.file_edit_cursor_col = 0;
        self.file_edit_scroll = 0;
        self.file_edit_scroll_x = 0;
        self.file_edit_preferred_col = 0;
        self.file_edit_dirty = false;
        self.file_edit_last_change_at = None;
        self.input_mode = InputMode::ViewingFile;
    }

    fn persist_inline_file_edit(&mut self, keep_editing: bool) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.viewed_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file is open").into());
        };
        fs::write(&path, self.file_edit_content.as_bytes())?;
        self.load_file_entries()?;
        self.select_entry_path(&path);
        self.viewed_file_content = self.file_edit_content.clone();
        self.previewed_file_path = Some(path.clone());
        self.previewed_file_content = render_preview_content(&path, &self.file_edit_content);
        self.file_edit_dirty = false;
        self.file_edit_last_change_at = None;
        if keep_editing {
            self.file_edit_message =
                Some(format!("Autosaved at {}", Local::now().format("%H:%M:%S")));
        } else {
            self.file_edit_message = Some("Saved".to_string());
            self.input_mode = InputMode::ViewingFile;
        }
        self.add_log(
            "INFO",
            &format!(
                "{} file: {}",
                if keep_editing { "Autosaved" } else { "Saved" },
                path.display()
            ),
        );
        Ok(())
    }

    pub fn maybe_autosave_inline_file_edit(&mut self) -> Result<(), Box<dyn Error>> {
        if self.input_mode != InputMode::EditingFile || !self.file_edit_dirty {
            return Ok(());
        }

        let Some(last_change) = self.file_edit_last_change_at else {
            return Ok(());
        };
        if last_change.elapsed() < AUTOSAVE_IDLE_DELAY {
            return Ok(());
        }

        self.persist_inline_file_edit(true)
    }

    pub fn inline_editor_lines(&self) -> Vec<String> {
        self.editor_lines()
    }

    pub fn inline_editor_preview(&self) -> String {
        self.viewed_file_path
            .as_ref()
            .map(|path| render_preview_content(path, &self.file_edit_content))
            .unwrap_or_else(|| self.file_edit_content.clone())
    }

    pub fn move_file_edit_left(&mut self) {
        if self.file_edit_cursor_col > 0 {
            self.file_edit_cursor_col -= 1;
        } else if self.file_edit_cursor_row > 0 {
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = self.current_line_len();
        }
        self.file_edit_preferred_col = self.file_edit_cursor_col;
    }

    pub fn move_file_edit_right(&mut self) {
        let current_len = self.current_line_len();
        if self.file_edit_cursor_col < current_len {
            self.file_edit_cursor_col += 1;
        } else if self.file_edit_cursor_row + 1 < self.editor_lines().len() {
            self.file_edit_cursor_row += 1;
            self.file_edit_cursor_col = 0;
        }
        self.file_edit_preferred_col = self.file_edit_cursor_col;
    }

    pub fn move_file_edit_up(&mut self) {
        if self.file_edit_cursor_row > 0 {
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = self.file_edit_preferred_col.min(self.current_line_len());
        }
    }

    pub fn move_file_edit_down(&mut self) {
        if self.file_edit_cursor_row + 1 < self.editor_lines().len() {
            self.file_edit_cursor_row += 1;
            self.file_edit_cursor_col = self.file_edit_preferred_col.min(self.current_line_len());
        }
    }

    pub fn scroll_file_edit_up(&mut self) {
        if self.file_edit_scroll > 0 {
            self.file_edit_scroll -= 1;
        }
    }

    pub fn scroll_file_edit_down(&mut self) {
        if self.file_edit_scroll + 1 < self.editor_lines().len() {
            self.file_edit_scroll += 1;
        }
    }

    pub fn ensure_file_edit_cursor_visible(&mut self, height: usize, width: usize) {
        if self.file_edit_cursor_row < self.file_edit_scroll {
            self.file_edit_scroll = self.file_edit_cursor_row;
        } else if self.file_edit_cursor_row >= self.file_edit_scroll + height {
            self.file_edit_scroll = self
                .file_edit_cursor_row
                .saturating_sub(height.saturating_sub(1));
        }
        if self.file_edit_cursor_col < self.file_edit_scroll_x {
            self.file_edit_scroll_x = self.file_edit_cursor_col;
        } else if self.file_edit_cursor_col >= self.file_edit_scroll_x + width {
            self.file_edit_scroll_x = self
                .file_edit_cursor_col
                .saturating_sub(width.saturating_sub(1));
        }
    }

    pub fn insert_file_edit_char(&mut self, c: char) {
        let mut lines = self.editor_lines();
        while self.file_edit_cursor_row >= lines.len() {
            lines.push(String::new());
        }
        let row = self.file_edit_cursor_row;
        let col = self.file_edit_cursor_col.min(lines[row].chars().count());
        let byte_idx = char_to_byte_idx(&lines[row], col);
        lines[row].insert(byte_idx, c);
        self.file_edit_cursor_col += 1;
        self.file_edit_preferred_col = self.file_edit_cursor_col;
        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
        self.file_edit_dirty = true;
        self.file_edit_last_change_at = Some(Instant::now());
    }

    pub fn insert_file_edit_newline(&mut self) {
        let mut lines = self.editor_lines();
        while self.file_edit_cursor_row >= lines.len() {
            lines.push(String::new());
        }
        let row = self.file_edit_cursor_row;
        let col = self.file_edit_cursor_col.min(lines[row].chars().count());
        let byte_idx = char_to_byte_idx(&lines[row], col);
        let remainder = lines[row][byte_idx..].to_string();
        lines[row].truncate(byte_idx);
        lines.insert(row + 1, remainder);
        self.file_edit_cursor_row += 1;
        self.file_edit_cursor_col = 0;
        self.file_edit_preferred_col = 0;
        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
        self.file_edit_dirty = true;
        self.file_edit_last_change_at = Some(Instant::now());
    }

    pub fn insert_file_edit_tab(&mut self) {
        for _ in 0..4 {
            self.insert_file_edit_char(' ');
        }
    }

    pub fn backspace_file_edit(&mut self) {
        let mut lines = self.editor_lines();
        if lines.is_empty() {
            lines.push(String::new());
        }

        if self.file_edit_cursor_col > 0 {
            let row = self.file_edit_cursor_row.min(lines.len().saturating_sub(1));
            let col = self.file_edit_cursor_col.min(lines[row].chars().count());
            let end = char_to_byte_idx(&lines[row], col);
            let start = char_to_byte_idx(&lines[row], col - 1);
            lines[row].replace_range(start..end, "");
            self.file_edit_cursor_col -= 1;
            self.file_edit_preferred_col = self.file_edit_cursor_col;
        } else if self.file_edit_cursor_row > 0 {
            let row = self.file_edit_cursor_row.min(lines.len().saturating_sub(1));
            let previous_len = lines[row - 1].chars().count();
            let current = lines.remove(row);
            lines[row - 1].push_str(&current);
            self.file_edit_cursor_row -= 1;
            self.file_edit_cursor_col = previous_len;
            self.file_edit_preferred_col = previous_len;
        }

        self.file_edit_content = lines.join("\n");
        self.file_edit_message = None;
        self.file_edit_dirty = true;
        self.file_edit_last_change_at = Some(Instant::now());
    }

    fn editor_lines(&self) -> Vec<String> {
        if self.file_edit_content.is_empty() {
            vec![String::new()]
        } else {
            self.file_edit_content
                .lines()
                .map(|line| line.to_string())
                .collect()
        }
    }

    fn current_line_len(&self) -> usize {
        self.editor_lines()
            .get(self.file_edit_cursor_row)
            .map(|line| line.chars().count())
            .unwrap_or(0)
    }

    pub fn open_selected_file_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Ok(());
        };

        if entry.is_dir {
            self.current_dir = entry.path;
            self.file_selected = 0;
            self.clear_file_search();
            self.load_file_entries()?;
            self.add_log(
                "INFO",
                &format!("Opened directory: {}", self.current_dir.display()),
            );
            return Ok(());
        }

        self.open_file_path(&entry.path)
    }

    pub fn open_file_path(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        let bytes = fs::read(path)?;
        self.current_dir = path.parent().unwrap_or(&self.notes_root).to_path_buf();
        self.clear_file_search();
        self.load_file_entries()?;
        self.select_entry_path(path);
        self.viewed_file_content = String::from_utf8_lossy(&bytes).into_owned();
        self.viewed_file_path = Some(path.to_path_buf());
        self.viewed_file_scroll = 0;
        self.file_link_selected = 0;
        self.file_view_links_focus = false;
        self.input_mode = InputMode::ViewingFile;
        self.add_log("INFO", &format!("Opened file: {}", path.display()));
        Ok(())
    }

    pub fn move_to_parent_directory(&mut self) -> Result<(), Box<dyn Error>> {
        if self.current_dir == self.notes_root {
            return Ok(());
        }

        if let Some(parent) = self.current_dir.parent() {
            if parent.starts_with(&self.notes_root) {
                self.current_dir = parent.to_path_buf();
                self.file_selected = 0;
                self.clear_file_search();
                self.load_file_entries()?;
            }
        }
        Ok(())
    }

    pub fn relative_current_dir(&self) -> String {
        self.relative_path_from_root(&self.current_dir)
    }

    pub fn relative_path_from_root(&self, path: &Path) -> String {
        path.strip_prefix(&self.notes_root)
            .ok()
            .and_then(|relative| {
                let display = relative.display().to_string();
                if display.is_empty() {
                    None
                } else {
                    Some(format!("/{}", display))
                }
            })
            .unwrap_or_else(|| "/".to_string())
    }

    pub fn selected_file_breadcrumb(&self) -> String {
        self.selected_file_entry()
            .map(|entry| self.relative_path_from_root(&entry.path))
            .unwrap_or_else(|| "/".to_string())
    }

    pub fn preview_summary(&self) -> String {
        if let Some(path) = &self.previewed_file_path {
            if path.is_dir() {
                "Directory preview".to_string()
            } else {
                let lines = self.previewed_file_content.lines().count();
                let backlink_count = self.file_backlinks(path).len();
                let metadata = self.file_metadata(path);
                let title_summary = metadata
                    .title
                    .as_deref()
                    .map(|title| format!("title: {title}"))
                    .unwrap_or_else(|| "no frontmatter title".to_string());
                let tag_summary = if metadata.tags.is_empty() {
                    "no tags".to_string()
                } else {
                    format!("tags: {}", metadata.tags.join(", "))
                };
                format!(
                    "{lines} lines | {backlink_count} backlinks | {title_summary} | {tag_summary}"
                )
            }
        } else {
            "No selection".to_string()
        }
    }

    pub fn file_metadata(&self, path: &Path) -> FileMetadata {
        let Ok(content) = fs::read_to_string(path) else {
            return FileMetadata::default();
        };
        parse_file_metadata(&content)
    }

    pub fn viewed_file_metadata(&self) -> FileMetadata {
        self.viewed_file_path
            .as_ref()
            .map(|path| self.file_metadata(path))
            .unwrap_or_default()
    }

    pub fn preview_line_count(&self) -> usize {
        line_count(&self.previewed_file_content)
    }

    pub fn viewed_file_line_count(&self) -> usize {
        line_count(&self.viewed_file_content)
    }

    pub fn scroll_preview_up(&mut self, amount: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(amount);
    }

    pub fn scroll_preview_down(&mut self, amount: usize) {
        self.preview_scroll = self
            .preview_scroll
            .saturating_add(amount)
            .min(self.preview_line_count().saturating_sub(1));
    }

    pub fn scroll_viewed_file_up(&mut self, amount: usize) {
        self.viewed_file_scroll = self.viewed_file_scroll.saturating_sub(amount);
    }

    pub fn scroll_viewed_file_down(&mut self, amount: usize) {
        self.viewed_file_scroll = self
            .viewed_file_scroll
            .saturating_add(amount)
            .min(self.viewed_file_line_count().saturating_sub(1));
    }

    pub fn file_references(&self, path: &Path) -> Vec<NoteReference> {
        let Ok(content) = fs::read_to_string(path) else {
            return Vec::new();
        };
        extract_note_references(&self.notes_root, path, &content)
    }

    pub fn file_backlinks(&self, target: &Path) -> Vec<NoteReference> {
        let Ok(entries) = Self::collect_recursive_entries(&self.notes_root) else {
            return Vec::new();
        };
        let mut backlinks = Vec::new();

        for entry in entries.into_iter().filter(|entry| !entry.is_dir) {
            let Ok(content) = fs::read_to_string(&entry.path) else {
                continue;
            };
            let refs = extract_note_references(&self.notes_root, &entry.path, &content);
            if refs.iter().any(|reference| reference.path == target) {
                backlinks.push(NoteReference {
                    label: self.relative_path_from_root(&entry.path),
                    path: entry.path,
                });
            }
        }

        backlinks.sort_by(|left, right| left.label.cmp(&right.label));
        backlinks
    }

    pub fn related_file_links(&self) -> Vec<RelatedFileLink> {
        let Some(path) = self.viewed_file_path.as_ref() else {
            return Vec::new();
        };
        let mut links = Vec::new();
        links.extend(
            self.file_references(path)
                .into_iter()
                .map(|reference| RelatedFileLink {
                    group: "References",
                    label: reference.label,
                    path: reference.path,
                }),
        );
        links.extend(
            self.file_backlinks(path)
                .into_iter()
                .map(|reference| RelatedFileLink {
                    group: "Backlinks",
                    label: reference.label,
                    path: reference.path,
                }),
        );
        links
    }

    pub fn toggle_file_view_links_focus(&mut self) {
        self.file_view_links_focus = !self.file_view_links_focus;
    }

    pub fn move_file_link_down(&mut self) {
        if self.file_link_selected + 1 < self.related_file_links().len() {
            self.file_link_selected += 1;
        }
    }

    pub fn move_file_link_up(&mut self) {
        if self.file_link_selected > 0 {
            self.file_link_selected -= 1;
        }
    }

    pub fn open_selected_related_link(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(link) = self
            .related_file_links()
            .get(self.file_link_selected)
            .cloned()
        else {
            return Ok(());
        };
        self.open_file_path(&link.path)
    }

    pub fn begin_create_file(&mut self) {
        self.pending_file_path = None;
        self.file_name_input.clear();
        self.file_form_message = None;
        let _ = self.reload_custom_file_templates();
        self.file_template_selected = 0;
        self.input_mode = InputMode::CreatingFile;
    }

    pub fn begin_create_directory(&mut self) {
        self.pending_file_path = None;
        self.file_name_input.clear();
        self.file_form_message = None;
        self.input_mode = InputMode::CreatingDirectory;
    }

    pub fn clear_file_form_message(&mut self) {
        self.file_form_message = None;
    }

    pub fn set_file_form_message<T: Into<String>>(&mut self, message: T) {
        self.file_form_message = Some(message.into());
    }

    pub fn all_file_templates(&self) -> Vec<TemplateDefinition> {
        let mut templates = vec![
            TemplateDefinition {
                name: FileTemplate::Blank.name().to_string(),
                content: String::new(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::DailyNote.name().to_string(),
                content: "# {{date}}\n\n## Goals\n\n- \n\n## Notes\n\n## Tasks\n\n- [ ] \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::MeetingNote.name().to_string(),
                content: "# Meeting: {{title}}\n\nDate: {{date}}\nTime: {{time}}\nAttendees:\n\n## Agenda\n\n## Notes\n\n## Action Items\n\n- [ ] \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::ProjectNote.name().to_string(),
                content: "# Project: {{title}}\n\nCreated: {{date}}\n\n## Summary\n\n## Milestones\n\n- \n\n## Open Questions\n\n- \n"
                    .to_string(),
                is_custom: false,
            },
            TemplateDefinition {
                name: FileTemplate::JournalEntry.name().to_string(),
                content: "# Journal - {{date}} ({{weekday}})\n\n## Mood\n\n## Highlights\n\n## Reflection\n\n"
                    .to_string(),
                is_custom: false,
            },
        ];
        templates.extend(self.custom_file_templates.clone());
        templates
    }

    pub fn selected_file_template_name(&self) -> String {
        self.all_file_templates()
            .get(self.file_template_selected)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| FileTemplate::Blank.name().to_string())
    }

    pub fn move_file_template_down(&mut self) {
        if self.file_template_selected + 1 < self.all_file_templates().len() {
            self.file_template_selected += 1;
        }
    }

    pub fn move_file_template_up(&mut self) {
        if self.file_template_selected > 0 {
            self.file_template_selected -= 1;
        }
    }

    pub fn create_or_open_daily_note(&mut self) -> Result<(), Box<dyn Error>> {
        let daily_dir = self.notes_root.join("daily");
        fs::create_dir_all(&daily_dir)?;
        let file_name = format!("{}.md", Local::now().format("%Y-%m-%d"));
        let target = daily_dir.join(&file_name);
        if !target.exists() {
            let previous = self.file_template_selected;
            self.file_template_selected = 1;
            fs::write(&target, self.render_selected_template(&file_name))?;
            self.file_template_selected = previous;
            self.add_log("INFO", &format!("Created daily note: {}", target.display()));
        }
        self.open_file_path(&target)
    }

    pub fn cancel_file_creation(&mut self) {
        self.file_name_input.clear();
        self.file_form_message = None;
        self.pending_file_path = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn create_file(&mut self) -> Result<(), Box<dyn Error>> {
        let final_name = self.normalized_child_name(true)?;
        let target = self.current_dir.join(&final_name);
        if target.exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "File already exists").into());
        }

        fs::write(&target, self.render_selected_template(&final_name))?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log("INFO", &format!("Created file: {}", target.display()));
        self.cancel_file_creation();
        Ok(())
    }

    pub fn create_directory(&mut self) -> Result<(), Box<dyn Error>> {
        let final_name = self.normalized_child_name(false)?;
        let target = self.current_dir.join(&final_name);
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Directory already exists").into(),
            );
        }

        fs::create_dir(&target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log("INFO", &format!("Created directory: {}", target.display()));
        self.cancel_file_creation();
        Ok(())
    }

    pub fn begin_rename_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = entry.name.clone();
            self.file_form_message = None;
            self.input_mode = InputMode::RenamingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to rename");
        }
    }

    pub fn begin_move_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = self
                .relative_path_from_root(&entry.path)
                .trim_start_matches('/')
                .to_string();
            self.file_form_message = None;
            self.input_mode = InputMode::MovingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to move");
        }
    }

    pub fn begin_copy_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.file_name_input = self
                .relative_path_from_root(&entry.path)
                .trim_start_matches('/')
                .to_string();
            self.file_form_message = None;
            self.input_mode = InputMode::CopyingFileEntry;
        } else {
            self.add_log("WARN", "No file entry selected to copy");
        }
    }

    pub fn rename_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(original_path) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let is_file = original_path.is_file();
        let new_name = self.normalized_child_name(is_file)?;
        let target = original_path
            .parent()
            .unwrap_or(&self.current_dir)
            .join(new_name);
        if target == original_path {
            self.cancel_file_creation();
            return Ok(());
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Target already exists").into(),
            );
        }

        fs::rename(&original_path, &target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!(
                "Renamed {} to {}",
                original_path.display(),
                target
                    .file_name()
                    .unwrap_or_else(|| OsStr::new(""))
                    .to_string_lossy()
            ),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn begin_delete_selected_entry(&mut self) {
        if let Some(entry) = self.selected_file_entry().cloned() {
            self.pending_file_path = Some(entry.path.clone());
            self.input_mode = InputMode::DeletingFileEntry;
            self.file_form_message = None;
        } else {
            self.add_log("WARN", "No file entry selected to delete");
        }
    }

    pub fn delete_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(target) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        } else {
            fs::remove_file(&target)?;
        }
        self.load_file_entries()?;
        self.add_log("INFO", &format!("Deleted {}", target.display()));
        self.pending_file_path = None;
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    pub fn move_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(source) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let target = self.resolve_destination_path(&source)?;
        if target == source {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Destination is unchanged").into(),
            );
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Destination already exists").into(),
            );
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target)?;
        if source.parent() != Some(&self.current_dir) {
            if let Some(parent) = target.parent() {
                self.current_dir = parent.to_path_buf();
            }
        }
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!("Moved {} to {}", source.display(), target.display()),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn copy_selected_entry(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(source) = self.pending_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file entry selected").into());
        };
        let target = self.resolve_destination_path(&source)?;
        if target == source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must differ from source",
            )
            .into());
        }
        if target.exists() {
            return Err(
                io::Error::new(io::ErrorKind::AlreadyExists, "Destination already exists").into(),
            );
        }
        copy_path_recursive(&source, &target)?;
        self.load_file_entries()?;
        self.select_entry_path(&target);
        self.add_log(
            "INFO",
            &format!("Copied {} to {}", source.display(), target.display()),
        );
        self.cancel_file_creation();
        Ok(())
    }

    pub fn edit_selected_file_in_editor(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file selected").into());
        };
        self.edit_file_in_editor(&entry.path)
    }

    pub fn edit_viewed_file_in_editor(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(path) = self.viewed_file_path.clone() else {
            return Err(io::Error::new(io::ErrorKind::NotFound, "No file is open").into());
        };
        self.edit_file_in_editor(&path)
    }

    fn edit_file_in_editor(&mut self, path: &Path) -> Result<(), Box<dyn Error>> {
        if path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Directories cannot be edited in an external editor",
            )
            .into());
        }
        let editor = self
            .editor_command
            .clone()
            .or_else(|| std::env::var("NOTES_EDITOR").ok())
            .or_else(|| std::env::var("EDITOR").ok())
            .unwrap_or_else(|| "nvim".to_string());
        let quoted_path = shell_quote(path);
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("{editor} {quoted_path}"))
            .status()?;
        if !status.success() {
            return Err(io::Error::other(format!("Editor exited with status {status}")).into());
        }

        self.load_file_entries()?;
        self.select_entry_path(path);
        let bytes = fs::read(path)?;
        let content = String::from_utf8_lossy(&bytes).into_owned();
        self.viewed_file_path = Some(path.to_path_buf());
        self.viewed_file_content = content.clone();
        self.previewed_file_path = Some(path.to_path_buf());
        self.previewed_file_content = render_preview_content(path, &content);
        self.add_log("INFO", &format!("Edited file: {}", path.display()));
        Ok(())
    }

    pub fn render_selected_template(&self, file_name: &str) -> String {
        let title = Path::new(file_name)
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or(file_name);
        let now = Local::now();
        let template = self
            .all_file_templates()
            .get(self.file_template_selected)
            .map(|template| template.content.clone())
            .unwrap_or_default();

        [
            ("{{date}}", now.format("%Y-%m-%d").to_string()),
            ("{{time}}", now.format("%H:%M").to_string()),
            ("{{weekday}}", now.format("%A").to_string()),
            ("{{title}}", title.to_string()),
        ]
        .into_iter()
        .fold(template, |acc, (key, value)| acc.replace(key, &value))
    }

    pub fn reload_custom_file_templates(&mut self) -> Result<(), Box<dyn Error>> {
        self.custom_file_templates = load_custom_templates(&self.templates_dir)?;
        Ok(())
    }

    fn normalized_child_name(&self, default_markdown: bool) -> Result<String, Box<dyn Error>> {
        let trimmed = self.file_name_input.trim();
        if trimmed.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Name cannot be empty").into());
        }
        if trimmed.contains(std::path::MAIN_SEPARATOR) || trimmed.contains("..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Name must stay within the current directory",
            )
            .into());
        }
        Ok(
            if default_markdown && Path::new(trimmed).extension().is_none() {
                format!("{trimmed}.md")
            } else {
                trimmed.to_string()
            },
        )
    }

    fn resolve_destination_path(&self, source: &Path) -> Result<PathBuf, Box<dyn Error>> {
        let raw = self.file_name_input.trim().trim_start_matches('/');
        if raw.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "Destination cannot be empty").into(),
            );
        }
        if raw.contains("..") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must stay within notes root",
            )
            .into());
        }

        let mut target = self.notes_root.join(raw);
        if target.exists() && target.is_dir() {
            target = target.join(source.file_name().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Source has no file name")
            })?);
        }
        if !target.starts_with(&self.notes_root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Destination must stay within notes root",
            )
            .into());
        }
        Ok(target)
    }

    fn select_entry_path(&mut self, target: &Path) {
        let target = target.to_path_buf();
        if self.has_file_search() {
            if let Some(index) = self
                .file_search_results
                .iter()
                .position(|entry| entry.path == target)
            {
                self.file_selected = index;
            }
        } else if let Some(index) = self
            .file_entries
            .iter()
            .position(|entry| entry.path == target)
        {
            self.file_selected = index;
        }
        let _ = self.sync_file_preview();
    }

    pub fn select_file_entry_path(&mut self, target: &Path) {
        self.select_entry_path(target);
    }

    pub fn all_file_shortcuts(&self) -> &[SavedFileShortcut] {
        &self.file_shortcuts
    }

    pub fn toggle_pin_current_directory(&mut self) -> Result<(), Box<dyn Error>> {
        let path_text = self.current_dir.to_string_lossy().to_string();
        if let Some(index) = self.file_shortcuts.iter().position(|shortcut| {
            shortcut.kind == FileShortcutKind::Directory && shortcut.target == path_text
        }) {
            let removed = self.file_shortcuts.remove(index);
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            self.add_log(
                "INFO",
                &format!("Removed pinned directory: {}", removed.name),
            );
        } else {
            let name = if self.current_dir == self.notes_root {
                "Root".to_string()
            } else {
                self.current_dir
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("directory"))
                    .to_string_lossy()
                    .to_string()
            };
            self.file_shortcuts.push(SavedFileShortcut {
                name,
                target: path_text,
                kind: FileShortcutKind::Directory,
            });
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            self.add_log("INFO", "Pinned current directory");
        }
        Ok(())
    }

    pub fn save_current_file_search(&mut self) -> Result<(), Box<dyn Error>> {
        let query = self.file_search_query.trim();
        if query.is_empty() {
            return Err(
                io::Error::new(io::ErrorKind::InvalidInput, "No file search to save").into(),
            );
        }

        if let Some(existing) = self.file_shortcuts.iter_mut().find(|shortcut| {
            shortcut.kind == FileShortcutKind::Search && shortcut.name.eq_ignore_ascii_case(query)
        }) {
            existing.target = query.to_string();
        } else {
            self.file_shortcuts.push(SavedFileShortcut {
                name: query.to_string(),
                target: query.to_string(),
                kind: FileShortcutKind::Search,
            });
        }
        save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
        self.add_log("INFO", &format!("Saved search: {}", query));
        Ok(())
    }

    pub fn move_file_shortcut_down(&mut self) {
        if self.file_shortcut_selected + 1 < self.file_shortcuts.len() {
            self.file_shortcut_selected += 1;
        }
    }

    pub fn move_file_shortcut_up(&mut self) {
        if self.file_shortcut_selected > 0 {
            self.file_shortcut_selected -= 1;
        }
    }

    pub fn apply_selected_file_shortcut(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(shortcut) = self
            .file_shortcuts
            .get(self.file_shortcut_selected)
            .cloned()
        else {
            return Ok(());
        };
        match shortcut.kind {
            FileShortcutKind::Directory => {
                let target = PathBuf::from(shortcut.target);
                if target.exists() && target.starts_with(&self.notes_root) {
                    self.current_dir = target;
                    self.clear_file_search();
                    self.file_selected = 0;
                    self.load_file_entries()?;
                }
            }
            FileShortcutKind::Search => {
                self.set_file_search_query(&shortcut.target)?;
            }
        }
        self.add_log("INFO", &format!("Opened shortcut: {}", shortcut.name));
        Ok(())
    }

    pub fn delete_selected_file_shortcut(&mut self) -> Result<(), Box<dyn Error>> {
        if self.file_shortcut_selected < self.file_shortcuts.len() {
            let removed = self.file_shortcuts.remove(self.file_shortcut_selected);
            save_file_shortcuts(&self.file_shortcuts_store_path, &self.file_shortcuts)?;
            if self.file_shortcut_selected > 0
                && self.file_shortcut_selected >= self.file_shortcuts.len()
            {
                self.file_shortcut_selected -= 1;
            }
            self.add_log("INFO", &format!("Removed shortcut: {}", removed.name));
        }
        Ok(())
    }

    fn refresh_file_search_results(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.has_file_search() {
            self.file_search_results.clear();
            return Ok(());
        }

        let query = self.file_search_query.clone();
        let mut results = Self::collect_recursive_entries(&self.notes_root)?
            .into_iter()
            .filter(|entry| self.file_entry_matches_filter(entry, &query))
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        self.file_search_results = results;
        self.ensure_file_selection_visible();
        Ok(())
    }

    fn file_entry_matches_filter(&self, entry: &FileEntry, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }

        let relative = entry
            .path
            .strip_prefix(&self.notes_root)
            .unwrap_or(&entry.path)
            .display()
            .to_string()
            .to_lowercase();
        let name = entry.name.to_lowercase();
        let metadata = if entry.is_dir {
            FileMetadata::default()
        } else {
            self.file_metadata(&entry.path)
        };
        let title = metadata.title.unwrap_or_default().to_lowercase();
        let tags = metadata.tags.join(" ").to_lowercase();

        Self::filter_tokens(trimmed).into_iter().all(|token| {
            let (negated, token) = if let Some(token) = token.strip_prefix('-') {
                (true, token)
            } else {
                (false, token.as_str())
            };

            let token_lower = token.to_lowercase();
            let matches = if let Some(value) = token_lower.strip_prefix("title:") {
                !value.is_empty() && title.contains(value)
            } else if let Some(value) = token_lower.strip_prefix("tag:") {
                !value.is_empty()
                    && metadata
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(value))
            } else if let Some(value) = token_lower.strip_prefix("tags:") {
                !value.is_empty()
                    && metadata
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(value))
            } else if let Some(value) = token_lower.strip_prefix("path:") {
                !value.is_empty() && relative.contains(value)
            } else if let Some(value) = token_lower.strip_prefix("name:") {
                !value.is_empty() && name.contains(value)
            } else {
                fuzzy_matches(&relative, &token_lower)
                    || fuzzy_matches(&name, &token_lower)
                    || (!title.is_empty() && title.contains(&token_lower))
                    || (!tags.is_empty() && tags.contains(&token_lower))
            };

            if negated {
                !matches
            } else {
                matches
            }
        })
    }

    fn collect_recursive_entries(root: &Path) -> Result<Vec<FileEntry>, Box<dyn Error>> {
        let mut entries = Vec::new();
        Self::collect_recursive_entries_into(root, &mut entries)?;
        Ok(entries)
    }

    fn collect_recursive_entries_into(
        dir: &Path,
        entries: &mut Vec<FileEntry>,
    ) -> Result<(), Box<dyn Error>> {
        let mut children = fs::read_dir(dir)?
            .map(|entry| {
                let entry = entry?;
                let path = entry.path();
                let file_type = entry.file_type()?;
                let metadata = entry.metadata()?;
                Ok(FileEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path,
                    is_dir: file_type.is_dir(),
                    size_bytes: metadata.len(),
                    modified_at: metadata
                        .modified()
                        .ok()
                        .map(|time| chrono::DateTime::<chrono::Local>::from(time))
                        .map(|time| time.format("%Y-%m-%d %H:%M").to_string()),
                })
            })
            .collect::<Result<Vec<_>, io::Error>>()?;

        children.sort_by(|left, right| {
            right
                .is_dir
                .cmp(&left.is_dir)
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });

        for child in children {
            let recurse_path = child.path.clone();
            let is_dir = child.is_dir;
            entries.push(child);
            if is_dir {
                Self::collect_recursive_entries_into(&recurse_path, entries)?;
            }
        }
        Ok(())
    }

    pub fn sync_file_preview(&mut self) -> Result<(), Box<dyn Error>> {
        let Some(entry) = self.selected_file_entry().cloned() else {
            self.previewed_file_path = None;
            self.previewed_file_content.clear();
            self.preview_scroll = 0;
            return Ok(());
        };

        self.previewed_file_path = Some(entry.path.clone());
        self.preview_scroll = 0;
        if entry.is_dir {
            let child_count = fs::read_dir(&entry.path)?.count();
            self.previewed_file_content = format!(
                "Directory: {}\nItems: {}\n\nPress Enter to open this folder.",
                entry.path.display(),
                child_count
            );
        } else {
            let bytes = fs::read(&entry.path)?;
            let content = String::from_utf8_lossy(&bytes).into_owned();
            self.previewed_file_content = render_preview_content(&entry.path, &content);
        }
        Ok(())
    }
}
