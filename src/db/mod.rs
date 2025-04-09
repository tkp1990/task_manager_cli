pub mod schema;
pub mod task_manager;

use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::error::Error;

// Type alias for the database connection pool
pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("src/db/migrations");

/// Initialize the database connection pool
pub fn establish_connection_pool(database_url: &str) -> Result<DbPool, Box<dyn Error>> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create DB connection pool");

    Ok(pool)
}

pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    // Use the `with_output` method to specify how output should be handled
    let _ = conn.run_pending_migrations(MIGRATIONS);
    Ok(())
}
