use musq_macros::sql;

fn main() {
    let _ = sql!("SELECT * FROM users WHERE id = {}", 1, 2);
}
