use musq_macros::sql;

fn main() {
    let id = 1;
    let _ = sql!("SELECT * FROM users WHERE id = {id}",);
}
