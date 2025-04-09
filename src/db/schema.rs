// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;

    task (id) {
        id -> Integer,
        topic_id -> Integer,
        name -> Text,
        description -> Text,
        completed -> Bool,
        favourite -> Bool,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    topic (id) {
        id -> Integer,
        name -> Text,
        description -> Text,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::joinable!(task -> topic (topic_id));

diesel::allow_tables_to_appear_in_same_query!(
    task,
    topic,
);
