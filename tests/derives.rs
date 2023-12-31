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
    #[musq(prefix = "prefix_")]
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
    .bind("one")
    .bind(2)
    .bind(3)
    .bind(1)
    .bind("foobar")
    .bind("foo")
    .bind(4)
    .bind("nest")
    .bind(5)
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
