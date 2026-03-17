use super::common::{temp_db_path, temp_notes_root};
use std::fs;
use task_manager_cli::notes::app::{App, InputMode};

#[test]
fn begin_edit_note_prefills_existing_content() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("begin_edit");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("begin_edit");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    app.add_note("Title", "Body")?;
    app.begin_edit_note();

    assert_eq!(app.input_mode, InputMode::EditingNote);
    assert_eq!(app.title_input, "Title");
    assert_eq!(app.content_input, "Body");
    assert!(app.editing_title);

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_dir_all(notes_root);
    Ok(())
}

#[test]
fn add_note_rejects_blank_titles() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("blank_title");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("blank_title");
    let mut app = App::new_with_notes_root(&db_path_str, notes_root.clone())?;

    let error = app.add_note("   ", "Body").unwrap_err();
    assert!(error.to_string().contains("Note title cannot be empty"));

    let _ = fs::remove_file(db_path);
    let _ = fs::remove_dir_all(notes_root);
    Ok(())
}
