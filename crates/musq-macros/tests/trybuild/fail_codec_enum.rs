use musq::Codec;

#[derive(Codec)]
enum Bad<'a, T> {
    A(&'a T),
}

fn main() {}
