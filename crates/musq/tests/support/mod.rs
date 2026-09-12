use std::env;

use musq::{Connection, Musq};

/// Shared database fixtures.
///
/// Not every test binary uses these, so allow the items to go unused.
#[allow(dead_code)]
pub mod db;

/// Create a new connection for tests.
pub async fn connection() -> anyhow::Result<Connection> {
    Ok(Connection::connect_with(&Musq::new()).await?)
}

/// Assert that an error is a configuration error containing `expected`.
///
/// Not every test binary uses this, so allow the item to go unused.
#[allow(dead_code)]
pub fn assert_configuration_contains(error: musq::Error, expected: &str) {
    match error {
        musq::Error::Configuration(message) => assert!(
            message.contains(expected),
            "configuration error {message:?} did not contain {expected:?}"
        ),
        other => panic!("expected configuration error containing {expected:?}, got {other:?}"),
    }
}

/// Return the iteration count for a stress loop.
///
/// Uses `default` for a normal run and ten times that when `MUSQ_STRESS` is
/// set.
#[allow(dead_code)]
pub fn stress_iters(default: usize) -> usize {
    match env::var("MUSQ_STRESS") {
        Ok(_) => default.saturating_mul(10),
        Err(_) => default,
    }
}

/// Test type encoding and decoding using both prepared and unprepared queries.
#[macro_export]
macro_rules! test_type {
    ($name:ident<$ty:ty>($sql:literal, $($text:literal == $value:expr),+ $(,)?)) => {
        $crate::__test_prepared_type!($name<$ty>($sql, $($text == $value),+));
        $crate::test_unprepared_type!($name<$ty>($($text == $value),+));
    };

    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            $crate::__test_prepared_type!($name<$ty>($crate::query_for_test_prepared_type!(), $($text == $value),+));
            $crate::test_unprepared_type!($name<$ty>($($text == $value),+));
        }
    };

    ($name:ident($($text:literal == $value:expr),+ $(,)?)) => {
        $crate::test_type!($name<$name>($($text == $value),+));
    };
}

/// Test type decoding for the simple (unprepared) query API.
#[macro_export]
macro_rules! test_unprepared_type {
    ($name:ident<$ty:ty>($($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_unprepared_type_ $name >] () -> anyhow::Result<()> {
                use futures::TryStreamExt;

                let conn = $crate::support::connection().await?;

                $(
                    let query_str = format!("SELECT {}", $text);
                    let mut s = musq::query(&query_str).fetch(&conn);
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

/// Test type encoding and decoding for the prepared query API.
#[macro_export]
macro_rules! __test_prepared_type {
    ($name:ident<$ty:ty>($sql:expr, $($text:literal == $value:expr),+ $(,)?)) => {
        paste::item! {
            #[tokio::test]
            async fn [< test_prepared_type_ $name >] () -> anyhow::Result<()> {


                let conn = $crate::support::connection().await?;

                $(
                    let query = format!($sql, $text);
                    println!("{query} bound to {:?}", $value);

                    let row = musq::query(&query)
                        .bind($value)
                        .bind($value)
                        .fetch_one(&conn)
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

/// Provide the SQL template used by prepared-type tests.
#[macro_export]
macro_rules! query_for_test_prepared_type {
    () => {
        "SELECT {0} is ?, {0}, ?"
    };
}
