use musq::*;

fn main() -> musq::Result<()> {
    let vals = values! { "id": 1, "name": "a" }?;
    let _ = sql!("INSERT INTO t {insert_values:vals}")?;
    let _ = sql!("UPDATE t SET {update_set:vals}")?;
    let _ = sql!("SELECT * FROM t WHERE {where_and:vals}")?;
    let _ = sql!("INSERT INTO t {insert_values:vals} ON CONFLICT(id) DO UPDATE SET {upsert_set:vals}")?;
    Ok(())
}
