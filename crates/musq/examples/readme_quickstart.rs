use musq::{FromRow, Musq, sql, sql_as};

#[derive(Debug, FromRow)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> musq::Result<()> {
    // Create an in-memory database pool
    let pool = Musq::new().open_in_memory().await?;

    // Create a table
    sql!("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")?
        .execute(&pool)
        .await?;

    // Insert a user
    let id = 1;
    let name = "Alice";
    sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
        .execute(&pool)
        .await?;

    // Fetch the user and map it to our struct
    let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
        .fetch_one(&pool)
        .await?;

    println!("Fetched user: {user:?}");

    Ok(())
}
