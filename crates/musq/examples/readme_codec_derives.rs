#![allow(dead_code)]

use musq::{Musq, sql, sql_as, FromRow};

// START - Type Handling section (Codec derive)
// Enum as TEXT (snake_case strings)
#[derive(musq::Codec, Debug, PartialEq)]
enum Status {
    Open,
    Closed,
}

// Enum as INTEGER
#[derive(musq::Codec, Debug, PartialEq)]
#[musq(repr = "i32")]
enum Priority {
    Low = 1,
    Medium = 2,
    High = 3,
}

// Newtype struct
#[derive(musq::Codec, Debug, PartialEq)]
struct UserId(i32);
// END - Type Handling section (Codec derive)

#[derive(FromRow, Debug)]
struct Task {
    id: i32,
    title: String,
    status: Status,
    priority: Priority,
    user_id: UserId,
}

#[tokio::main]
async fn main() -> musq::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    // Create table
    sql!("CREATE TABLE tasks (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        status TEXT NOT NULL,
        priority INTEGER NOT NULL,
        user_id INTEGER NOT NULL
    );")?
    .execute(&pool)
    .await?;

    // Insert data using the derived types
    let task_id = 1;
    let title = "Complete project";
    let status = Status::Open;
    let priority = Priority::High;
    let user_id = UserId(42);

    sql!("INSERT INTO tasks (id, title, status, priority, user_id) 
          VALUES ({task_id}, {title}, {status}, {priority}, {user_id})")?
        .execute(&pool)
        .await?;

    // Query back and verify the types work correctly
    let task: Task = sql_as!("SELECT id, title, status, priority, user_id FROM tasks WHERE id = {task_id}")?
        .fetch_one(&pool)
        .await?;

    println!("Retrieved task: {task:?}");

    // Verify the values are correct
    assert_eq!(task.status, Status::Open);
    assert_eq!(task.priority, Priority::High);
    assert_eq!(task.user_id, UserId(42));

    println!("All Codec derives working correctly!");

    Ok(())
}