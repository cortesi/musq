use musq::*;

fn main() -> musq::Result<()> {
    let vals = values! { "id": 1, "name": "a" }?;
    let _ = sql!("INSERT INTO t {insert:vals}")?;
    let _ = sql!("UPDATE t SET {set:vals}")?;
    let _ = sql!("SELECT * FROM t WHERE {where:vals}")?;
    let _ = sql!("INSERT INTO t {insert:vals} ON CONFLICT(id) DO UPDATE SET {upsert:vals}")?;
    Ok(())
}
