use musq::FromRow;

#[derive(FromRow)]
struct TupleDefault(#[musq(default)] i32);

#[derive(FromRow)]
#[musq(rename_all = "camel_case")]
struct TupleRenameAll(i32);

fn main() {}
