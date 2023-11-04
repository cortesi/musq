use musqlite::{
    pool::{Pool, PoolOptions},
    Connection,
};

const TEST_SCHEMA: &str = include_str!("setup.sql");

pub fn setup_if_needed() {
    let _ = env_logger::builder().is_test(true).try_init();
}

// Make a new connection
pub async fn connection() -> anyhow::Result<Connection> {
    setup_if_needed();
    Ok(Connection::connect("sqlite::memory:").await?)
}

// Make a new pool
// Ensure [dotenvy] and [env_logger] have been setup
pub async fn pool() -> anyhow::Result<Pool> {
    setup_if_needed();
    let pool = PoolOptions::new()
        .min_connections(0)
        .max_connections(5)
        .connect("sqlite::memory:")
        .await?;
    Ok(pool)
}

/// Return a connection to a database pre-configured with our test schema.
pub async fn tdb() -> anyhow::Result<Connection> {
    let mut conn = connection().await?;
    musqlite::query::query(TEST_SCHEMA)
        .execute(&mut conn)
        .await?;
    Ok(conn)
}

// Test type encoding and decoding
#[macro_export]
macro_rules! test_type {
    ($name:ident<$ty:ty>($sql:literal, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$ty>($sql, $($text == $value),+));
        $crate::test_unprepared_type!($name<$ty>($($text == $value),+));
    };

    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            $crate::__test_prepared_type!($name<$ty>($crate::[< query_for_test_prepared_type >]!(), $($text == $value),+));
            $crate::test_unprepared_type!($name<$ty>($($text == $value),+));
        }
    };

    ($name:ident($($text:literal == $value:expr),+ $(,)?)) => {
        $crate::test_type!($name<$name>($($text == $value),+));
    };
}

// Test type decoding only
#[macro_export]
macro_rules! test_decode_type {
    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_decode_type!($name<$ty>($($text == $value),+));
        $crate::test_unprepared_type!($name<$ty>($($text == $value),+));
    };

    ($name:ident($($text:literal == $value:expr),+ $(,)?)) => {
        $crate::test_decode_type!($name<$name>($($text == $value),+));
    };
}

// Test type encoding and decoding
#[macro_export]
macro_rules! test_prepared_type {
    ($name:ident<$ty:ty>($sql:literal, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$ty>($sql, $($text == $value),+));
    };

    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            $crate::__test_prepared_type!($name<$ty>($crate::[< query_for_test_prepared_type >]!(), $($text == $value),+));
        }
    };

    ($name:ident($($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$name>($($text == $value),+));
    };
}

// Test type decoding for the simple (unprepared) query API
#[macro_export]
macro_rules! test_unprepared_type {
    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_unprepared_type_ $name >] () -> anyhow::Result<()> {
                use musqlite::*;
                use futures::TryStreamExt;

                let mut conn = musqlite_test::connection().await?;

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
    ($name:ident<$ty:ty>( $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_decode_type_ $name >] () -> anyhow::Result<()> {
                use Row;

                let mut conn = musqlite_test::new().await?;

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
    ($name:ident<$ty:ty>($sql:expr, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_type_ $name >] () -> anyhow::Result<()> {
                use musqlite::Row;

                let mut conn = musqlite_test::connection().await?;

                $(
                    let query = format!($sql, $text);
                    println!("{query}");

                    let row = musqlite::query(&query)
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
macro_rules! query_for_test_prepared_type {
    () => {
        "SELECT {0} is ?, {0}, ?"
    };
}
