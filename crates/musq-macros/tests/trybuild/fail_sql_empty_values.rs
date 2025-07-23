use musq::*;

fn main() -> musq::Result<()> {
    let _ = sql!("SELECT * FROM t WHERE id IN ({values:[]})")?;
    Ok(())
}
