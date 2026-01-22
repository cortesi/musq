//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    // Allow approximate constants in this test file - we're testing specific float representations
    #![allow(clippy::approx_constant)]

    use crate::test_type;

    test_type!(null<Option<i32>>(
        "NULL" == None::<i32>
    ));

    test_type!(bool("FALSE" == false, "TRUE" == true));

    test_type!(i32("94101" == 94101_i32));

    test_type!(i64("9358295312" == 9358295312_i64));

    // NOTE: This behavior can be surprising. Floating-point parameters are widening to double which can
    //       result in strange rounding.
    test_type!(f32("3.1410000324249268" == 3.141f32 as f64 as f32));

    test_type!(f64("939399419.1225182" == 939399419.1225182_f64));

    test_type!(numeric_f64<f64>(
        "SELECT CAST({0} AS NUMERIC) is CAST(? AS NUMERIC), CAST({0} AS NUMERIC), CAST(? AS NUMERIC)",
        "CAST(5.5 AS NUMERIC)" == 5.5_f64,
    ));

    test_type!(numeric_i64<i64>(
        "SELECT CAST({0} AS NUMERIC) is CAST(? AS NUMERIC), CAST({0} AS NUMERIC), CAST(? AS NUMERIC)",
        "CAST(1234 AS NUMERIC)" == 1234_i64,
    ));

    test_type!(str<String>(
        "'this is foo'" == "this is foo",
        "cast(x'7468697320006973206E756C2D636F6E7461696E696E67' as text)" == "this \0is nul-containing",
        "''" == ""
    ));

    test_type!(bytes<Vec<u8>>(
        "X'DEADBEEF'"
            == vec![0xDE_u8, 0xAD, 0xBE, 0xEF],
        "X''"
            == Vec::<u8>::new(),
        "X'0000000052'"
            == vec![0_u8, 0, 0, 0, 0x52]
    ));

    mod time_tests {
        use musq::types::time::{Date, OffsetDateTime, PrimitiveDateTime, Time};
        use time::macros::{date, datetime, time};

        use super::*;

        test_type!(time_offset_date_time<OffsetDateTime>(
            "SELECT datetime({0}) is datetime(?), {0}, ?",
            "'2015-11-19 01:01:39+01:00'" == datetime!(2015 - 11 - 19 1:01:39 +1),
            "'2014-10-18 00:00:38.697+00:00'" == datetime!(2014 - 10 - 18 00:00:38.697 +0),
            "'2013-09-17 23:59-01:00'" == datetime!(2013 - 9 - 17 23:59 -1),
            "'2016-03-07T22:36:55.135+03:30'" == datetime!(2016 - 3 - 7 22:36:55.135 +3:30),
            "'2017-04-11T14:35+02:00'" == datetime!(2017 - 4 - 11 14:35 +2),
            // Test the specific problematic RFC3339 format that was failing
            "'2025-07-22T06:20:47.847729Z'" == datetime!(2025 - 7 - 22 6:20:47.847729 UTC),
            // Test other microsecond precision formats
            "'2025-01-15T12:30:45.123456Z'" == datetime!(2025 - 1 - 15 12:30:45.123456 UTC),
            "'2024-12-31T23:59:59.999999Z'" == datetime!(2024 - 12 - 31 23:59:59.999999 UTC),
        ));

        test_type!(time_primitive_date_time<PrimitiveDateTime>(
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
            "SELECT date({0}) is date(?), {0}, ?",
            "'2002-06-04'" == date!(2002 - 6 - 4),
        ));

        test_type!(time_time<Time>(
            "SELECT time({0}) is time(?), {0}, ?",
            "'21:46:32'" == time!(21:46:32),
            "'20:45:31.133'" == time!(20:45:31.133),
            "'19:44'" == time!(19:44),
        ));
    }

    mod bstr {
        use musq::types::bstr::BString;

        use super::*;

        test_type!(bstring<BString>(
            "cast('abc123' as blob)" == BString::from(&b"abc123"[..]),
            "x'0001020304'" == BString::from(&b"\x00\x01\x02\x03\x04"[..])
        ));
    }
}
