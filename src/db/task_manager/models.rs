use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::db::schema::{task, topic};

#[derive(Debug, Clone, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = topic)]
pub struct Topic {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = topic)]
pub struct NewTopic<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(
    Debug, Clone, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize,
)]
#[diesel(table_name = task)]
#[diesel(belongs_to(Topic))]
pub struct Task {
    pub id: i32,
    pub topic_id: i32,
    pub name: String,
    pub description: String,
    pub completed: bool,
    pub favourite: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = task)]
pub struct NewTask<'a> {
    pub topic_id: i32,
    pub name: &'a str,
    pub description: &'a str,
    pub completed: bool,
    pub favourite: bool,
    pub created_at: &'a str,
    pub updated_at: &'a str,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = task)]
pub struct TaskUpdate<'a> {
    pub name: Option<&'a str>,
    pub description: Option<&'a str>,
    pub completed: Option<bool>,
    pub favourite: Option<bool>,
    pub updated_at: &'a str,
}
