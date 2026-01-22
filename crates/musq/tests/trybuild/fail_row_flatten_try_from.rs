use musq::FromRow;

#[derive(FromRow)]
struct Inner {
    a: i32,
}

#[derive(FromRow)]
struct Bad {
    #[musq(flatten, try_from = "i32")]
    inner: Inner,
}

fn main() {}
