use musq::*;

fn main() -> musq::Result<()> {
    let id = 1;
    let _ = sql!("SELECT {id}")?;
    Ok(())
}
