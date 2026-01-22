use musq::*;

fn main() -> musq::Result<()> {
    let _ = sql!("SELECT {idents:[]} FROM t")?;
    Ok(())
}
