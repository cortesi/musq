use musq_macros::Type;
use musq_test::test_type;

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
