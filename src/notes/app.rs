mod core;
mod files;
mod helpers;
mod notes;
mod presets;
mod types;
pub use helpers::format_file_size;
use helpers::*;
pub use types::{
    App, FileEntry, FileMetadata, FileShortcutKind, FileTemplate, InputMode, NoteReference,
    NotesView, RelatedFileLink, SavedFileShortcut, TemplateDefinition,
};

#[cfg(test)]
mod tests {
    use super::{parse_file_metadata, App, InputMode};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_db_path(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_notes_app_{unique}.db"))
    }

    fn temp_notes_root(prefix: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}_{}",
            prefix,
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        );
        std::env::temp_dir().join(format!("task_manager_cli_notes_files_{unique}"))
    }

    fn test_app(db_path: &str, prefix: &str) -> Result<App, Box<dyn std::error::Error>> {
        App::new_with_notes_root(db_path, temp_notes_root(prefix))
    }

    #[test]
    fn begin_add_note_resets_inputs() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("begin_add");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "begin_add")?;

        app.title_input = "stale".to_string();
        app.content_input = "stale".to_string();
        app.editing_title = false;

        app.begin_add_note();

        assert_eq!(app.input_mode, InputMode::AddingNote);
        assert!(app.title_input.is_empty());
        assert!(app.content_input.is_empty());
        assert!(app.editing_title);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn reset_inputs_clears_inline_feedback() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("feedback");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "feedback")?;

        app.title_input = "Draft".to_string();
        app.content_input = "Draft body".to_string();
        app.set_note_form_message("Note title cannot be empty");

        app.reset_inputs();

        assert!(app.title_input.is_empty());
        assert!(app.content_input.is_empty());
        assert!(app.note_form_message.is_none());

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn parse_file_metadata_extracts_title_and_tags() {
        let metadata = parse_file_metadata(
            "---\n\
title: Sprint Review\n\
tags:\n\
- work\n\
- weekly\n\
---\n\
# Body\n",
        );

        assert_eq!(metadata.title.as_deref(), Some("Sprint Review"));
        assert_eq!(
            metadata.tags,
            vec!["weekly".to_string(), "work".to_string()]
        );
    }

    #[test]
    fn inline_editor_supports_cursor_navigation_and_insertions(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_cursor");
        let notes_root = temp_notes_root("inline_cursor");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"ab\ncd")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        app.move_file_edit_up();
        app.move_file_edit_left();
        app.insert_file_edit_char('X');
        app.insert_file_edit_newline();
        app.insert_file_edit_char('Y');

        assert_eq!(app.file_edit_content, "aX\nYb\ncd");
        assert_eq!(app.file_edit_cursor_row, 1);
        assert_eq!(app.file_edit_cursor_col, 1);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_editor_backspace_merges_lines() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_backspace");
        let notes_root = temp_notes_root("inline_backspace");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"ab\ncd")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        app.move_file_edit_up();
        app.move_file_edit_down();
        app.file_edit_cursor_col = 0;
        app.backspace_file_edit();

        assert_eq!(app.file_edit_content, "abcd");
        assert_eq!(app.file_edit_cursor_row, 0);
        assert_eq!(app.file_edit_cursor_col, 2);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_editor_tracks_preferred_column_and_horizontal_scroll(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_scroll");
        let notes_root = temp_notes_root("inline_scroll");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"abcdefghij\nxy")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;

        for _ in 0..5 {
            app.move_file_edit_left();
        }
        app.ensure_file_edit_cursor_visible(4, 3);
        assert_eq!(app.file_edit_scroll_x, 6);

        app.move_file_edit_down();
        assert_eq!(app.file_edit_cursor_col, 2);
        app.move_file_edit_up();
        assert_eq!(app.file_edit_cursor_col, 8);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }
}
