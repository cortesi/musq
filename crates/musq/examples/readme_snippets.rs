//! Documentation snippets used by the README.

#![allow(dead_code, unused)]

/// Demonstrate basic pool creation.
async fn pool() -> musq::Result<()> {
    // snips-start: pool
    use musq::Musq;
    let pool = Musq::new().max_connections(10).open_in_memory().await?;
    // snips-end
    Ok(())
}

/// Demonstrate SQL query macros.
async fn sql() -> musq::Result<()> {
    use musq::Musq;
    let pool = Musq::new().max_connections(10).open_in_memory().await?;

    {
        // snips-start: sql_basic
        use musq::{FromRow, sql, sql_as};

        #[derive(FromRow, Debug)]
        struct User {
            id: i32,
            name: String,
        }

        let id = 1;
        let name = "Bob";

        // Positional and named arguments
        sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
            .execute(&pool)
            .await?;

        // Map results directly to a struct
        let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
            .fetch_one(&pool)
            .await?;
        // snips-end

        // snips-start: sql_in
        let table_name = "users";
        let user_ids = vec![1, 2, 3];
        let columns = ["id", "name"];

        // Dynamic table and column identifiers
        let users: Vec<User> = sql_as!(
            "SELECT {idents:columns} FROM {ident:table_name} WHERE id IN ({values:user_ids})"
        )?
        .fetch_all(&pool)
        .await?;
        // snips-end
    }

    Ok(())
}

/// Demonstrate value helpers and macros.
async fn values() -> musq::Result<()> {
    use musq::{FromRow, Musq, Null, Values, sql, sql_as, values};
    let pool = Musq::new().max_connections(10).open_in_memory().await?;

    {
        // snips-start: values-fluent
        let vals = Values::new()
            .val("id", 1)?
            .val("name", "Alice")?
            .val("status", "active")?;
        // snips-end
    }

    {
        // snips-start: values-macro
        let vals = values! {
            "id": 1,
            "name": "Alice",
            "status": "active"
        }?;
        // snips-end
    }

    {
        // snips-start: values-optional
        async fn add_user(pool: musq::Pool, name: &str, phone: Option<String>) -> musq::Result<()> {
            let user_data = values! {
                "name": name,
                "phone": phone,  // Nullable field
            }?;
            sql!("INSERT INTO users {insert: user_data}")?
                .execute(&pool)
                .await?;
            Ok(())
        }
        // snips-end
    }

    {
        // snips-start: values-null
        use musq::{Null, values};
        let user_data = values! {
            "name": "Alice",
            "email": Null,           // No type annotation needed
        }?;
        // snips-end
    }

    {
        // snips-start: values-expr
        use musq::expr;
        let changes = values! {
            "updated_at": expr::now_rfc3339_utc(),
            "payload": expr::jsonb(r#"{"event":"hello"}"#),
        }?;

        sql!("UPDATE events SET {set:changes} WHERE id = 1")?
            .execute(&pool)
            .await?;
        // snips-end
    }

    {
        #[derive(FromRow, Debug)]
        struct User {
            id: i32,
            name: String,
        }

        // snips-start: values
        use musq::{Values, sql, sql_as, values};

        let user_data = values! { "id": 1, "name": "Alice", "status": "active" }?;

        sql!("INSERT INTO users {insert:user_data}")?
            .execute(&pool)
            .await?;

        let changes = Values::new()
            .val("name", "Alicia")?
            .val("status", "inactive")?;

        sql!("UPDATE users SET {set:changes} WHERE id = 1")?
            .execute(&pool)
            .await?;

        let filters = values! { "status": "inactive" }?;
        let user: User = sql_as!("SELECT id, name FROM users WHERE {where:filters}")?
            .fetch_one(&pool)
            .await?;

        let upsert = values! { "id": 1, "name": "Alicia", "status": "active" }?;
        sql!(
            "INSERT INTO users {insert:upsert} ON CONFLICT(id) DO UPDATE SET {upsert:upsert, exclude: id}"
        )?
        .execute(&pool)
        .await?;
        // snips-end
    }

    Ok(())
}

/// Demonstrate derive macros for types.
fn derives() {
    use musq::FromRow;

    // snips-start: text_enum
    #[derive(musq::Codec, Debug, PartialEq)]
    enum Status {
        Open,
        Closed,
    }
    // snips-end

    // snips-start: num_enum
    #[derive(musq::Codec, Debug, PartialEq)]
    #[musq(repr = "i32")]
    enum Priority {
        Low = 1,
        Medium = 2,
        High = 3,
    }
    // snips-end

    // snips-start: newtype
    #[derive(musq::Codec, Debug, PartialEq)]
    struct UserId(i32);
    // snips-end

    // snips-start: json
    #[derive(musq::Json, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct Metadata {
        tags: Vec<String>,
        version: i32,
    }
    // snips-end

    // snips-start: fromrow_fields
    #[derive(FromRow)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(FromRow)]
    struct User {
        id: i32,
        name: String,
        // `address` will be `Some` if either `street` or `city` is not NULL.
        // It will be `None` only if both `street` and `city` are NULL.
        #[musq(flatten)]
        address: Option<Address>,
    }
    // snips-end

    // snips-start: fromrow_flatten
    #[derive(FromRow)]
    struct UserWithAddresses {
        id: i32,
        // Looks for `billing_street` and `billing_city`.
        #[musq(flatten, prefix = "billing_")]
        billing_address: Address,

        // Looks for `shipping_street` and `shipping_city`.
        // Will be `None` if both are NULL.
        #[musq(flatten, prefix = "shipping_")]
        shipping_address: Option<Address>,
    }
    // snips-end

    {
        // snips-start: fromrow_basic
        #[derive(FromRow, Debug)]
        struct User {
            id: i32,
            name: String,
            email: String,
        }
        // snips-end
    }
}

/// Main entry point for running snippet examples locally.
fn main() {
    println!("This file contains code snippets for documentation purposes.");
}
