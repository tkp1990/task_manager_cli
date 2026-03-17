use super::common::temp_db_path;
use task_manager_cli::task_manager::app::App;

#[test]
fn applying_task_preset_sets_filter_and_selection() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("task_preset");
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

    app.preset_selected = 0;
    app.apply_selected_task_preset();

    assert_eq!(app.task_filter, "status:open");
    assert_eq!(app.filtered_task_indices(), vec![0, 1]);

    let _ = std::fs::remove_file(db_path);
    Ok(())
}

#[test]
fn applying_special_task_preset_sets_special_filter() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path("special_task_preset");
    let db_path_str = db_path.to_string_lossy().to_string();
    let mut app = App::new(&db_path_str)?;

    app.selected_topic = app
        .topics
        .iter()
        .position(|topic| topic.name == "Default")
        .expect("default topic should exist");
    app.add_task_with_details("Alpha", "Open work")?;
    app.selected = 0;
    app.toggle_favourite()?;
    app.load_special_tasks()?;

    app.preset_selected = 2;
    app.apply_selected_special_task_preset();

    assert_eq!(app.special_task_filter, "fav:true");

    let _ = std::fs::remove_file(db_path);
    Ok(())
}
