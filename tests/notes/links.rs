use super::common::{temp_db_path, temp_notes_root};
use std::fs;
use task_manager_cli::notes::app::{App, InputMode};

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
