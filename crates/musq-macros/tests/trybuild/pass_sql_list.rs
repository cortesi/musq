use musq::*;

fn main() -> musq::Result<()> {
    let ids = vec![1,2];
    let cols = ["id", "name"];
    let _ = sql!("SELECT {idents:cols} FROM t WHERE id IN ({values:ids})")?;
    Ok(())
}
