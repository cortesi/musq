use musqlite_core::database::Database;
use musqlite_core::Connection;

use musqlite_core::pool::{Pool, PoolOptions};
use musqlite_core::sqlite::Sqlite;

const TEST_SCHEMA: &str = include_str!("setup.sql");

pub fn setup_if_needed() {
    let _ = env_logger::builder().is_test(true).try_init();
}

// Make a new connection
// Ensure [dotenvy] and [env_logger] have been setup
pub async fn new<DB>() -> anyhow::Result<Connection>
where
    DB: Database,
{
    setup_if_needed();
    Ok(Connection::connect("sqlite::memory:").await?)
}

// Make a new pool
// Ensure [dotenvy] and [env_logger] have been setup
pub async fn pool<DB>() -> anyhow::Result<Pool>
where
    DB: Database,
{
    setup_if_needed();
    let pool = PoolOptions::new()
        .min_connections(0)
        .max_connections(5)
        .test_before_acquire(true)
        .connect("sqlite::memory:")
        .await?;
    Ok(pool)
}

/// Return a connection to a database pre-configured with our test schema.
pub async fn tdb() -> anyhow::Result<Connection> {
    let mut conn = new::<Sqlite>().await?;
    musqlite_core::query::query(TEST_SCHEMA)
        .execute(&mut conn)
        .await?;
    Ok(conn)
}

// Test type encoding and decoding
#[macro_export]
macro_rules! test_type {
    ($name:ident<$ty:ty>($db:ident, $sql:literal, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$ty>($db, $sql, $($text == $value),+));
        $crate::test_unprepared_type!($name<$ty>($db, $($text == $value),+));
    };

    ($name:ident<$ty:ty>($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            $crate::__test_prepared_type!($name<$ty>($db, $crate::[< $db _query_for_test_prepared_type >]!(), $($text == $value),+));
            $crate::test_unprepared_type!($name<$ty>($db, $($text == $value),+));
        }
    };

    ($name:ident($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::test_type!($name<$name>($db, $($text == $value),+));
    };
}

// Test type decoding only
#[macro_export]
macro_rules! test_decode_type {
    ($name:ident<$ty:ty>($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_decode_type!($name<$ty>($db, $($text == $value),+));
        $crate::test_unprepared_type!($name<$ty>($db, $($text == $value),+));
    };

    ($name:ident($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::test_decode_type!($name<$name>($db, $($text == $value),+));
    };
}

// Test type encoding and decoding
#[macro_export]
macro_rules! test_prepared_type {
    ($name:ident<$ty:ty>($db:ident, $sql:literal, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$ty>($db, $sql, $($text == $value),+));
    };

    ($name:ident<$ty:ty>($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            $crate::__test_prepared_type!($name<$ty>($db, $crate::[< $db _query_for_test_prepared_type >]!(), $($text == $value),+));
        }
    };

    ($name:ident($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$name>($db, $($text == $value),+));
    };
}

// Test type decoding for the simple (unprepared) query API
#[macro_export]
macro_rules! test_unprepared_type {
    ($name:ident<$ty:ty>($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_unprepared_type_ $name >] () -> anyhow::Result<()> {
                use musqlite_core::*;
                use futures::TryStreamExt;

                let mut conn = musqlite_test::new::<$db>().await?;

                $(
                    let query = format!("SELECT {}", $text);
                    let mut s = conn.fetch(&*query);
                    let row = s.try_next().await?.unwrap();
                    let rec = row.try_get::<$ty, _>(0)?;

                    assert_eq!($value, rec);

                    drop(s);
                )+

                Ok(())
            }
        }
    }
}

// Test type decoding only for the prepared query API
#[macro_export]
macro_rules! __test_prepared_decode_type {
    ($name:ident<$ty:ty>($db:ident, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_decode_type_ $name >] () -> anyhow::Result<()> {
                use Row;

                let mut conn = musqlite_test::new::<$db>().await?;

                $(
                    let query = format!("SELECT {}", $text);

                    let row = query(&query)
                        .fetch_one(&mut conn)
                        .await?;

                    let rec: $ty = row.try_get(0)?;

                    assert_eq!($value, rec);
                )+

                Ok(())
            }
        }
    };
}

// Test type encoding and decoding for the prepared query API
#[macro_export]
macro_rules! __test_prepared_type {
    ($name:ident<$ty:ty>($db:ident, $sql:expr, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_type_ $name >] () -> anyhow::Result<()> {
                use musqlite_core::Row;

                let mut conn = musqlite_test::new::<$db>().await?;

                $(
                    let query = format!($sql, $text);
                    println!("{query}");

                    let row = musqlite_core::query(&query)
                        .bind($value)
                        .bind($value)
                        .fetch_one(&mut conn)
                        .await?;

                    let matches: i32 = row.try_get(0)?;
                    let returned: $ty = row.try_get(1)?;
                    let round_trip: $ty = row.try_get(2)?;

                    assert!(matches != 0,
                            "[1] DB value mismatch; given value: {:?}\n\
                             as returned: {:?}\n\
                             round-trip: {:?}",
                            $value, returned, round_trip);

                    assert_eq!($value, returned,
                            "[2] DB value mismatch; given value: {:?}\n\
                                     as returned: {:?}\n\
                                     round-trip: {:?}",
                                    $value, returned, round_trip);

                    assert_eq!($value, round_trip,
                            "[3] DB value mismatch; given value: {:?}\n\
                                     as returned: {:?}\n\
                                     round-trip: {:?}",
                                    $value, returned, round_trip);
                )+

                Ok(())
            }
        }
    };
}

#[macro_export]
macro_rules! MySql_query_for_test_prepared_type {
    () => {
        // MySQL 8.0.27 changed `<=>` to return an unsigned integer
        "SELECT CAST({0} <=> ? AS SIGNED INTEGER), {0}, ?"
    };
}

#[macro_export]
macro_rules! Mssql_query_for_test_prepared_type {
    () => {
        "SELECT CASE WHEN {0} IS NULL AND @p1 IS NULL THEN 1 WHEN {0} = @p1 THEN 1 ELSE 0 END, {0}, @p2"
    };
}

#[macro_export]
macro_rules! Sqlite_query_for_test_prepared_type {
    () => {
        "SELECT {0} is ?, {0}, ?"
    };
}

#[macro_export]
macro_rules! Postgres_query_for_test_prepared_type {
    () => {
        "SELECT ({0} is not distinct from $1)::int4, {0}, $2"
    };
}
