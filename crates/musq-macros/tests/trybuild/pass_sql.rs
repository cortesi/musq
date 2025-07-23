use musq::*;

fn main() -> musq::Result<()> {
    let id = 5;
    let _ = sql!("SELECT {}", id)?;
    Ok(())
}
