use super::common::{temp_db_path, temp_notes_root};
use std::fs;
use task_manager_cli::notes::app::{App, InputMode};

#[test]
fn note_filter_keeps_selection_on_visible_match() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("filter");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("filter");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    app.add_note("Alpha", "First body")?;
    app.add_note("Beta", "Second body")?;
    app.selected = 1;
    app.note_filter = "Alpha".to_string();
    app.ensure_selected_visible();

    assert_eq!(app.filtered_note_indices(), vec![1]);
    assert_eq!(app.selected, 1);

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_dir_all(notes_root);
    Ok(())
}

#[test]
fn note_filter_supports_title_and_body_tokens() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("note_filter_tokens");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("note_filter_tokens");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    app.add_note("Project Alpha", "Meeting notes")?;
    app.add_note("Shopping", "Buy apples and bread")?;

    app.note_filter = "title:project".to_string();
    assert_eq!(app.filtered_note_indices(), vec![1]);

    app.note_filter = "body:apples".to_string();
    assert_eq!(app.filtered_note_indices(), vec![0]);

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_dir_all(notes_root);
    Ok(())
}

#[test]
fn note_filter_supports_phrases_and_negation() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("note_filter_phrases");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("note_filter_phrases");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    app.add_note("Project Alpha", "Roadmap review notes")?;
    app.add_note("Shopping", "Buy apples and bread")?;

    app.note_filter = "title:\"Project Alpha\"".to_string();
    assert_eq!(app.filtered_note_indices(), vec![1]);

    app.note_filter = "\"buy apples\" -title:shopping".to_string();
    assert!(app.filtered_note_indices().is_empty());

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_dir_all(notes_root);
    Ok(())
}

#[test]
fn applying_note_preset_sets_filter_and_selection() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("note_preset");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("note_preset");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    app.add_note("Project Alpha", "Meeting notes")?;
    app.add_note("Shopping", "Buy apples and bread")?;
    app.selected = 0;

    app.preset_selected = 2;
    app.apply_selected_note_preset();

    assert_eq!(app.note_filter, "title:shopping");
    assert_eq!(app.filtered_note_indices(), vec![0]);
    assert_eq!(app.selected, 0);

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
