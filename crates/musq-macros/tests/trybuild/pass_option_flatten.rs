use musq::FromRow;

#[derive(FromRow)]
struct Address {
    street: String,
    city: String,
}

#[derive(FromRow)]
struct User {
    id: i32,
    #[musq(flatten)]
    addr: Option<Address>,
}

fn main() {}
