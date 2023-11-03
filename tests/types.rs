extern crate time_ as time;

use musqlite_core::row::Row;
use musqlite_core::sqlite::{Sqlite, SqliteRow};
use musqlite_test::new;
use musqlite_test::test_type;

test_type!(null<Option<i32>>(Sqlite,
    "NULL" == None::<i32>
));

test_type!(bool(Sqlite, "FALSE" == false, "TRUE" == true));

test_type!(i32(Sqlite, "94101" == 94101_i32));

test_type!(i64(Sqlite, "9358295312" == 9358295312_i64));

// NOTE: This behavior can be surprising. Floating-point parameters are widening to double which can
//       result in strange rounding.
test_type!(f32(Sqlite, "3.1410000324249268" == 3.141f32 as f64 as f32));

test_type!(f64(Sqlite, "939399419.1225182" == 939399419.1225182_f64));

test_type!(str<String>(Sqlite,
    "'this is foo'" == "this is foo",
    "cast(x'7468697320006973206E756C2D636F6E7461696E696E67' as text)" == "this \0is nul-containing",
    "''" == ""
));

test_type!(bytes<Vec<u8>>(Sqlite,
    "X'DEADBEEF'"
        == vec![0xDE_u8, 0xAD, 0xBE, 0xEF],
    "X''"
        == Vec::<u8>::new(),
    "X'0000000052'"
        == vec![0_u8, 0, 0, 0, 0x52]
));

mod json_tests {
    use super::*;
    use musqlite_core::types;
    use musqlite_test::test_type;
    use serde_json::{json, Value as JsonValue};

    test_type!(json<JsonValue>(
        Sqlite,
        "'\"Hello, World\"'" == json!("Hello, World"),
        "'\"😎\"'" == json!("😎"),
        "'\"🙋‍♀️\"'" == json!("🙋‍♀️"),
        "'[\"Hello\",\"World!\"]'" == json!(["Hello", "World!"])
    ));

    #[derive(serde::Deserialize, serde::Serialize, Debug, PartialEq)]
    struct Friend {
        name: String,
        age: u32,
    }

    test_type!(json_struct<types::Json<Friend>>(
        Sqlite,
        "\'{\"name\":\"Joe\",\"age\":33}\'" == types::Json(Friend { name: "Joe".to_string(), age: 33 })
    ));

    // NOTE: This is testing recursive (and transparent) usage of the `Json` wrapper. You don't
    //       need to wrap the Vec in Json<_> to make the example work.

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Customer {
        json_column: types::Json<Vec<i64>>,
    }

    test_type!(json_struct_json_column<types::Json<Customer>>(
        Sqlite,
        "\'{\"json_column\":[1,2]}\'" == types::Json(Customer { json_column: types::Json(vec![1, 2]) })
    ));

    #[tokio::test]
    async fn it_json_extracts() -> anyhow::Result<()> {
        let mut conn = new::<Sqlite>().await?;

        let value = musqlite_core::query(
            "select JSON_EXTRACT(JSON('{ \"number\": 42 }'), '$.number') = ?1",
        )
        .bind(42_i32)
        .try_map(|row: SqliteRow| row.try_get::<bool, _>(0))
        .fetch_one(&mut conn)
        .await?;

        assert_eq!(true, value);

        Ok(())
    }
}

mod chrono {
    use super::*;
    use musqlite_core::types::chrono::{
        DateTime, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Utc,
    };

    test_type!(chrono_naive_date_time<NaiveDateTime>(Sqlite, "SELECT datetime({0}) is datetime(?), {0}, ?",
        "'2019-01-02 05:10:20'" == NaiveDate::from_ymd_opt(2019, 1, 2).unwrap().and_hms_opt(5, 10, 20).unwrap()
    ));

    test_type!(chrono_date_time_utc<DateTime::<Utc>>(Sqlite, "SELECT datetime({0}) is datetime(?), {0}, ?",
        "'1996-12-20T00:39:57+00:00'" == Utc.with_ymd_and_hms(1996, 12, 20, 0, 39, 57).unwrap()
    ));

    test_type!(chrono_date_time_fixed_offset<DateTime::<FixedOffset>>(Sqlite, "SELECT datetime({0}) is datetime(?), {0}, ?",
        "'2016-11-08T03:50:23-05:00'" == DateTime::<Utc>::from(FixedOffset::west_opt(5 * 3600).unwrap().with_ymd_and_hms(2016, 11, 08, 3, 50, 23).unwrap())
    ));
}

