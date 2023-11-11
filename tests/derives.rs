use musqlite_macros::Type;
use musqlite_test::test_type;

#[derive(Debug, PartialEq, Type)]
#[musqlite(repr = "u32")]
enum Origin {
    Foo = 1,
    Bar = 2,
}

test_type!(origin_enum<Origin>(
    "1" == Origin::Foo,
    "2" == Origin::Bar,
));
