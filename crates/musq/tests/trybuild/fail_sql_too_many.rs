use musq::*;

fn main() -> musq::Result<()> {
    let _ = sql!("SELECT {}", 1, 2)?;
    Ok(())
}
