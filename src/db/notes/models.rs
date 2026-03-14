use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::note;

#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = note)]
pub struct Note {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = note)]
pub struct NewNote<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = note)]
pub struct NoteUpdate<'a> {
    pub title: Option<&'a str>,
    pub content: Option<&'a str>,
    pub updated_at: &'a str,
}
