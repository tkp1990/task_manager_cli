use super::common::{temp_db_path, temp_notes_root};
use chrono::Local;
use std::fs;
use task_manager_cli::notes::app::App;

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
