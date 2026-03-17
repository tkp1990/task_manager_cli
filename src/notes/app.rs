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
    use super::{parse_file_metadata, App, FileShortcutKind, InputMode, NotesView};
    use chrono::Local;
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
    fn begin_edit_note_prefills_existing_content() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("begin_edit");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "begin_edit")?;

        app.add_note("Title", "Body")?;
        app.begin_edit_note();

        assert_eq!(app.input_mode, InputMode::EditingNote);
        assert_eq!(app.title_input, "Title");
        assert_eq!(app.content_input, "Body");
        assert!(app.editing_title);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn add_note_rejects_blank_titles() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("blank_title");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "blank_title")?;

        let error = app.add_note("   ", "Body").unwrap_err();
        assert!(error.to_string().contains("Note title cannot be empty"));

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
    fn note_filter_keeps_selection_on_visible_match() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("filter");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "filter")?;

        app.add_note("Alpha", "First body")?;
        app.add_note("Beta", "Second body")?;
        app.selected = 1;
        app.note_filter = "Alpha".to_string();
        app.ensure_selected_visible();

        assert_eq!(app.filtered_note_indices(), vec![1]);
        assert_eq!(app.selected, 1);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn note_filter_supports_title_and_body_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_filter_tokens");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "note_filter_tokens")?;

        app.add_note("Project Alpha", "Meeting notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;

        app.note_filter = "title:project".to_string();
        assert_eq!(app.filtered_note_indices(), vec![1]);

        app.note_filter = "body:apples".to_string();
        assert_eq!(app.filtered_note_indices(), vec![0]);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn note_filter_supports_phrases_and_negation() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_filter_phrases");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "note_filter_phrases")?;

        app.add_note("Project Alpha", "Roadmap review notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;

        app.note_filter = "title:\"Project Alpha\"".to_string();
        assert_eq!(app.filtered_note_indices(), vec![1]);

        app.note_filter = "\"buy apples\" -title:shopping".to_string();
        assert!(app.filtered_note_indices().is_empty());

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn applying_note_preset_sets_filter_and_selection() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_preset");
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = test_app(&db_path_str, "note_preset")?;

        app.add_note("Project Alpha", "Meeting notes")?;
        app.add_note("Shopping", "Buy apples and bread")?;
        app.selected = 0;

        app.preset_selected = 2;
        app.apply_selected_note_preset();

        assert_eq!(app.note_filter, "title:shopping");
        assert_eq!(app.filtered_note_indices(), vec![0]);
        assert_eq!(app.selected, 0);

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn file_browser_loads_directories_before_files() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_browser");
        let notes_root = temp_notes_root("file_browser");
        fs::create_dir_all(notes_root.join("projects"))?;
        fs::write(notes_root.join("inbox.md"), b"# Inbox")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        assert_eq!(app.active_view, NotesView::Files);
        assert_eq!(app.file_entries.len(), 2);
        assert!(app.file_entries[0].is_dir);
        assert_eq!(app.file_entries[0].name, "projects");
        assert_eq!(app.file_entries[1].name, "inbox.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_browser_can_create_and_open_files() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("create_file");
        let notes_root = temp_notes_root("create_file");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();
        app.file_name_input = "daily".to_string();
        app.create_file()?;

        assert!(notes_root.join("daily.md").exists());
        assert_eq!(app.input_mode, InputMode::Normal);

        app.open_selected_file_entry()?;
        assert_eq!(app.input_mode, InputMode::ViewingFile);
        assert_eq!(
            app.viewed_file_path.as_deref(),
            Some(notes_root.join("daily.md").as_path())
        );

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn create_file_uses_selected_template() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_template");
        let notes_root = temp_notes_root("file_template");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();
        app.file_name_input = "standup".to_string();
        app.file_template_selected = 2;
        app.create_file()?;

        let content = fs::read_to_string(notes_root.join("standup.md"))?;
        assert!(content.contains("# Meeting: standup"));
        assert!(content.contains("## Agenda"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn daily_template_renders_date_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("template_render");
        let notes_root = temp_notes_root("template_render");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.file_template_selected = 1;
        let content = app.render_selected_template("daily.md");

        assert!(content.contains("## Goals"));
        assert!(content.contains(&Local::now().format("%Y-%m-%d").to_string()));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn custom_templates_load_from_notes_templates_dir() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("custom_templates");
        let notes_root = temp_notes_root("custom_templates");
        let templates_dir = notes_root
            .parent()
            .expect("temp notes root should have parent")
            .join("templates");
        fs::create_dir_all(&templates_dir)?;
        fs::write(
            templates_dir.join("brainstorm.md"),
            "# {{title}}\n\n## Ideas\n\n- \n",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_create_file();

        let templates = app.all_file_templates();
        assert!(templates
            .iter()
            .any(|template| template.name == "brainstorm"));

        app.file_template_selected = templates
            .iter()
            .position(|template| template.name == "brainstorm")
            .expect("custom template should be selectable");
        let content = app.render_selected_template("idea.md");
        assert!(content.contains("# idea"));
        assert!(content.contains("## Ideas"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        let _ = fs::remove_dir_all(templates_dir);
        Ok(())
    }

    #[test]
    fn file_browser_can_create_rename_and_delete_directories(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("dir_ops");
        let notes_root = temp_notes_root("dir_ops");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.begin_create_directory();
        app.file_name_input = "projects".to_string();
        app.create_directory()?;
        assert!(notes_root.join("projects").exists());

        app.begin_rename_selected_entry();
        app.file_name_input = "archive".to_string();
        app.rename_selected_entry()?;
        assert!(notes_root.join("archive").exists());
        assert!(!notes_root.join("projects").exists());

        app.begin_delete_selected_entry();
        app.delete_selected_entry()?;
        assert!(!notes_root.join("archive").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_search_finds_nested_entries() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_search");
        let notes_root = temp_notes_root("file_search");
        fs::create_dir_all(notes_root.join("projects/alpha"))?;
        fs::write(notes_root.join("projects/alpha/roadmap.md"), b"# Roadmap")?;
        fs::write(notes_root.join("scratch.md"), b"scratch")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.begin_file_search();
        for ch in "rdmp".chars() {
            app.append_file_search_char(ch);
        }

        assert!(app.has_file_search());
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "roadmap.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
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
    fn file_search_supports_frontmatter_title_and_tag_tokens(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_search_metadata");
        let notes_root = temp_notes_root("file_search_metadata");
        fs::create_dir_all(notes_root.join("projects"))?;
        fs::write(
            notes_root.join("projects/review.md"),
            b"---\ntitle: Sprint Review\ntags: [work, planning]\n---\n# Review\n",
        )?;
        fs::write(
            notes_root.join("personal.md"),
            b"---\ntitle: Weekend Plans\ntags: [personal]\n---\n# Plans\n",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.set_file_search_query("tag:work")?;
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "review.md");

        app.set_file_search_query("title:\"Sprint Review\" -tag:personal")?;
        assert_eq!(app.file_search_results.len(), 1);
        assert_eq!(app.file_search_results[0].name, "review.md");

        app.set_file_search_query("path:projects")?;
        assert!(app
            .file_search_results
            .iter()
            .any(|entry| entry.name == "review.md"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn markdown_files_render_as_terminal_preview() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("markdown_preview");
        let notes_root = temp_notes_root("markdown_preview");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("roadmap.md");
        fs::write(&file_path, b"# Roadmap\n\n- Alpha\n- Beta\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);

        assert!(app.previewed_file_content.contains("# Roadmap"));
        assert!(app.previewed_file_content.contains("- Alpha"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn inline_file_edit_saves_updated_content() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("inline_edit");
        let notes_root = temp_notes_root("inline_edit");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("draft.md");
        fs::write(&file_path, b"# Draft\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.begin_inline_file_edit()?;
        app.file_edit_content = "# Updated\nBody".to_string();
        app.save_inline_file_edit()?;

        assert_eq!(fs::read_to_string(&file_path)?, "# Updated\nBody");
        assert_eq!(app.input_mode, InputMode::ViewingFile);
        assert!(app.previewed_file_content.contains("# Updated"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
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

    #[test]
    fn file_entries_include_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_metadata");
        let notes_root = temp_notes_root("file_metadata");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("info.md");
        fs::write(&file_path, b"12345")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let entry = app
            .file_entries
            .iter()
            .find(|entry| entry.path == file_path)
            .expect("file entry should exist");

        assert_eq!(entry.size_bytes, 5);
        assert!(entry.modified_at.is_some());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_shortcuts_can_pin_dirs_and_save_searches() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("file_shortcuts");
        let notes_root = temp_notes_root("file_shortcuts");
        fs::create_dir_all(notes_root.join("projects"))?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("projects");
        app.toggle_pin_current_directory()?;
        app.set_file_search_query("roadmap")?;
        app.save_current_file_search()?;

        assert_eq!(app.file_shortcuts.len(), 2);
        assert_eq!(app.file_shortcuts[0].kind, FileShortcutKind::Directory);
        assert_eq!(app.file_shortcuts[1].kind, FileShortcutKind::Search);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn file_entries_can_be_moved_to_new_paths() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("move_entry");
        let notes_root = temp_notes_root("move_entry");
        fs::create_dir_all(notes_root.join("inbox"))?;
        fs::create_dir_all(notes_root.join("archive"))?;
        let source = notes_root.join("inbox/note.md");
        fs::write(&source, b"hello")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("inbox");
        app.load_file_entries()?;
        app.select_file_entry_path(&source);
        app.begin_move_selected_entry();
        app.file_name_input = "archive/note.md".to_string();
        app.move_selected_entry()?;

        assert!(!source.exists());
        assert!(notes_root.join("archive/note.md").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn directories_can_be_copied_recursively() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("copy_entry");
        let notes_root = temp_notes_root("copy_entry");
        fs::create_dir_all(notes_root.join("projects/alpha"))?;
        fs::write(notes_root.join("projects/alpha/roadmap.md"), b"roadmap")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.current_dir = notes_root.join("projects");
        app.load_file_entries()?;
        app.select_file_entry_path(&notes_root.join("projects/alpha"));
        app.begin_copy_selected_entry();
        app.file_name_input = "archive/alpha-copy".to_string();
        app.copy_selected_entry()?;

        assert!(notes_root.join("projects/alpha/roadmap.md").exists());
        assert!(notes_root.join("archive/alpha-copy/roadmap.md").exists());

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn note_references_detect_wiki_and_markdown_links() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("note_refs");
        let notes_root = temp_notes_root("note_refs");
        fs::create_dir_all(notes_root.join("projects"))?;
        let target = notes_root.join("projects/roadmap.md");
        fs::write(&target, b"# Roadmap")?;
        let source = notes_root.join("index.md");
        fs::write(
            &source,
            b"See [[projects/roadmap.md]] and [Roadmap](projects/roadmap.md).",
        )?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let refs = app.file_references(&source);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].path, target);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn backlink_discovery_finds_reverse_links() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("backlinks");
        let notes_root = temp_notes_root("backlinks");
        fs::create_dir_all(notes_root.join("topics"))?;
        let target = notes_root.join("topics/alpha.md");
        fs::write(&target, b"# Alpha")?;
        fs::write(notes_root.join("index.md"), b"[[topics/alpha.md]]")?;
        fs::write(notes_root.join("journal.md"), b"[Alpha](topics/alpha.md)")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        let backlinks = app.file_backlinks(&target);

        assert_eq!(backlinks.len(), 2);
        assert_eq!(backlinks[0].label, "/index.md");
        assert_eq!(backlinks[1].label, "/journal.md");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn related_links_can_be_opened_from_current_note() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("open_related");
        let notes_root = temp_notes_root("open_related");
        fs::create_dir_all(notes_root.join("topics"))?;
        let target = notes_root.join("topics/alpha.md");
        fs::write(&target, b"# Alpha")?;
        let source = notes_root.join("index.md");
        fs::write(&source, b"[[topics/alpha.md]]")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.open_file_path(&source)?;
        app.begin_file_links();
        app.open_selected_related_link()?;

        assert_eq!(app.viewed_file_path.as_deref(), Some(target.as_path()));
        assert_eq!(app.input_mode, InputMode::ViewingFile);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn create_or_open_daily_note_uses_daily_template() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("daily_shortcut");
        let notes_root = temp_notes_root("daily_shortcut");
        fs::create_dir_all(&notes_root)?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.create_or_open_daily_note()?;

        let expected = notes_root
            .join("daily")
            .join(format!("{}.md", Local::now().format("%Y-%m-%d")));
        assert_eq!(app.viewed_file_path.as_deref(), Some(expected.as_path()));
        assert!(expected.exists());
        assert!(fs::read_to_string(&expected)?.contains("## Goals"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn external_editor_updates_preview_and_view_content() -> Result<(), Box<dyn std::error::Error>>
    {
        let db_path = temp_db_path("editor");
        let notes_root = temp_notes_root("editor");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("edit.md");
        fs::write(&file_path, b"before")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.editor_command = Some("printf 'after' >".to_string() + " ");
        app.select_file_entry_path(&file_path);
        app.edit_selected_file_in_editor()?;

        assert_eq!(fs::read_to_string(&file_path)?, "after");
        assert_eq!(app.previewed_file_content, "after");
        assert_eq!(app.viewed_file_content, "after");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn opening_or_reselecting_files_resets_scroll_offsets() -> Result<(), Box<dyn std::error::Error>>
    {
        let db_path = temp_db_path("scroll_reset");
        let notes_root = temp_notes_root("scroll_reset");
        fs::create_dir_all(&notes_root)?;
        let file_path = notes_root.join("long.md");
        fs::write(&file_path, b"one\ntwo\nthree\nfour\nfive\nsix\n")?;

        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;
        app.select_file_entry_path(&file_path);
        app.scroll_preview_down(3);
        app.open_file_path(&file_path)?;
        app.scroll_viewed_file_down(4);
        assert_eq!(app.preview_scroll, 0);
        assert_eq!(app.viewed_file_scroll, 4);

        app.select_file_entry_path(&file_path);
        assert_eq!(app.preview_scroll, 0);
        app.open_file_path(&file_path)?;
        assert_eq!(app.viewed_file_scroll, 0);

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }

    #[test]
    fn command_palette_round_trips_mode_and_query() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("command_palette");
        let notes_root = temp_notes_root("command_palette");
        fs::create_dir_all(&notes_root)?;
        let db_path_str = db_path.to_string_lossy().to_string();
        let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

        app.begin_file_search();
        app.command_palette_query = "stale".to_string();
        app.begin_command_palette();

        assert_eq!(app.input_mode, InputMode::CommandPalette);
        assert_eq!(app.command_palette_return_mode, InputMode::SearchingFiles);
        assert!(app.command_palette_query.is_empty());

        app.close_command_palette();
        assert_eq!(app.input_mode, InputMode::SearchingFiles);

        app.record_palette_command("search_files")?;
        app.record_palette_command("help")?;
        app.record_palette_command("search_files")?;
        assert_eq!(app.recent_palette_commands[0], "search_files");
        assert_eq!(app.recent_palette_commands[1], "help");

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_dir_all(notes_root);
        Ok(())
    }
}
