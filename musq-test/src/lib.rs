use musq::{Connection, Musq};

const TEST_SCHEMA: &str = include_str!("setup.sql");

// Make a new connection
pub async fn connection() -> anyhow::Result<Connection> {
    Ok(Connection::connect_with(&Musq::new()).await?)
}

/// Return a connection to a database pre-configured with our test schema.
pub async fn tdb() -> anyhow::Result<Connection> {
    let mut conn = connection().await?;
    musq::query::query(TEST_SCHEMA).execute(&mut conn).await?;
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

// Test type decoding for the simple (unprepared) query API
#[macro_export]
macro_rules! test_unprepared_type {
    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_unprepared_type_ $name >] () -> anyhow::Result<()> {
                use musq::*;
                use futures::TryStreamExt;

                let mut conn = musq_test::connection().await?;

                $(
                    let query = format!("SELECT {}", $text);
                    let mut s = conn.fetch(&*query);
                    let row = s.try_next().await?.unwrap();
                    let rec = row.get_value_idx::<$ty>(0)?;

                    assert_eq!($value, rec);

                    drop(s);
                )+

                Ok(())
            }
        }
    }
}

// Test type encoding and decoding for the prepared query API
#[macro_export]
macro_rules! __test_prepared_type {
    ($name:ident<$ty:ty>($sql:expr, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_type_ $name >] () -> anyhow::Result<()> {
                use musq::Row;

                let mut conn = musq_test::connection().await?;

                $(
                    let query = format!($sql, $text);
                    println!("{query} bound to {:?}", $value);

                    let row = musq::query(&query)
                        .bind($value)
                        .bind($value)
                        .fetch_one(&mut conn)
                        .await?;

                    let matches: i32 = row.get_value_idx(0)?;
                    let returned: $ty = row.get_value_idx(1)?;
                    let round_trip: $ty = row.get_value_idx(2)?;

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
