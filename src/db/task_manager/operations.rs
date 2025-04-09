use diesel::prelude::*;
use diesel::result::Error;
use std::collections::HashSet;

use crate::db::schema::{task, topic};
use crate::db::task_manager::models::{NewTask, NewTopic, Task, TaskUpdate, Topic};
use crate::db::DbPool;

pub struct DbOperations {
    pub pool: DbPool,
    special_topics: HashSet<String>,
}

impl DbOperations {
    pub fn new(pool: DbPool) -> Self {
        let mut special_topics = HashSet::new();
        special_topics.insert("Favourites".to_string());
        special_topics.insert("Default".to_string());

        Self {
            pool,
            special_topics,
        }
    }

    pub fn is_special_topic(&self, name: &str) -> bool {
        self.special_topics.contains(name)
    }

    // Topic Operations
    pub fn load_topics(&self) -> Result<Vec<Topic>, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        topic::table.order_by(topic::id).load::<Topic>(&mut conn)
    }

    pub fn add_topic(&self, name: &str, description: &str) -> Result<Topic, Error> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let new_topic = NewTopic {
            name,
            description,
            created_at: &now,
            updated_at: &now,
        };

        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        diesel::insert_into(topic::table)
            .values(&new_topic)
            .execute(&mut conn)?;

        topic::table
            .order_by(topic::id.desc())
            .limit(1)
            .get_result::<Topic>(&mut conn)
    }

    pub fn delete_topic(&self, topic_id: i32) -> Result<usize, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        // First get the topic to check if it's a special topic
        let topic: Topic = topic::table
            .filter(topic::id.eq(topic_id))
            .first(&mut conn)?;

        if self.is_special_topic(&topic.name) {
            // Don't delete special topics
            return Ok(0);
        }

        diesel::delete(topic::table.find(topic_id)).execute(&mut conn)
    }

    // Task Operations
    pub fn load_tasks(&self, current_topic: &Topic) -> Result<Vec<Task>, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        match current_topic.name.as_str() {
            "Favourites" => task::table
                .filter(task::favourite.eq(true))
                .order_by(task::id)
                .load::<Task>(&mut conn),
            "Default" => task::table.order_by(task::id).load::<Task>(&mut conn),
            _ => task::table
                .filter(task::topic_id.eq(current_topic.id))
                .order_by(task::id)
                .load::<Task>(&mut conn),
        }
    }

    pub fn add_task(&self, topic_id: i32, name: &str, description: &str) -> Result<Task, Error> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let new_task = NewTask {
            topic_id,
            name,
            description,
            completed: false,
            favourite: false,
            created_at: &now,
            updated_at: &now,
        };

        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        diesel::insert_into(task::table)
            .values(&new_task)
            .execute(&mut conn)?;

        task::table
            .order_by(task::id.desc())
            .limit(1)
            .get_result::<Task>(&mut conn)
    }

    pub fn update_task(&self, task_id: i32, update: TaskUpdate) -> Result<Task, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        diesel::update(task::table.find(task_id))
            .set(update)
            .execute(&mut conn)?;

        task::table.find(task_id).get_result::<Task>(&mut conn)
    }

    pub fn toggle_task_completion(&self, task_id: i32) -> Result<Task, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        // Get current task
        let current_task = task::table.find(task_id).get_result::<Task>(&mut conn)?;

        // Create update with toggled completion
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let update = TaskUpdate {
            name: None,
            description: None,
            completed: Some(!current_task.completed),
            favourite: None,
            updated_at: &now,
        };

        // Apply the update
        diesel::update(task::table.find(task_id))
            .set(update)
            .execute(&mut conn)?;

        task::table.find(task_id).get_result::<Task>(&mut conn)
    }

    pub fn toggle_task_favourite(&self, task_id: i32) -> Result<Task, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        // Get current task
        let current_task = task::table
            .filter(task::id.eq(task_id))
            .get_result::<Task>(&mut conn)?;

        // Create update with toggled favourite
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let update = TaskUpdate {
            name: None,
            description: None,
            completed: None,
            favourite: Some(!current_task.favourite),
            updated_at: &now,
        };

        // Apply the update
        diesel::update(task::table.find(task_id))
            .set(update)
            .execute(&mut conn)?;

        // task::table
        //     .filter(task::id.eq(task_id))
        //     .get_result::<Task>(&mut conn)
        task::table.filter(task::id.eq(task_id)).first(&mut conn)
    }

    pub fn delete_task(&self, task_id: i32) -> Result<usize, Error> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get DB connection from pool");

        diesel::delete(task::table.find(task_id)).execute(&mut conn)
    }
}
