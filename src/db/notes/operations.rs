use diesel::prelude::*;
use std::error::Error;

use crate::db::notes::models::{NewNote, Note, NoteUpdate};
use crate::db::schema::note;
use crate::db::DbPool;

pub struct DbOperations {
    pub pool: DbPool,
}

impl DbOperations {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn get_conn(
        &self,
    ) -> Result<
        diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<SqliteConnection>>,
        Box<dyn Error>,
    > {
        Ok(self.pool.get()?)
    }

    // Note Operations
    pub fn load_notes(&self) -> Result<Vec<Note>, Box<dyn Error>> {
        let mut conn = self.get_conn()?;

        Ok(note::table
            .order_by(note::id.desc())
            .load::<Note>(&mut conn)?)
    }

    pub fn add_note(&self, title: &str, content: &str) -> Result<Note, Box<dyn Error>> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let new_note = NewNote {
            title,
            content,
            created_at: &now,
            updated_at: &now,
        };

        let mut conn = self.get_conn()?;

        diesel::insert_into(note::table)
            .values(&new_note)
            .execute(&mut conn)?;

        Ok(note::table
            .order_by(note::id.desc())
            .limit(1)
            .get_result::<Note>(&mut conn)?)
    }

    pub fn update_note(&self, note_id: i32, update: NoteUpdate) -> Result<Note, Box<dyn Error>> {
        let mut conn = self.get_conn()?;

        diesel::update(note::table.find(note_id))
            .set(update)
            .execute(&mut conn)?;

        Ok(note::table.find(note_id).get_result::<Note>(&mut conn)?)
    }

    pub fn delete_note(&self, note_id: i32) -> Result<usize, Box<dyn Error>> {
        let mut conn = self.get_conn()?;

        Ok(diesel::delete(note::table.find(note_id)).execute(&mut conn)?)
    }

    pub fn get_note(&self, note_id: i32) -> Result<Note, Box<dyn Error>> {
        let mut conn = self.get_conn()?;

        Ok(note::table.find(note_id).get_result::<Note>(&mut conn)?)
    }

    pub fn find_note(&self, note_id: i32) -> Result<Option<Note>, Box<dyn Error>> {
        let mut conn = self.get_conn()?;

        Ok(note::table
            .find(note_id)
            .first::<Note>(&mut conn)
            .optional()?)
    }
}
