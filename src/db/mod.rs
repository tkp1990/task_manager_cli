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
