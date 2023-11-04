use crate::{
    decode::Decode,
    encode::{Encode, IsNull},
    sqlite::{error::BoxDynError, ArgumentValue, SqliteDataType, TypeInfo},
    Type, ValueRef,
};
use time::format_description::{well_known::Rfc3339, FormatItem};
use time::macros::format_description as fd;
pub use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

impl Type for OffsetDateTime {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Datetime)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        <PrimitiveDateTime as Type>::compatible(ty)
    }
}

impl Type for PrimitiveDateTime {
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
        )
    }
}

impl Type for Date {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Date)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Date | SqliteDataType::Text)
    }
}

impl Type for Time {
    fn type_info() -> TypeInfo {
        TypeInfo(SqliteDataType::Time)
    }

    fn compatible(ty: &TypeInfo) -> bool {
        matches!(ty.0, SqliteDataType::Time | SqliteDataType::Text)
    }
}

impl Encode<'_> for OffsetDateTime {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        Encode::encode(self.format(&Rfc3339).unwrap(), buf)
    }
}

impl Encode<'_> for PrimitiveDateTime {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        let format = fd!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond]");
        Encode::encode(self.format(&format).unwrap(), buf)
    }
}

impl Encode<'_> for Date {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        let format = fd!("[year]-[month]-[day]");
        Encode::encode(self.format(&format).unwrap(), buf)
    }
}

impl Encode<'_> for Time {
    fn encode_by_ref(&self, buf: &mut Vec<ArgumentValue<'_>>) -> IsNull {
        let format = fd!("[hour]:[minute]:[second].[subsecond]");
        Encode::encode(self.format(&format).unwrap(), buf)
    }
}

impl<'r> Decode<'r> for OffsetDateTime {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        decode_offset_datetime(value)
    }
}

impl<'r> Decode<'r> for PrimitiveDateTime {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        decode_datetime(value)
    }
}

impl<'r> Decode<'r> for Date {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Date::parse(value.text()?, &fd!("[year]-[month]-[day]"))?)
    }
}

impl<'r> Decode<'r> for Time {
    fn decode(value: ValueRef<'r>) -> Result<Self, BoxDynError> {
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

        Err(format!("invalid time: {}", value).into())
    }
}

fn decode_offset_datetime(value: ValueRef<'_>) -> Result<OffsetDateTime, BoxDynError> {
    let dt = match value.type_info().0 {
        SqliteDataType::Text => decode_offset_datetime_from_text(value.text()?),
        SqliteDataType::Int | SqliteDataType::Int64 => {
            Some(OffsetDateTime::from_unix_timestamp(value.int64())?)
        }

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

fn decode_datetime(value: ValueRef<'_>) -> Result<PrimitiveDateTime, BoxDynError> {
    let dt = match value.type_info().0 {
        SqliteDataType::Text => decode_datetime_from_text(value.text()?),
        SqliteDataType::Int | SqliteDataType::Int64 => {
            let parsed = OffsetDateTime::from_unix_timestamp(value.int64()).unwrap();
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
    use time::format_description::{modifier, Component::*, FormatItem, FormatItem::*};

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
