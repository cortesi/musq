use musq::FromRow;

#[derive(FromRow)]
struct Foo {
    a: i32,
}

#[derive(FromRow)]
struct Bad {
    #[musq(flatten, skip)]
    foo: Foo,
}

fn main() {}
