//! Documentation snippets used by the README.

#![allow(dead_code, unused)]

/// Demonstrate opening a database with options.
async fn open() -> musq::Result<()> {
    // snips-start: open
    use musq::{JournalMode, Musq};

    let pool = Musq::new()
        .max_connections(10)
        .create_if_missing(true)
        .journal_mode(JournalMode::Wal)
        .open("app.db")
        .await?;
    // snips-end
    Ok(())
}

/// Demonstrate SQL query macros.
async fn sql() -> musq::Result<()> {
    use musq::Musq;
    let pool = Musq::new().open_in_memory().await?;

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

        sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
            .execute(&pool)
            .await?;

        let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
            .fetch_one(&pool)
            .await?;
        // snips-end

        // snips-start: sql_in
        let table_name = "users";
        let user_ids = vec![1, 2, 3];
        let columns = ["id", "name"];

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
    let pool = Musq::new().open_in_memory().await?;

    {
        // snips-start: values-null
        async fn add_user(
            pool: &musq::Pool,
            name: &str,
            phone: Option<String>,
        ) -> musq::Result<()> {
            let user_data = values! {
                "name": name,
                "phone": phone,      // Option: None encodes as NULL
                "email": musq::Null, // untyped NULL literal
            }?;
            sql!("INSERT INTO users {insert:user_data}")?
                .execute(pool)
                .await?;
            Ok(())
        }
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

/// Demonstrate transactions.
async fn transactions() -> musq::Result<()> {
    use musq::{Musq, sql};
    let pool = Musq::new().open_in_memory().await?;

    let id = 1;
    let name = "Alice";

    // snips-start: transaction
    let mut tx = pool.begin().await?;
    sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
        .execute(&tx)
        .await?;
    tx.commit().await?;
    // snips-end

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

    // snips-start: flatten
    #[derive(FromRow)]
    struct Address {
        street: String,
        city: String,
    }

    #[derive(FromRow)]
    struct User {
        id: i32,
        // Reads the `street` and `city` columns.
        #[musq(flatten)]
        address: Address,
        // Reads `billing_street` and `billing_city`; None iff both are NULL.
        #[musq(flatten, prefix = "billing_")]
        billing: Option<Address>,
    }
    // snips-end
}

/// Main entry point for running snippet examples locally.
fn main() {
    println!("This file contains code snippets for documentation purposes.");
}
