use std::fmt::Display;

use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, TypeInfo},
    Type, ValueRef,
};
pub use chrono::{
    DateTime, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Offset, SecondsFormat,
    TimeZone, Utc,
};

impl<Tz: TimeZone> Type for DateTime<Tz> {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Datetime)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <NaiveDateTime as Type>::compatible(ty)
    }
}

impl Type for NaiveDateTime {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Datetime)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(
            ty.0,
            SqliteDataType::Datetime
                | SqliteDataType::Text
                | SqliteDataType::Int64
                | SqliteDataType::Int
                | SqliteDataType::Float
        )
    }
}

impl Type for NaiveDate {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Date)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Date | SqliteDataType::Text)
    }
}

impl Type for NaiveTime {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Time)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Time | SqliteDataType::Text)
    }
}

impl<Tz: TimeZone> Encode<'_> for DateTime<Tz>
where
    Tz::Offset: Display,
{
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.to_rfc3339_opts(SecondsFormat::AutoSi, false), buf)
    }
}

impl Encode<'_> for NaiveDateTime {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.format("%F %T%.f").to_string(), buf)
    }
}

impl Encode<'_> for NaiveDate {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.format("%F").to_string(), buf)
    }
}

impl Encode<'_> for NaiveTime {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.format("%T%.f").to_string(), buf)
    }
}

impl<'r> Decode<'r> for DateTime<Utc> {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Utc.from_utc_datetime(&decode_datetime(value)?.naive_utc()))
    }
}

impl<'r> Decode<'r> for DateTime<Local> {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Local.from_utc_datetime(&decode_datetime(value)?.naive_utc()))
    }
}

impl<'r> Decode<'r> for DateTime<FixedOffset> {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        decode_datetime(value)
    }
}

fn decode_datetime(value: ValueRef<'_>) -> Result<DateTime<FixedOffset>, BoxDynError> {
    let dt = match value.type_info().0 {
        SqliteDataType::Text => decode_datetime_from_text(value.text()?),
        SqliteDataType::Int | SqliteDataType::Int64 => decode_datetime_from_int(value.int64()),
        SqliteDataType::Float => decode_datetime_from_float(value.double()),

        _ => None,
    };

    if let Some(dt) = dt {
        Ok(dt)
    } else {
        Err(format!("invalid datetime: {}", value.text()?).into())
    }
}

fn decode_datetime_from_text(value: &str) -> Option<DateTime<FixedOffset>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
        return Some(dt);
    }

    // Loop over common date time patterns, inspired by Diesel
    // https://github.com/diesel-rs/diesel/blob/93ab183bcb06c69c0aee4a7557b6798fd52dd0d8/diesel/src/sqlite/types/date_and_time/chrono.rs#L56-L97
    let sqlite_datetime_formats = &[
        // Most likely format
        "%F %T%.f",
        // Other formats in order of appearance in docs
        "%F %R",
        "%F %RZ",
        "%F %R%:z",
        "%F %T%.fZ",
        "%F %T%.f%:z",
        "%FT%R",
        "%FT%RZ",
        "%FT%R%:z",
        "%FT%T%.f",
        "%FT%T%.fZ",
        "%FT%T%.f%:z",
    ];

    for format in sqlite_datetime_formats {
        if let Ok(dt) = DateTime::parse_from_str(value, format) {
            return Some(dt);
        }

        if let Ok(dt) = NaiveDateTime::parse_from_str(value, format) {
            return Some(Utc.fix().from_utc_datetime(&dt));
        }
    }

    None
}

fn decode_datetime_from_int(value: i64) -> Option<DateTime<FixedOffset>> {
    NaiveDateTime::from_timestamp_opt(value, 0).map(|dt| Utc.fix().from_utc_datetime(&dt))
}

fn decode_datetime_from_float(value: f64) -> Option<DateTime<FixedOffset>> {
    let epoch_in_julian_days = 2_440_587.5;
    let seconds_in_day = 86400.0;
    let timestamp = (value - epoch_in_julian_days) * seconds_in_day;
    let seconds = timestamp as i64;
    let nanos = (timestamp.fract() * 1E9) as u32;

    NaiveDateTime::from_timestamp_opt(seconds, nanos).map(|dt| Utc.fix().from_utc_datetime(&dt))
}

impl<'r> Decode<'r> for NaiveDateTime {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(decode_datetime(value)?.naive_local())
    }
}

impl<'r> Decode<'r> for NaiveDate {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(NaiveDate::parse_from_str(value.text()?, "%F")?)
    }
}

impl<'r> Decode<'r> for NaiveTime {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = value.text()?;

        // Loop over common time patterns, inspired by Diesel
        // https://github.com/diesel-rs/diesel/blob/93ab183bcb06c69c0aee4a7557b6798fd52dd0d8/diesel/src/sqlite/types/date_and_time/chrono.rs#L29-L47
        #[rustfmt::skip] // don't like how rustfmt mangles the comments
        let sqlite_time_formats = &[
            // Most likely format
            "%T.f", "%T%.f",
            // Other formats in order of appearance in docs
            "%R", "%RZ", "%T%.fZ", "%R%:z", "%T%.f%:z",
        ];

        for format in sqlite_time_formats {
            if let Ok(dt) = NaiveTime::parse_from_str(value, format) {
                return Ok(dt);
            }
        }

        Err(format!("invalid time: {}", value).into())
    }
}
