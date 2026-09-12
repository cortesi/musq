use musq::Decode;

#[derive(Decode)]
struct Borrowed<'r>(&'r str);

fn main() {}
