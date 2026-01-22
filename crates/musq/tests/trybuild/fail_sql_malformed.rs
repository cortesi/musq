use musq::*;

fn main() -> musq::Result<()> {
    let _ = sql!("SELECT {ident}")?;
    Ok(())
}
