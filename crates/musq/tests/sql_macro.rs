use musq::{FromRow, sql, sql_as};
use musq_test::tdb;

#[derive(Debug, PartialEq, FromRow)]
struct User {
    id: i32,
    name: String,
}

#[tokio::test]
async fn basic_sql_macro() -> anyhow::Result<()> {
    let mut conn = tdb().await?;
    musq::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)")
        .execute(&mut conn)
        .await?;
    musq::query("INSERT INTO users (id, name) VALUES (1, 'Alice')")
        .execute(&mut conn)
        .await?;

    let id = 1;
    let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(user.id, 1);
    assert_eq!(user.name, "Alice");

    let q = sql!("SELECT * FROM users WHERE id = {}", 1)?;
    let row = q.fetch_one(&mut conn).await?;
    let name: String = row.get_value_idx(1)?;
    assert_eq!(name, "Alice");
    Ok(())
}
