use musq::*;

fn main() -> musq::Result<()> {
    let table = "foo";
    let _ = sql!("SELECT * FROM {ident:table}")?;
    Ok(())
}
