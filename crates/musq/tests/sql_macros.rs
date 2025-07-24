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
            let conn = connection().await?;
            let val: $ty = $value;

            let row = sql!("SELECT {}", val.clone())?.fetch_one(&conn).await?;
            let out: $ty = row.get_value_idx(0)?;
            assert_eq!(out, val);

            let row = sql!("SELECT {v}", v = val.clone())?
                .fetch_one(&conn)
                .await?;
            let out: $ty = row.get_value_idx(0)?;
            assert_eq!(out, val);

            let list = vec![val.clone(), val.clone()];
            let row = sql!("SELECT {values:list}")?.fetch_one(&conn).await?;
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
    let conn = connection().await?;
    sql!("CREATE TABLE users (id INTEGER, name TEXT, status TEXT)")?
        .execute(&conn)
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
    insert.execute(&conn).await?;

    let row = sql!("SELECT name FROM users WHERE id = {id}")?
        .fetch_one(&conn)
        .await?;
    assert_eq!(row.get_value_idx::<String>(0)?, "Alice");
    Ok(())
}

#[tokio::test]
async fn test_ident_and_lists() -> anyhow::Result<()> {
    let conn = connection().await?;
    let table = "user-data";
    sql!("CREATE TABLE {ident:table} (id INTEGER, name TEXT)")?
        .execute(&conn)
        .await?;
    sql!("INSERT INTO {ident:table} (id, name) VALUES (1, 'a'), (2, 'b')")?
        .execute(&conn)
        .await?;
    let ids = [1, 2];
    let cols = ["id", "name"];
    let rows = sql!("SELECT {idents:cols} FROM {ident:table} WHERE id IN ({values:ids})")?
        .fetch_all(&conn)
        .await?;
    assert_eq!(rows.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_raw_and_taint() -> anyhow::Result<()> {
    let conn = connection().await?;
    sql!("CREATE TABLE t (id INTEGER)")?.execute(&conn).await?;
    sql!("INSERT INTO t (id) VALUES (1), (2)")?
        .execute(&conn)
        .await?;
    let q = sql!("SELECT * FROM t {raw:\"ORDER BY id DESC\"}")?;
    assert!(q.is_tainted());
    let rows = q.fetch_all(&conn).await?;
    assert_eq!(rows.len(), 2);
    Ok(())
}

#[tokio::test]
async fn test_sql_as_and_builder() -> anyhow::Result<()> {
    let conn = connection().await?;
    sql!("CREATE TABLE users (id INTEGER, name TEXT, status TEXT)")?
        .execute(&conn)
        .await?;
    sql!("INSERT INTO users (id, name, status) VALUES (1, 'Alice', 'active')")?
        .execute(&conn)
        .await?;
    let user: User = sql_as!("SELECT id, name, status FROM users WHERE id = {id}", id = 1)?
        .fetch_one(&conn)
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
        .fetch_one(&conn)
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
        .fetch_all(&conn)
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

#[tokio::test]
async fn test_sql_non_result_context() -> anyhow::Result<()> {
    let conn = connection().await?;

    // Test sql! macro in non-result context with unwrap()
    let query = sql!("SELECT 1 as value").unwrap();
    let row = query.fetch_one(&conn).await?;
    let value: i32 = row.get_value_idx(0)?;
    assert_eq!(value, 1);

    // Test sql! macro with parameters in non-result context
    let test_value = 42;
    let query = sql!("SELECT {} as value", test_value).unwrap();
    let row = query.fetch_one(&conn).await?;
    let value: i32 = row.get_value_idx(0)?;
    assert_eq!(value, 42);

    // Test sql! macro with named parameters in non-result context
    let query = sql!("SELECT {val} as value", val = 123).unwrap();
    let row = query.fetch_one(&conn).await?;
    let value: i32 = row.get_value_idx(0)?;
    assert_eq!(value, 123);

    Ok(())
}

#[tokio::test]
async fn test_sql_as_non_result_context() -> anyhow::Result<()> {
    let conn = connection().await?;
    sql!("CREATE TABLE test_table (id INTEGER, name TEXT)")
        .unwrap()
        .execute(&conn)
        .await?;
    sql!("INSERT INTO test_table (id, name) VALUES (1, 'test')")
        .unwrap()
        .execute(&conn)
        .await?;

    // Test sql_as! macro in non-result context with unwrap()
    let query = sql_as!("SELECT id, name FROM test_table WHERE id = {}", 1).unwrap();
    let row: (i32, String) = query.fetch_one(&conn).await?;
    assert_eq!(row, (1, "test".to_string()));

    // Test sql_as! macro with named parameters in non-result context
    let query = sql_as!(
        "SELECT id, name FROM test_table WHERE id = {test_id}",
        test_id = 1
    )
    .unwrap();
    let row: (i32, String) = query.fetch_one(&conn).await?;
    assert_eq!(row, (1, "test".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_sql_result_methods() -> anyhow::Result<()> {
    let conn = connection().await?;

    // Test unwrap_or method
    let query = sql!("SELECT 'default' as value")
        .unwrap_or_else(|_| panic!("Should not error for valid SQL"));
    let row = query.fetch_one(&conn).await?;
    let value: String = row.get_value_idx(0)?;
    assert_eq!(value, "default");

    Ok(())
}

// Helper functions that don't return Results - these are true non-Result contexts
// The fact that these compile proves the macros work in non-Result contexts
fn create_simple_query() {
    let _query = sql!("SELECT 42 as answer").unwrap();
}

fn create_parameterized_query() {
    let value = 42;
    let _query = sql!("SELECT {} as value", value).unwrap();
}

fn create_named_query() {
    let name = "test";
    let _query = sql!("SELECT {name} as greeting", name = name).unwrap();
}

// Test sql_as! in non-result context - provide concrete return types
fn create_sql_as_simple() -> musq::query::Map<impl FnMut(musq::Row) -> musq::Result<(i32, String)>>
{
    sql_as!("SELECT 1 as id, 'hello' as name").unwrap()
}

fn create_sql_as_with_params()
-> musq::query::Map<impl FnMut(musq::Row) -> musq::Result<(i32, String)>> {
    let id = 42;
    let name = "world";
    sql_as!("SELECT {id} as id, {name} as name", id = id, name = name).unwrap()
}

// Alternative approach using a struct that implements FromRow
#[derive(musq::FromRow, Debug, PartialEq)]
struct TestRecord {
    id: i32,
    message: String,
}

fn create_sql_as_with_struct() -> musq::query::Map<impl FnMut(musq::Row) -> musq::Result<TestRecord>>
{
    sql_as!("SELECT 999 as id, 'struct test' as message").unwrap()
}

// Additional test showing sql_as! parameters in non-result context
fn create_sql_as_with_complex_params()
-> musq::query::Map<impl FnMut(musq::Row) -> musq::Result<(i32, String, bool)>> {
    let user_id = 123;
    let status = "active";
    let is_admin = true;
    sql_as!(
        "SELECT {user_id} as id, {status} as status, {is_admin} as is_admin",
        user_id = user_id,
        status = status,
        is_admin = is_admin
    )
    .unwrap()
}

#[tokio::test]
async fn test_sql_macros_compilation_in_non_result_context() -> anyhow::Result<()> {
    // Test sql! macros in non-result contexts
    create_simple_query();
    create_parameterized_query();
    create_named_query();

    // Test sql_as! macros in non-result contexts - these compile successfully
    let query1 = create_sql_as_simple();
    let query2 = create_sql_as_with_params();
    let query3 = create_sql_as_with_struct();
    let query4 = create_sql_as_with_complex_params();

    // Now test that the queries actually work
    let conn = connection().await?;

    // Test basic sql! macro with unwrap
    let query = sql!("SELECT 123 as test_value").unwrap();
    let row = query.fetch_one(&conn).await?;
    let value: i32 = row.get_value_idx(0)?;
    assert_eq!(value, 123);

    // Test the sql_as! queries created in non-result contexts
    let rows: Vec<(i32, String)> = query1.fetch_all(&conn).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], (1, "hello".to_string()));

    let rows: Vec<(i32, String)> = query2.fetch_all(&conn).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], (42, "world".to_string()));

    let rows: Vec<TestRecord> = query3.fetch_all(&conn).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0],
        TestRecord {
            id: 999,
            message: "struct test".to_string()
        }
    );

    let rows: Vec<(i32, String, bool)> = query4.fetch_all(&conn).await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], (123, "active".to_string(), true));

    // Test sql_as! with direct unwrap() and immediate usage
    let rows: Vec<(i32, String)> = sql_as!("SELECT 1 as id, 'direct' as name")
        .unwrap()
        .fetch_all(&conn)
        .await?;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], (1, "direct".to_string()));

    Ok(())
}