mod time_tests {
    use super::*;
    use musqlite_core::types::time::{Date, OffsetDateTime, PrimitiveDateTime, Time};
    use time::macros::{date, datetime, time};

    test_type!(time_offset_date_time<OffsetDateTime>(
        Sqlite,
        "SELECT datetime({0}) is datetime(?), {0}, ?",
        "'2015-11-19 01:01:39+01:00'" == datetime!(2015 - 11 - 19 1:01:39 +1),
        "'2014-10-18 00:00:38.697+00:00'" == datetime!(2014 - 10 - 18 00:00:38.697 +0),
        "'2013-09-17 23:59-01:00'" == datetime!(2013 - 9 - 17 23:59 -1),
        "'2016-03-07T22:36:55.135+03:30'" == datetime!(2016 - 3 - 7 22:36:55.135 +3:30),
        "'2017-04-11T14:35+02:00'" == datetime!(2017 - 4 - 11 14:35 +2),
    ));

    test_type!(time_primitive_date_time<PrimitiveDateTime>(
        Sqlite,
        "SELECT datetime({0}) is datetime(?), {0}, ?",
        "'2019-01-02 05:10:20'" == datetime!(2019 - 1 - 2 5:10:20),
        "'2018-12-01 04:09:19.543'" == datetime!(2018 - 12 - 1 4:09:19.543),
        "'2017-11-30 03:08'" == datetime!(2017 - 11 - 30 3:08),
        "'2016-10-29T02:07:17'" == datetime!(2016 - 10 - 29 2:07:17),
        "'2015-09-28T01:06:16.432'" == datetime!(2015 - 9 - 28 1:06:16.432),
        "'2014-08-27T00:05'" == datetime!(2014 - 8 - 27 0:05),
        "'2013-07-26 23:04:14Z'" == datetime!(2013 - 7 - 26 23:04:14),
        "'2012-06-25 22:03:13.321Z'" == datetime!(2012 - 6 - 25 22:03:13.321),
        "'2011-05-24 21:02Z'" == datetime!(2011 - 5 - 24 21:02),
        "'2010-04-23T20:01:11Z'" == datetime!(2010 - 4 - 23 20:01:11),
        "'2009-03-22T19:00:10.21Z'" == datetime!(2009 - 3 - 22 19:00:10.21),
        "'2008-02-21T18:59Z'" == datetime!(2008 - 2 - 21 18:59:00),
    ));

    test_type!(time_date<Date>(
        Sqlite,
        "SELECT date({0}) is date(?), {0}, ?",
        "'2002-06-04'" == date!(2002 - 6 - 4),
    ));

    test_type!(time_time<Time>(
        Sqlite,
        "SELECT time({0}) is time(?), {0}, ?",
        "'21:46:32'" == time!(21:46:32),
        "'20:45:31.133'" == time!(20:45:31.133),
        "'19:44'" == time!(19:44),
    ));
}

mod bstr {
    use super::*;
    use musqlite_core::types::bstr::BString;

    test_type!(bstring<BString>(Sqlite,
        "cast('abc123' as blob)" == BString::from(&b"abc123"[..]),
        "x'0001020304'" == BString::from(&b"\x00\x01\x02\x03\x04"[..])
    ));
}

test_type!(uuid<musqlite_core::types::Uuid>(Sqlite,
    "x'b731678f636f4135bc6f19440c13bd19'"
        == musqlite_core::types::Uuid::parse_str("b731678f-636f-4135-bc6f-19440c13bd19").unwrap(),
    "x'00000000000000000000000000000000'"
        == musqlite_core::types::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
));

test_type!(uuid_hyphenated<musqlite_core::types::uuid::fmt::Hyphenated>(Sqlite,
    "'b731678f-636f-4135-bc6f-19440c13bd19'"
        == musqlite_core::types::Uuid::parse_str("b731678f-636f-4135-bc6f-19440c13bd19").unwrap().hyphenated(),
    "'00000000-0000-0000-0000-000000000000'"
        == musqlite_core::types::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap().hyphenated()
));

test_type!(uuid_simple<musqlite_core::types::uuid::fmt::Simple>(Sqlite,
    "'b731678f636f4135bc6f19440c13bd19'"
        == musqlite_core::types::Uuid::parse_str("b731678f636f4135bc6f19440c13bd19").unwrap().simple(),
    "'00000000000000000000000000000000'"
        == musqlite_core::types::Uuid::parse_str("00000000000000000000000000000000").unwrap().simple()
));