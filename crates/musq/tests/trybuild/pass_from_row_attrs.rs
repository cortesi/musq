#![deny(warnings)]

use musq::{FromRow, Result, Row};

#[allow(dead_code)]
struct Custom(String);

fn custom_deserializer(prefix: &str, row: &Row) -> Result<Custom> {
    let column_name = format!("{prefix}custom");
    row.get_value::<String>(&column_name).map(Custom)
}

#[allow(dead_code)]
#[derive(FromRow)]
#[musq(rename_all = "snake_case")]
struct NamedAttributes {
    first_name: String,
    #[musq(default)]
    optional_count: i32,
    #[musq(skip)]
    skipped: bool,
    #[musq(deserialize_with = "custom_deserializer")]
    custom: Custom,
}

fn main() {}
