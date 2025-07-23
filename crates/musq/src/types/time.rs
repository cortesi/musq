use crate::{
    Value,
    decode::Decode,
    encode::Encode,
    error::{DecodeError, EncodeError},
    sqlite::SqliteDataType,
};
use time::format_description::{FormatItem, well_known::Rfc3339};
use time::macros::format_description as fd;
pub use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

impl Encode for OffsetDateTime {
    fn encode(self) -> Result<Value, EncodeError> {
        let formatted = self.format(&Rfc3339).map_err(|e| {
            EncodeError::Conversion(format!("failed to format OffsetDateTime: {e}"))
        })?;
        Ok(Value::Text {
            value: formatted,
            type_info: None,
        })
    }
}

impl Encode for PrimitiveDateTime {
    fn encode(self) -> Result<Value, EncodeError> {
        let format = fd!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]");
        let formatted = self.format(&format).map_err(|e| {
            EncodeError::Conversion(format!("failed to format PrimitiveDateTime: {e}"))
        })?;
        Ok(Value::Text {
            value: formatted,
            type_info: None,
        })
    }
}

impl Encode for Date {
    fn encode(self) -> Result<Value, EncodeError> {
        let format = fd!("[year]-[month]-[day]");
        let formatted = self
            .format(&format)
            .map_err(|e| EncodeError::Conversion(format!("failed to format Date: {e}")))?;
        Ok(Value::Text {
            value: formatted,
            type_info: None,
        })
    }
}

impl Encode for Time {
    fn encode(self) -> Result<Value, EncodeError> {
        let format = fd!("[hour]:[minute]:[second].[subsecond]");
        let formatted = self
            .format(&format)
            .map_err(|e| EncodeError::Conversion(format!("failed to format Time: {e}")))?;
        Ok(Value::Text {
            value: formatted,
            type_info: None,
        })
    }
}

impl<'r> Decode<'r> for OffsetDateTime {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        decode_offset_datetime(value)
    }
}

impl<'r> Decode<'r> for PrimitiveDateTime {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        decode_datetime(value)
    }
}

impl<'r> Decode<'r> for Date {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        Date::parse(value.text()?, &fd!("[year]-[month]-[day]"))
            .map_err(|e| DecodeError::Conversion(e.to_string()))
    }
}

impl<'r> Decode<'r> for Time {
    fn decode(value: &'r Value) -> std::result::Result<Self, DecodeError> {
        let value = value.text()?;

        let sqlite_time_formats = &[
            fd!("[hour]:[minute]:[second].[subsecond]"),
            fd!("[hour]:[minute]:[second]"),
            fd!("[hour]:[minute]"),
        ];

        for format in sqlite_time_formats {
            if let Ok(dt) = Time::parse(value, &format) {
                return Ok(dt);
            }
        }

        Err(format!("invalid time: {value}").into())
    }
}

fn decode_offset_datetime(value: &Value) -> std::result::Result<OffsetDateTime, DecodeError> {
    compatible!(
        value,
        SqliteDataType::Text
            | SqliteDataType::Int64
            | SqliteDataType::Int
            | SqliteDataType::Datetime
    );
    let dt = match value.type_info() {
        SqliteDataType::Text | SqliteDataType::Datetime => {
            decode_offset_datetime_from_text(value.text()?)
        }
        SqliteDataType::Int | SqliteDataType::Int64 => Some(
            OffsetDateTime::from_unix_timestamp(value.int64()?)
                .map_err(|e| DecodeError::Conversion(e.to_string()))?,
        ),

        _ => None,
    };

    if let Some(dt) = dt {
        Ok(dt)
    } else {
        Err(format!("invalid offset datetime: {}", value.text()?).into())
    }
}

fn decode_offset_datetime_from_text(value: &str) -> Option<OffsetDateTime> {
    if let Ok(dt) = OffsetDateTime::parse(value, &Rfc3339) {
        return Some(dt);
    }

    if let Ok(dt) = OffsetDateTime::parse(value, formats::OFFSET_DATE_TIME) {
        return Some(dt);
    }

    if let Some(dt) = decode_datetime_from_text(value) {
        return Some(dt.assume_utc());
    }

    None
}

