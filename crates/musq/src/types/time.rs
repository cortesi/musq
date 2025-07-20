use crate::{
    compatible,
    decode::Decode,
    encode::Encode,
    error::DecodeError,
    sqlite::{SqliteDataType, Value},
};
use time::format_description::{FormatItem, well_known::Rfc3339};
use time::macros::format_description as fd;
pub use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

impl Encode for OffsetDateTime {
    fn encode(self) -> Value {
        Value::Text(self.format(&Rfc3339).unwrap().into_bytes(), None)
    }
}

impl Encode for PrimitiveDateTime {
    fn encode(self) -> Value {
        let format = fd!(
            "[year]-[month]-[day] [hour padding:zero]:[minute padding:zero]:[second padding:zero].[subsecond]"
        );
        Value::Text(self.format(&format).unwrap().into_bytes(), None)
    }
}

impl Encode for Date {
    fn encode(self) -> Value {
        let format = fd!("[year]-[month]-[day]");
        Value::Text(self.format(&format).unwrap().into_bytes(), None)
    }
}

impl Encode for Time {
    fn encode(self) -> Value {
        let format =
            fd!("[hour padding:zero]:[minute padding:zero]:[second padding:zero].[subsecond]");
        Value::Text(self.format(&format).unwrap().into_bytes(), None)
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
        SqliteDataType::Text | SqliteDataType::Int64 | SqliteDataType::Int
    );
    let dt = match value.type_info() {
        SqliteDataType::Text => decode_offset_datetime_from_text(value.text()?),
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
        SqliteDataType::Text | SqliteDataType::Int64 | SqliteDataType::Int
    );
    let dt = match value.type_info() {
        SqliteDataType::Text => decode_datetime_from_text(value.text()?),
        SqliteDataType::Int | SqliteDataType::Int64 => {
            let parsed = OffsetDateTime::from_unix_timestamp(value.int64()?).unwrap();
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
            Optional(&Literal(b" ")),
            Optional(&Literal(b"T")),
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
