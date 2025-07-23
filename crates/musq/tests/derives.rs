use musq_macros::*;
use musq_test::{connection, test_type};

#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize, Json)]
struct JsonType {
    a: String,
    b: i32,
}

#[derive(Debug, PartialEq, Codec)]
enum PlainEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Codec)]
#[musq(rename_all = "verbatim")]
enum VerbatimEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Codec)]
#[musq(rename_all = "lower_case")]
enum LowerCaseEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Codec)]
#[musq(repr = "u32")]
enum ReprEnum {
    Foo = 1,
    Bar = 2,
}

#[derive(Debug, PartialEq, Codec)]
struct NewtypeStruct(i32);

#[derive(Debug, PartialEq, FromRow)]
pub struct Flattened {
    f: String,
    g: u32,
}

#[derive(Debug, PartialEq, FromRow)]
pub struct FromRowPlain {
    a: String,
    b: u32,
    c: NewtypeStruct,
    d: ReprEnum,
    e: LowerCaseEnum,
    #[musq(flatten)]
    f: Flattened,
    #[musq(flatten, prefix = "prefix_")]
    g: Flattened,
}

#[tokio::test]
async fn it_derives_fromrow_plain() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let row: FromRowPlain = musq::query_as(
        r"
        SELECT
        ? AS a,
        ? as b,
        ? as c,
        ? as d,
        ? as e,
        ? as f,
        ? as g,
        ? as prefix_f,
        ? as prefix_g
    ",
    )
    .bind("one")?
    .bind(2)?
    .bind(3)?
    .bind(1)?
    .bind("foobar")?
    .bind("foo")?
    .bind(4)?
    .bind("nest")?
    .bind(5)?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(
        row,
        FromRowPlain {
            a: "one".into(),
            b: 2,
            c: NewtypeStruct(3),
            d: ReprEnum::Foo,
            e: LowerCaseEnum::FooBar,
            f: Flattened {
                f: "foo".into(),
                g: 4,
            },
            g: Flattened {
                f: "nest".into(),
                g: 5,
            },
        }
    );
    Ok(())
}

#[derive(Debug, PartialEq, FromRow)]
struct Address {
    street: String,
    city: String,
    country: String,
}

#[derive(Debug, PartialEq, FromRow)]
struct UserOpt {
    id: i32,
    name: String,
    #[musq(flatten)]
    address: Option<Address>,
}

#[derive(Debug, PartialEq, FromRow)]
struct UserOptPref {
    id: i32,
    name: String,
    #[musq(flatten, prefix = "addr_")]
    address: Option<Address>,
}

#[tokio::test]
async fn flatten_option_all_null() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let user: UserOpt =
        musq::query_as("SELECT ? as id, ? as name, NULL as street, NULL as city, NULL as country")
            .bind(1i32)?
            .bind("Bob")?
            .fetch_one(&mut conn)
            .await?;
    assert_eq!(user.address, None);
    Ok(())
}

#[tokio::test]
async fn flatten_option_some() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let user: UserOpt =
        musq::query_as("SELECT ? as id, ? as name, ? as street, ? as city, ? as country")
            .bind(1i32)?
            .bind("Bob")?
            .bind("Main")?
            .bind("NYC")?
            .bind("US")?
            .fetch_one(&mut conn)
            .await?;
    assert_eq!(
        user.address,
        Some(Address {
            street: "Main".into(),
            city: "NYC".into(),
            country: "US".into(),
        })
    );
    Ok(())
}

#[tokio::test]
async fn flatten_option_prefix_all_null() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let user: UserOptPref = musq::query_as(
        "SELECT ? as id, ? as name, NULL as addr_street, NULL as addr_city, NULL as addr_country",
    )
    .bind(1i32)?
    .bind("Bob")?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(user.address, None);
    Ok(())
}

#[tokio::test]
async fn flatten_option_prefix_some() -> anyhow::Result<()> {
    let mut conn = connection().await?;
    let user: UserOptPref = musq::query_as(
        "SELECT ? as id, ? as name, ? as addr_street, ? as addr_city, ? as addr_country",
    )
    .bind(1i32)?
    .bind("Bob")?
    .bind("Main")?
    .bind("NYC")?
    .bind("US")?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(
        user.address,
        Some(Address {
            street: "Main".into(),
            city: "NYC".into(),
            country: "US".into(),
        })
    );
    Ok(())
}

test_type!(plain_enum<PlainEnum>(
    "\"foo\"" == PlainEnum::Foo,
    "\"foo_bar\"" == PlainEnum::FooBar,
));

test_type!(verbatim_enum<VerbatimEnum>(
    "\"Foo\"" == VerbatimEnum::Foo,
    "\"FooBar\"" == VerbatimEnum::FooBar,
));

test_type!(lowercase_enum<LowerCaseEnum>(
    "\"foo\"" == LowerCaseEnum::Foo,
    "\"foobar\"" == LowerCaseEnum::FooBar,
));

test_type!(origin_enum<ReprEnum>(
    "1" == ReprEnum::Foo,
    "2" == ReprEnum::Bar,
));

test_type!(newtype_struct<NewtypeStruct>(
    "1" == NewtypeStruct(1),
));

test_type!(json_type<JsonType>(
    r#"'{"a":"1","b":1}'"# == JsonType {
        a: "1".into(),
        b: 1,
    },
));