fn decode_datetime(value: &Value) -> std::result::Result<PrimitiveDateTime, DecodeError> {
    compatible!(
        value,
        SqliteDataType::Text
            | SqliteDataType::Int64
            | SqliteDataType::Int
            | SqliteDataType::Datetime
    );
    let dt = match value.type_info() {
        SqliteDataType::Text | SqliteDataType::Datetime => decode_datetime_from_text(value.text()?),
        SqliteDataType::Int | SqliteDataType::Int64 => {
            let parsed = OffsetDateTime::from_unix_timestamp(value.int64()?)
                .map_err(|e| DecodeError::Conversion(e.to_string()))?;
            Some(PrimitiveDateTime::new(parsed.date(), parsed.time()))
        }
        _ => None,
    };

    if let Some(dt) = dt {
        Ok(dt)
    } else {
        Err(format!("invalid datetime: {}", value.text()?).into())
    }
}

fn decode_datetime_from_text(value: &str) -> Option<PrimitiveDateTime> {
    let default_format = fd!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]");
    if let Ok(dt) = PrimitiveDateTime::parse(value, &default_format) {
        return Some(dt);
    }

    let formats = [
        FormatItem::Compound(formats::PRIMITIVE_DATE_TIME_SPACE_SEPARATED),
        FormatItem::Compound(formats::PRIMITIVE_DATE_TIME_T_SEPARATED),
    ];

    if let Ok(dt) = PrimitiveDateTime::parse(value, &FormatItem::First(&formats)) {
        return Some(dt);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Value, sqlite::SqliteDataType};
    use time::macros::{date, datetime, time};

    #[test]
    fn test_offset_datetime_encode_decode() {
        let dt = datetime!(2023-12-25 15:30:45.123456789 UTC);
        let encoded = dt.encode().unwrap();
        let decoded: OffsetDateTime = Decode::decode(&encoded).unwrap();
        assert_eq!(dt, decoded);
    }

    #[test]
    fn test_offset_datetime_encode_decode_with_offset() {
        let dt = datetime!(2023-12-25 15:30:45.123456789 +05:30);
        let encoded = dt.encode().unwrap();
        let decoded: OffsetDateTime = Decode::decode(&encoded).unwrap();
        assert_eq!(dt, decoded);
    }

    #[test]
    fn test_offset_datetime_decode_from_unix_timestamp() {
        let timestamp = 1703516445i64; // 2023-12-25 15:00:45 UTC
        let value = Value::Integer {
            value: timestamp,
            type_info: Some(SqliteDataType::Int64),
        };
        let decoded: OffsetDateTime = Decode::decode(&value).unwrap();
        let expected = OffsetDateTime::from_unix_timestamp(timestamp).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_offset_datetime_decode_various_text_formats() {
        // Test RFC3339 format
        let value = Value::Text {
            value: "2023-12-25T15:30:45.123Z".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: OffsetDateTime = Decode::decode(&value).unwrap();
        let expected = datetime!(2023-12-25 15:30:45.123 UTC);
        assert_eq!(decoded, expected);

        // Test with timezone offset
        let value = Value::Text {
            value: "2023-12-25T15:30:45.123+05:30".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: OffsetDateTime = Decode::decode(&value).unwrap();
        let expected = datetime!(2023-12-25 15:30:45.123 +05:30);
        assert_eq!(decoded, expected);

        // Test space-separated format
        let value = Value::Text {
            value: "2023-12-25 15:30:45.123".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: OffsetDateTime = Decode::decode(&value).unwrap();
        let expected = datetime!(2023-12-25 15:30:45.123 UTC);
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_offset_datetime_decode_edge_cases() {
        // Test format with space and offset (this might reveal the bug)
        let value = Value::Text {
            value: "2023-12-25 15:30:45+05:30".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let result: Result<OffsetDateTime, _> = Decode::decode(&value);
        // This should work but might fail due to the bug
        match result {
            Ok(dt) => {
                let expected = datetime!(2023-12-25 15:30:45 +05:30);
                assert_eq!(dt, expected);
            }
            Err(e) => {
                println!("Failed to parse '2023-12-25 15:30:45+05:30': {e}");
                // This reveals the bug
            }
        }
    }

    #[test]
    fn test_offset_datetime_format_bug_fixed() {
        // Test that the bug fix works correctly

        // This should now FAIL to parse (which is correct)
        let value = Value::Text {
            value: "2023-12-2515:30:45+05:30".to_string(), // No separator between date and time
            type_info: Some(SqliteDataType::Text),
        };
        let result: Result<OffsetDateTime, _> = Decode::decode(&value);

        match result {
            Ok(_) => panic!("Bug still exists: invalid format was parsed"),
            Err(_) => println!("✓ Bug fixed: invalid format correctly rejected"),
        }

        // These should still work correctly
        let valid_formats = vec![
            "2023-12-25 15:30:45+05:30", // Space separator
            "2023-12-25T15:30:45+05:30", // T separator
        ];

        for format_str in valid_formats {
            let value = Value::Text {
                value: format_str.to_string(),
                type_info: Some(SqliteDataType::Text),
            };
            let result: Result<OffsetDateTime, _> = Decode::decode(&value);

            match result {
                Ok(_) => println!("✓ Valid format correctly parsed: {format_str}"),
                Err(e) => panic!("Valid format failed to parse {format_str}: {e}"),
            }
        }
    }

    #[test]
    fn test_specific_rfc3339_failure() {
        // Test the exact failing format from the error message
        let problematic_format = "2025-07-22T06:20:47.847729Z";

        let value = Value::Text {
            value: problematic_format.to_string(),
            type_info: Some(SqliteDataType::Datetime),
        };
        let result: Result<OffsetDateTime, _> = Decode::decode(&value);

        match result {
            Ok(dt) => {
                // Verify it parsed correctly
                assert_eq!(dt.year(), 2025);
                assert_eq!(dt.month() as u8, 7);
                assert_eq!(dt.day(), 22);
                assert_eq!(dt.hour(), 6);
                assert_eq!(dt.minute(), 20);
                assert_eq!(dt.second(), 47);
                assert_eq!(dt.offset(), time::UtcOffset::UTC);
            }
            Err(e) => {
                panic!("Failed to parse valid RFC3339 format '{problematic_format}': {e}");
            }
        }

        // Test similar formats that might also fail
        let similar_formats = vec![
            "2025-07-22T06:20:47Z",        // No microseconds
            "2025-07-22T06:20:47.123456Z", // 6 digit microseconds
            "2025-07-22T06:20:47.1Z",      // Single digit subseconds
            "2025-07-22T06:20:47.123Z",    // 3 digit subseconds
        ];

        for format_str in similar_formats {
            let value = Value::Text {
                value: format_str.to_string(),
                type_info: Some(SqliteDataType::Datetime),
            };
            let result: Result<OffsetDateTime, _> = Decode::decode(&value);

            match result {
                Ok(_) => {} // Success is expected
                Err(e) => panic!("Failed to parse valid format '{format_str}': {e}"),
            }
        }
    }

    #[test]
    fn test_primitive_datetime_encode_decode() {
        let dt = datetime!(2023-12-25 15:30:45.123456789);
        let encoded = dt.encode().unwrap();
        let decoded: PrimitiveDateTime = Decode::decode(&encoded).unwrap();
        assert_eq!(dt, decoded);
    }

    #[test]
    fn test_primitive_datetime_decode_from_unix_timestamp() {
        let timestamp = 1703516445i64;
        let value = Value::Integer {
            value: timestamp,
            type_info: Some(SqliteDataType::Int64),
        };
        let decoded: PrimitiveDateTime = Decode::decode(&value).unwrap();
        let expected_dt = OffsetDateTime::from_unix_timestamp(timestamp).unwrap();
        let expected = PrimitiveDateTime::new(expected_dt.date(), expected_dt.time());
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_date_encode_decode() {
        let d = date!(2023 - 12 - 25);
        let encoded = d.encode().unwrap();
        let decoded: Date = Decode::decode(&encoded).unwrap();
        assert_eq!(d, decoded);
    }

    #[test]
    fn test_time_encode_decode() {
        let t = time!(15:30:45.123456789);
        let encoded = t.encode().unwrap();
        let decoded: Time = Decode::decode(&encoded).unwrap();
        assert_eq!(t, decoded);
    }

    #[test]
    fn test_time_decode_various_formats() {
        // Test with subseconds
        let value = Value::Text {
            value: "15:30:45.123".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: Time = Decode::decode(&value).unwrap();
        let expected = time!(15:30:45.123);
        assert_eq!(decoded, expected);

        // Test without subseconds
        let value = Value::Text {
            value: "15:30:45".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: Time = Decode::decode(&value).unwrap();
        let expected = time!(15:30:45);
        assert_eq!(decoded, expected);

        // Test without seconds
        let value = Value::Text {
            value: "15:30".to_string(),
            type_info: Some(SqliteDataType::Text),
        };
        let decoded: Time = Decode::decode(&value).unwrap();
        let expected = time!(15:30);
        assert_eq!(decoded, expected);
    }
}

mod formats {
    use time::format_description::{Component::*, FormatItem, FormatItem::*, modifier};

    const YEAR: FormatItem<'_> = Component(Year({
        let mut value = modifier::Year::default();
        value.padding = modifier::Padding::Zero;
        value.repr = modifier::YearRepr::Full;
        value.iso_week_based = false;
        value.sign_is_mandatory = false;
        value
    }));

    const MONTH: FormatItem<'_> = Component(Month({
        let mut value = modifier::Month::default();
        value.padding = modifier::Padding::Zero;
        value.repr = modifier::MonthRepr::Numerical;
        value.case_sensitive = true;
        value
    }));

    const DAY: FormatItem<'_> = Component(Day({
        let mut value = modifier::Day::default();
        value.padding = modifier::Padding::Zero;
        value
    }));

    const HOUR: FormatItem<'_> = Component(Hour({
        let mut value = modifier::Hour::default();
        value.padding = modifier::Padding::Zero;
        value.is_12_hour_clock = false;
        value
    }));

    const MINUTE: FormatItem<'_> = Component(Minute({
        let mut value = modifier::Minute::default();
        value.padding = modifier::Padding::Zero;
        value
    }));

    const SECOND: FormatItem<'_> = Component(Second({
        let mut value = modifier::Second::default();
        value.padding = modifier::Padding::Zero;
        value
    }));

    const SUBSECOND: FormatItem<'_> = Component(Subsecond({
        let mut value = modifier::Subsecond::default();
        value.digits = modifier::SubsecondDigits::OneOrMore;
        value
    }));

    const OFFSET_HOUR: FormatItem<'_> = Component(OffsetHour({
        let mut value = modifier::OffsetHour::default();
        value.sign_is_mandatory = true;
        value.padding = modifier::Padding::Zero;
        value
    }));

    const OFFSET_MINUTE: FormatItem<'_> = Component(OffsetMinute({
        let mut value = modifier::OffsetMinute::default();
        value.padding = modifier::Padding::Zero;
        value
    }));

    pub(super) const OFFSET_DATE_TIME: &[FormatItem<'_>] = {
        &[
            YEAR,
            Literal(b"-"),
            MONTH,
            Literal(b"-"),
            DAY,
            First(&[Literal(b" "), Literal(b"T")]),
            HOUR,
            Literal(b":"),
            MINUTE,
            Optional(&Literal(b":")),
            Optional(&SECOND),
            Optional(&Literal(b".")),
            Optional(&SUBSECOND),
            Optional(&OFFSET_HOUR),
            Optional(&Literal(b":")),
            Optional(&OFFSET_MINUTE),
        ]
    };

    pub(super) const PRIMITIVE_DATE_TIME_SPACE_SEPARATED: &[FormatItem<'_>] = {
        &[
            YEAR,
            Literal(b"-"),
            MONTH,
            Literal(b"-"),
            DAY,
            Literal(b" "),
            HOUR,
            Literal(b":"),
            MINUTE,
            Optional(&Literal(b":")),
            Optional(&SECOND),
            Optional(&Literal(b".")),
            Optional(&SUBSECOND),
            Optional(&Literal(b"Z")),
        ]
    };

    pub(super) const PRIMITIVE_DATE_TIME_T_SEPARATED: &[FormatItem<'_>] = {
        &[
            YEAR,
            Literal(b"-"),
            MONTH,
            Literal(b"-"),
            DAY,
            Literal(b"T"),
            HOUR,
            Literal(b":"),
            MINUTE,
            Optional(&Literal(b":")),
            Optional(&SECOND),
            Optional(&Literal(b".")),
            Optional(&SUBSECOND),
            Optional(&Literal(b"Z")),
        ]
    };
}
