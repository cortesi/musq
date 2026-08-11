use musq::Encode;

#[derive(Encode)]
#[musq(try_from = "String")]
struct Id(String);

fn main() {}
