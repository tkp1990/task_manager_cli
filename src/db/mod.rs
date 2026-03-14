pub mod notes;
pub mod schema;
pub mod task_manager;

use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::error::Error;
use std::path::PathBuf;

// Type alias for the database connection pool
pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("src/db/migrations");

/// Initialize the database connection pool
pub fn establish_connection_pool(database_url: &str) -> Result<DbPool, Box<dyn Error>> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder().build(manager)?;

    Ok(pool)
}

pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| -> Box<dyn std::error::Error> { e })?;
    Ok(())
}

pub fn resolve_db_path(
    dir_var: &str,
    default_dir: &str,
    filename_var: &str,
    default_filename: &str,
) -> PathBuf {
    let db_dir = std::env::var(dir_var).unwrap_or_else(|_| default_dir.to_string());
    let db_filename = std::env::var(filename_var).unwrap_or_else(|_| default_filename.to_string());

    PathBuf::from(db_dir).join(db_filename)
}

#[cfg(test)]
mod tests {
    use crate::notes::app::App as NotesApp;
    use crate::task_manager::app::App as TaskManagerApp;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        std::env::temp_dir().join(format!("task_manager_cli_{unique}.db"))
    }

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

        let _ = fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn notes_app_supports_basic_note_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("notes");
        let db_path_str = db_path.to_string_lossy().to_string();

        let mut app = NotesApp::new(&db_path_str)?;
        app.add_note("First note", "Draft content")?;

        assert_eq!(app.notes.len(), 1);
        let note_id = app.notes[0].id;
        assert_eq!(app.notes[0].title, "First note");

        app.update_note(note_id, "Updated note", "Final content")?;
        assert_eq!(app.notes[0].title, "Updated note");
        assert_eq!(app.notes[0].content, "Final content");

        app.delete_note()?;
        assert!(app.notes.is_empty());

        let _ = fs::remove_file(db_path);
        Ok(())
    }
}
