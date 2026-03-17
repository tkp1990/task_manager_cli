use super::common::{temp_db_path, temp_notes_root};
use task_manager_cli::notes::app::App as NotesApp;
use task_manager_cli::task_manager::app::App as TaskManagerApp;

#[test]
fn task_manager_app_supports_basic_task_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("task_manager");
    let db_path_str = db_path.to_string_lossy().to_string();

    let mut app = TaskManagerApp::new(&db_path_str)?;
    app.add_topic("Work")?;
    app.load_topics()?;
    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Work")
        .expect("work topic should exist");

    app.add_task_with_details("Write tests", "Cover the task lifecycle")?;

    assert_eq!(app.tasks.len(), 1);
    assert_eq!(app.tasks[0].name, "Write tests");
    assert_eq!(app.tasks[0].description, "Cover the task lifecycle");
    assert!(!app.tasks[0].completed);
    assert!(!app.tasks[0].favourite);

    app.toggle_task()?;
    assert!(app.tasks[0].completed);

    app.toggle_favourite()?;
    assert!(app.tasks[0].favourite);

    app.load_special_tasks()?;
    assert_eq!(app.favourites_tasks.len(), 1);
    assert_eq!(app.completed_tasks.len(), 1);

    app.delete_task()?;
    assert!(app.tasks.is_empty());

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn notes_app_supports_basic_note_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("notes");
    let db_path_str = db_path.to_string_lossy().to_string();
    let notes_root = temp_notes_root("notes");

    let mut app = NotesApp::new_with_notes_root(&db_path_str, notes_root.clone())?;
    app.add_note("First note", "Draft content")?;

    assert_eq!(app.notes.len(), 1);
    let note_id = app.notes[0].id;
    assert_eq!(app.notes[0].title, "First note");

    app.update_note(note_id, "Updated note", "Final content")?;
    assert_eq!(app.notes[0].title, "Updated note");
    assert_eq!(app.notes[0].content, "Final content");

    app.delete_note()?;
    assert!(app.notes.is_empty());

    let _ = std::fs::remove_file(db_path);
    let _ = std::fs::remove_dir_all(notes_root);
    Ok(())
}
