use musq::*;

fn main() -> musq::Result<()> {
    let v = values! {"id": 1, "name": "test"}?;
    
    // Should fail - old syntax with brackets and strings
    let _query = sql!("INSERT INTO users (id, name) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: [\"id\"]}");
    Ok(())
}