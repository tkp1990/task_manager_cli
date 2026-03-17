use super::common::{temp_db_path, temp_notes_root};
use std::fs;
use task_manager_cli::notes::app::{App, FileShortcutKind, InputMode, NotesView};

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
fn file_browser_can_create_rename_and_delete_directories() -> Result<(), Box<dyn std::error::Error>>
{
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
fn file_search_supports_frontmatter_title_and_tag_tokens() -> Result<(), Box<dyn std::error::Error>>
{
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
fn external_editor_updates_preview_and_view_content() -> Result<(), Box<dyn std::error::Error>> {
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
fn opening_or_reselecting_files_resets_scroll_offsets() -> Result<(), Box<dyn std::error::Error>> {
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
