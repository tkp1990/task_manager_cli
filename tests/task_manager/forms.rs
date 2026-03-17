use super::common::temp_db_path;
use task_manager_cli::db::task_manager::models::Topic;
use task_manager_cli::task_manager::app::{App, InputMode};

#[test]
fn begin_add_task_requires_regular_topic() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("begin_add_special");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.topics.push(Topic {
        id: -1,
        name: "Favourites".to_string(),
        description: String::new(),
        created_at: String::new(),
        updated_at: String::new(),
    });
    app.selected_topic = app.topics.len() - 1;
    app.begin_add_task();

    assert_eq!(app.input_mode, InputMode::Normal);
    assert!(app
        .logs
        .last()
        .is_some_and(|entry| entry.contains("regular topic")));

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn begin_add_and_edit_task_manage_form_state() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("form_state");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.begin_add_task();
    assert_eq!(app.input_mode, InputMode::AddingTaskName);

    app.task_name_input = "Draft".to_string();
    app.task_description_input = "Body".to_string();
    app.add_task_with_details(
        &app.task_name_input.clone(),
        &app.task_description_input.clone(),
    )?;

    app.begin_edit_task();
    assert_eq!(app.input_mode, InputMode::EditingTaskName);
    assert_eq!(app.task_name_input, "Draft");
    assert_eq!(app.task_description_input, "Body");

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn reset_task_inputs_clears_inline_feedback() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("reset_feedback");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.task_name_input = "Draft".to_string();
    app.task_description_input = "Body".to_string();
    app.set_task_form_message("Task name cannot be empty");

    app.reset_task_inputs();

    assert!(app.task_name_input.is_empty());
    assert!(app.task_description_input.is_empty());
    assert!(app.task_form_message.is_none());

    let _ = std::fs::remove_file(db_path);
    Ok(())
}
