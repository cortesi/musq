use musq::*;

fn main() -> musq::Result<()> {
    let _ = sql!("SELECT 1", id = 5)?;
    Ok(())
}
