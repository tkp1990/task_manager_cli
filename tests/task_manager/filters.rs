use super::common::temp_db_path;
use task_manager_cli::task_manager::app::{App, InputMode};

#[test]
fn task_filter_repositions_selection_to_visible_result() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("task_filter_selection");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.add_task_with_details("Alpha", "Open work")?;
    app.add_task_with_details("Beta", "Closed work")?;
    app.selected = 1;

    app.task_filter = "alpha".to_string();
    app.ensure_selected_visible();

    assert_eq!(app.filtered_task_indices(), vec![0]);
    assert_eq!(app.selected, 0);

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn special_task_filter_repositions_selection_to_visible_result(
) -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("special_filter_selection");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.add_task_with_details("Alpha", "Open work")?;
    app.add_task_with_details("Beta", "Closed work")?;
    app.selected = 0;
    app.toggle_favourite()?;
    app.selected = 1;
    app.toggle_favourite()?;
    app.load_special_tasks()?;
    app.special_task_selected = 1;

    app.special_task_filter = "alpha".to_string();
    app.ensure_special_selection_visible();

    assert_eq!(app.filtered_special_task_indices(), vec![0]);
    assert_eq!(app.special_task_selected, 0);

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn task_filter_supports_status_topic_and_favourite_tokens() -> Result<(), Box<dyn std::error::Error>>
{
    let db_path = temp_db_path("task_filter_tokens");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.add_task_with_details("Alpha", "Open work")?;
    app.add_task_with_details("Beta", "Closed work")?;

    app.selected = 0;
    app.toggle_favourite()?;
    app.selected = 1;
    app.toggle_task()?;

    app.task_filter = "fav:true".to_string();
    assert_eq!(app.filtered_task_indices(), vec![0]);

    app.task_filter = "status:done".to_string();
    assert_eq!(app.filtered_task_indices(), vec![1]);

    app.task_filter = "topic:default status:open".to_string();
    assert_eq!(app.filtered_task_indices(), vec![0]);

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn task_filter_supports_phrases_and_negation() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("task_filter_phrases");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.add_task_with_details("Project Alpha", "Roadmap review")?;
    app.add_task_with_details("Shopping", "Buy apples")?;

    app.task_filter = "\"project alpha\"".to_string();
    assert_eq!(app.filtered_task_indices(), vec![0]);

    app.task_filter = "apples -topic:default".to_string();
    assert!(app.filtered_task_indices().is_empty());

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn command_palette_round_trips_mode_and_query() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("command_palette");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.begin_task_filter();
    app.command_palette_query = "stale".to_string();
    app.begin_command_palette();

    assert_eq!(app.input_mode, InputMode::CommandPalette);
    assert_eq!(app.command_palette_return_mode, InputMode::Filtering);
    assert!(app.command_palette_query.is_empty());

    app.close_command_palette();
    assert_eq!(app.input_mode, InputMode::Filtering);

    app.record_palette_command("filter")?;
    app.record_palette_command("help")?;
    app.record_palette_command("filter")?;
    assert_eq!(app.recent_palette_commands[0], "filter");
    assert_eq!(app.recent_palette_commands[1], "help");

    let _ = std::fs::remove_file(db_path);
    Ok(())
}
