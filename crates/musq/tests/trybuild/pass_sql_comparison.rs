use musq::sql;

fn main() {
    let a = 1;
    let b = 2;
    let _ = sql!("SELECT {}", a == b);
}
