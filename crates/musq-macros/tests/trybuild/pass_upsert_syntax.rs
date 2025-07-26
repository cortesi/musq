use musq::*;

fn main() -> musq::Result<()> {
    let v = values! {"id": 1, "name": "test", "email": "test@example.com"}?;
    
    // Test basic new syntax with one column
    let _query1 = sql!("INSERT INTO users (id, name) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: id}");

    // Test new syntax with multiple columns
    let _query2 = sql!("INSERT INTO users (id, name, email) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: id, email}");

    // Test with no spaces
    let _query3 = sql!("INSERT INTO users (id, name) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert:v,exclude:id}");

    // Test with extra spaces
    let _query4 = sql!("INSERT INTO users (id, name, email) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude:  id  ,   email  }");

    // Test with trailing comma
    let _query5 = sql!("INSERT INTO users (id, name, email) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: id, email, }");

    // Test with Rust keywords as identifiers
    let _query6 = sql!("INSERT INTO table_test (id, type, ref) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: type, ref}");

    // Test with underscores
    let _query7 = sql!("INSERT INTO table_test (id, user_id, last_modified) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v, exclude: user_id, last_modified}");

    // Test without exclude clause (should still work)
    let _query8 = sql!("INSERT INTO users (id, name) VALUES {insert: v} ON CONFLICT (id) DO UPDATE SET {upsert: v}");
    
    Ok(())
}