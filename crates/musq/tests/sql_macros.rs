use musq::types::time::{Date, OffsetDateTime, PrimitiveDateTime, Time};
use musq::*;
use musq_test::connection;
use time::macros::{date, datetime, time};

#[derive(FromRow, Debug, PartialEq)]
struct User {
    id: i32,
    name: String,
    status: String,
}

macro_rules! bind_check {
    ($name:ident: $ty:ty = $value:expr) => {
        #[tokio::test]
        async fn $name() -> anyhow::Result<()> {
            let mut conn = connection().await?;
            let val: $ty = $value;

            let row = sql!("SELECT {}", val.clone())?.fetch_one(&mut conn).await?;
            let out: $ty = row.get_value_idx(0)?;
            assert_eq!(out, val);

            let row = sql!("SELECT {v}", v = val.clone())?
                .fetch_one(&mut conn)
                .await?;
            let out: $ty = row.get_value_idx(0)?;
            assert_eq!(out, val);

            let list = vec![val.clone(), val.clone()];
            let row = sql!("SELECT {values:list}")?.fetch_one(&mut conn).await?;
            let out0: $ty = row.get_value_idx(0)?;
            let out1: $ty = row.get_value_idx(1)?;
            assert_eq!(out0, val);
            assert_eq!(out1, val);
            Ok(())
        }
    };
}

#[tokio::test]
async fn test_placeholders() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    sql!("CREATE TABLE users (id INTEGER, name TEXT, status TEXT)")?
        .execute(&mut conn)
        .await?;

    let id = 1;
    let name = "Alice";
    // positional and named
    let insert = sql!(
        "INSERT INTO users (id, name, status) VALUES ({}, {}, 'active')",
        id,
        name
    )?;
    println!("insert sql: {}", insert.sql());
    insert.execute(&mut conn).await?;

    let row = sql!("SELECT name FROM users WHERE id = {id}")?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.get_value_idx::<String>(0)?, "Alice");
    Ok(())
}

#[tokio::test]
async fn test_ident_and_lists() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let table = "user-data";
    sql!("CREATE TABLE {ident:table} (id INTEGER, name TEXT)")?
        .execute(&mut conn)
        .await?;
    sql!("INSERT INTO {ident:table} (id, name) VALUES (1, 'a'), (2, 'b')")?
        .execute(&mut conn)
        .await?;
    let ids = [1, 2];
    let cols = ["id", "name"];
    let rows = sql!("SELECT {idents:cols} FROM {ident:table} WHERE id IN ({values:ids})")?
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(rows.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_raw_and_taint() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    sql!("CREATE TABLE t (id INTEGER)")?
        .execute(&mut conn)
        .await?;
    sql!("INSERT INTO t (id) VALUES (1), (2)")?
        .execute(&mut conn)
        .await?;
    let q = sql!("SELECT * FROM t {raw:\"ORDER BY id DESC\"}")?;
    assert!(q.is_tainted());
    let rows = q.fetch_all(&mut conn).await?;
    assert_eq!(rows.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_sql_as_and_builder() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    sql!("CREATE TABLE users (id INTEGER, name TEXT, status TEXT)")?
        .execute(&mut conn)
        .await?;
    sql!("INSERT INTO users (id, name, status) VALUES (1, 'Alice', 'active')")?
        .execute(&mut conn)
        .await?;
    let user: User = sql_as!("SELECT id, name, status FROM users WHERE id = {id}", id = 1)?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(
        user,
        User {
            id: 1,
            name: "Alice".into(),
            status: "active".into()
        }
    );
    let tuple: (i32, String) = sql_as!("SELECT id, name FROM users WHERE id = {id}", id = 1)?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(tuple, (1, "Alice".into()));

    let base = sql!(
        "SELECT id FROM users WHERE status = {status}",
        status = "active"
    )?;
    let final_q = {
        let mut b = base.into_builder();
        b.push_sql(" ORDER BY id");
        b.build()
    };
    let ids: Vec<i32> = final_q
        .try_map(|row| row.get_value_idx::<i32>(0))
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(ids, vec![1]);
    Ok(())
}

bind_check!(bind_bool: bool = true);
bind_check!(bind_i8: i8 = -5);
bind_check!(bind_i16: i16 = -1234);
bind_check!(bind_i32: i32 = 42);
bind_check!(bind_i64: i64 = 9001);
bind_check!(bind_u8: u8 = 5);
bind_check!(bind_u16: u16 = 55);
bind_check!(bind_u32: u32 = 999);
bind_check!(bind_f32: f32 = std::f32::consts::PI);
bind_check!(bind_f64: f64 = std::f64::consts::E);
bind_check!(bind_string: String = "hello".to_string());
bind_check!(bind_bytes: Vec<u8> = vec![1u8, 2, 3]);
bind_check!(bind_offset_datetime: OffsetDateTime = datetime!(2025 - 7 - 22 6:20:47 UTC));
bind_check!(bind_primitive_datetime: PrimitiveDateTime = datetime!(2025 - 1 - 15 12:30:45));
bind_check!(bind_date: Date = date!(2025 - 1 - 1));
bind_check!(bind_time: Time = time!(23:59:59));
