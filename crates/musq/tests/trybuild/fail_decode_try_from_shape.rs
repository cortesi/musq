use musq::Decode;

#[derive(Decode)]
#[musq(try_from = "String")]
struct Id {
    value: String,
}

fn main() {}
