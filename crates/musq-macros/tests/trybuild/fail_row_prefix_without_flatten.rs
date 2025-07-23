use musq::FromRow;

#[derive(FromRow)]
struct Foo {
    a: i32,
}

#[derive(FromRow)]
struct Bad {
    #[musq(prefix = "pre_")]
    foo: Foo,
}

fn main() {}
