#![allow(dead_code)]

use musq::{FromRow, Musq, insert_into, sql, sql_as};

#[tokio::main]
async fn main() -> musq::Result<()> {
    // START - Connection Pooling section
    // Connection pooling example
    {
        let _pool = Musq::new().max_connections(10).open_in_memory().await?;

        // `pool` can now be shared across your application
        println!("Created pool with max 10 connections");
    }
    // END - Connection Pooling section

    // Create main pool for remaining examples
    let pool = Musq::new().open_in_memory().await?;

    // Set up tables for examples
    sql!("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")?
        .execute(&pool)
        .await?;

    sql!("CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY, name TEXT, category TEXT, price REAL);")?
        .execute(&pool)
        .await?;

    // START - Querying section (sql! and sql_as! macros)
    // Basic querying with sql! and sql_as! macros
    {
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

        println!("Retrieved user: {user:?}");
    }
    // END - Querying section (sql! and sql_as! macros)

    // START - Querying section (dynamic identifiers and lists)
    // Dynamic identifiers and lists
    {
        #[derive(FromRow, Debug)]
        struct User {
            id: i32,
            name: String,
        }

        // Insert more test data
        sql!("INSERT INTO users (id, name) VALUES (2, 'Charlie')")?
            .execute(&pool)
            .await?;
        sql!("INSERT INTO users (id, name) VALUES (3, 'Diana')")?
            .execute(&pool)
            .await?;

        let table_name = "users";
        let user_ids = vec![1, 2, 3];
        let columns = ["id", "name"];

        // Dynamic table and column identifiers
        let users: Vec<User> = sql_as!(
            "SELECT {idents:columns} FROM {ident:table_name} WHERE id IN ({values:user_ids})"
        )?
        .fetch_all(&pool)
        .await?;

        println!("Retrieved {} users with dynamic query", users.len());
    }
    // END - Querying section (dynamic identifiers and lists)

    // START - Querying section (dynamic query composition)
    // Dynamic query composition
    {
        // Insert test product data
        sql!("INSERT INTO products (id, name, category, price) VALUES (1, 'Laptop', 'electronics', 999.99)")?
            .execute(&pool)
            .await?;
        sql!(
            "INSERT INTO products (id, name, category, price) VALUES (2, 'Book', 'books', 19.99)"
        )?
        .execute(&pool)
        .await?;
        sql!("INSERT INTO products (id, name, category, price) VALUES (3, 'Phone', 'electronics', 599.99)")?
            .execute(&pool)
            .await?;

        struct SearchParams {
            category: Option<String>,
            min_price: Option<f64>,
        }

        let params = SearchParams {
            category: Some("electronics".to_string()),
            min_price: Some(500.0),
        };

        let mut query = sql!("SELECT * FROM products WHERE 1 = 1")?;

        if let Some(category) = &params.category {
            query = query.join(sql!("AND category = {category}")?);
        }

        if let Some(min_price) = params.min_price {
            query = query.join(sql!("AND price >= {min_price}")?);
        }

        let products = query.fetch_all(&pool).await?;
        println!("Found {} products matching criteria", products.len());
    }
    // END - Querying section (dynamic query composition)

    // START - Helpers section (insert_into builder)
    // insert_into builder
    {
        // Construct the query
        let insert_query = insert_into("users")
            .value("id", 4)
            .value("name", "Bob")
            .query()?;

        insert_query.execute(&pool).await?;

        // The builder can also execute directly
        insert_into("users")
            .value("id", 5)
            .value("name", "Carol")
            .execute(&pool)
            .await?;

        println!("Inserted users using builder pattern");
    }
    // END - Helpers section (insert_into builder)

    Ok(())
}
