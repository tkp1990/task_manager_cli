--- Create the topic table
CREATE TABLE IF NOT EXISTS topic (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Create the task table
CREATE TABLE IF NOT EXISTS task (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    topic_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    completed BOOLEAN NOT NULL DEFAULT 0,
    favourite BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(topic_id) REFERENCES topic(id)
);

-- Insert default topics
INSERT INTO topic (name, description, created_at, updated_at)
VALUES 
    ('Favourites', 'Favourite tasks', datetime('now'), datetime('now')),
    ('Default', 'All tasks', datetime('now'), datetime('now'));
