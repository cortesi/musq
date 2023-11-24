use musq_macros::*;
use musq_test::{connection, test_type};

#[derive(Debug, PartialEq, Type)]
enum PlainEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Type)]
#[musq(rename_all = "verbatim")]
enum VerbatimEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Type)]
#[musq(rename_all = "lower_case")]
enum LowerCaseEnum {
    Foo,
    FooBar,
}

#[derive(Debug, PartialEq, Type)]
#[musq(repr = "u32")]
enum ReprEnum {
    Foo = 1,
    Bar = 2,
}

#[derive(Debug, PartialEq, Type)]
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
        ? as g
    ",
    )
    .bind("one")
    .bind(2)
    .bind(3)
    .bind(1)
    .bind("foobar")
    .bind("foo")
    .bind(4)
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
