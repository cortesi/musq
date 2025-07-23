use musq::FromRow;

#[derive(FromRow)]
struct Record<'r, T> {
    a: &'r T,
    b: i32,
}

fn main() {}
