use std::{
    error::Error,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::db::notes::operations::DbOperations;
use crate::filter_presets::load_presets;

use super::{
    load_custom_templates, load_file_shortcuts, load_palette_history, App, InputMode, NotesView,
};

impl App {
    pub fn new_with_notes_root(db_path: &str, notes_root: PathBuf) -> Result<App, Box<dyn Error>> {
        let db_file_path = PathBuf::from(db_path);
        let db_stem = db_file_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("notes");
        let db_path_string = format!("sqlite://{}", db_path);
        let pool = crate::db::establish_connection_pool(&db_path_string)?;
        let preset_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_filter_presets.json"));
        let file_shortcuts_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_file_shortcuts.json"));
        let palette_history_store_path = db_file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{db_stem}.note_palette_history.json"));
        let templates_dir = notes_root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("templates");
        fs::create_dir_all(&notes_root)?;
        fs::create_dir_all(&templates_dir)?;

        {
            let mut conn = pool.get()?;
            crate::db::run_migrations(&mut conn)?;
        }

        let db_ops = DbOperations::new(pool);
        let mut app = App {
            db_ops,
            notes: Vec::new(),
            active_view: NotesView::Files,
            current_dir: notes_root.clone(),
            notes_root,
            file_entries: Vec::new(),
            file_search_query: String::new(),
            file_search_results: Vec::new(),
            file_shortcuts: load_file_shortcuts(&file_shortcuts_store_path)?,
            file_shortcut_selected: 0,
            file_link_selected: 0,
            file_view_links_focus: false,
            file_selected: 0,
            previewed_file_path: None,
            previewed_file_content: String::new(),
            preview_scroll: 0,
            viewed_file_path: None,
            viewed_file_content: String::new(),
            viewed_file_scroll: 0,
            editor_command: None,
            file_shortcuts_store_path,
            templates_dir: templates_dir.clone(),
            pending_file_path: None,
            file_name_input: String::new(),
            file_form_message: None,
            file_template_selected: 0,
            custom_file_templates: load_custom_templates(&templates_dir)?,
            file_edit_content: String::new(),
            file_edit_message: None,
            file_edit_cursor_row: 0,
            file_edit_cursor_col: 0,
            file_edit_scroll: 0,
            file_edit_scroll_x: 0,
            file_edit_preferred_col: 0,
            note_filter: String::new(),
            preset_selected: 0,
            custom_note_presets: load_presets(&preset_store_path)?,
            preset_store_path,
            palette_history_store_path: palette_history_store_path.clone(),
            selected: 0,
            input_mode: InputMode::Normal,
            command_palette_return_mode: InputMode::Normal,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            recent_palette_commands: load_palette_history(&palette_history_store_path)?,
            title_input: String::new(),
            content_input: String::new(),
            note_form_message: None,
            preset_name_input: String::new(),
            preset_form_message: None,
            editing_title: true,
            logs: Vec::new(),
            log_offset: 0,
        };
        app.load_notes()?;
        app.load_file_entries()?;
        app.add_log("INFO", "Notes loaded");
        app.add_log("INFO", "Application started");
        Ok(app)
    }

    pub fn load_notes(&mut self) -> Result<(), Box<dyn Error>> {
        self.notes = self.db_ops.load_notes()?;
        self.ensure_selected_visible();
        Ok(())
    }

    pub fn add_log(&mut self, level: &str, msg: &str) {
        crate::common::logs::push_timestamped_log(&mut self.logs, &mut self.log_offset, level, msg);
    }

    pub fn begin_command_palette(&mut self) {
        crate::common::palette::begin_palette(
            &mut self.command_palette_query,
            &mut self.command_palette_selected,
            &mut self.command_palette_return_mode,
            &mut self.input_mode,
            InputMode::CommandPalette,
        );
    }

    pub fn close_command_palette(&mut self) {
        crate::common::palette::close_palette(
            &mut self.command_palette_query,
            &mut self.command_palette_selected,
            self.command_palette_return_mode,
            &mut self.input_mode,
        );
    }

    pub fn record_palette_command(&mut self, command_id: &str) -> Result<(), Box<dyn Error>> {
        crate::common::palette::record_recent_command(
            &self.palette_history_store_path,
            &mut self.recent_palette_commands,
            command_id,
            8,
        )?;
        Ok(())
    }

    pub fn toggle_active_view(&mut self) {
        self.input_mode = InputMode::Normal;
        self.active_view = match self.active_view {
            NotesView::Files => NotesView::Database,
            NotesView::Database => NotesView::Files,
        };
    }

    pub fn focus_note_by_id(&mut self, note_id: i32) -> Result<bool, Box<dyn Error>> {
        let Some(note) = self.db_ops.find_note(note_id)? else {
            return Ok(false);
        };

        self.active_view = NotesView::Database;
        self.note_filter.clear();
        self.load_notes()?;

        if let Some(index) = self
            .notes
            .iter()
            .position(|candidate| candidate.id == note.id)
        {
            self.selected = index;
            self.ensure_selected_visible();
        }

        self.add_log("INFO", &format!("Focused note id: {}", note_id));
        Ok(true)
    }
}
