#![allow(dead_code)]

use musq::{FromRow, Musq, sql, sql_as};

// START - FromRow section (basic usage)
// Basic FromRow usage
#[derive(musq::FromRow, Debug)]
struct User {
    id: i32,
    name: String,
    email: String,
}
// END - FromRow section (basic usage)

// START - FromRow section (example with attributes)
// Nested struct for flattening examples
#[derive(FromRow, Debug)]
struct Address {
    street: String,
    city: String,
}

// Simplified FromRow with common features
#[derive(FromRow, Debug)]
struct SimpleUser {
    id: i32,
    full_name: String,

    #[musq(default)]
    bio: Option<String>,

    // Looks for `street` and `city`.
    #[musq(flatten)]
    address: Address,
}
// END - FromRow section (example with attributes)

// START - FromRow section (optional flattened struct with prefix)
// Example with optional flattened struct
#[derive(FromRow, Debug)]
struct UserWithAddresses {
    id: i32,
    name: String,
    // Looks for `billing_street` and `billing_city`.
    #[musq(flatten, prefix = "billing_")]
    billing_address: Address,

    // Looks for `shipping_street` and `shipping_city`.
    // Will be `None` if both are NULL.
    #[musq(flatten, prefix = "shipping_")]
    shipping_address: Option<Address>,
}
// END - FromRow section (optional flattened struct with prefix)

#[tokio::main]
async fn main() -> musq::Result<()> {
    let pool = Musq::new().open_in_memory().await?;

    // Set up tables
    sql!(
        "CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        email TEXT NOT NULL
    );"
    )?
    .execute(&pool)
    .await?;

    sql!(
        "CREATE TABLE simple_users (
        id INTEGER PRIMARY KEY,
        full_name TEXT NOT NULL,
        bio TEXT,
        street TEXT NOT NULL,
        city TEXT NOT NULL
    );"
    )?
    .execute(&pool)
    .await?;

    sql!(
        "CREATE TABLE user_addresses (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        billing_street TEXT NOT NULL,
        billing_city TEXT NOT NULL,
        shipping_street TEXT,
        shipping_city TEXT
    );"
    )?
    .execute(&pool)
    .await?;

    // Test basic FromRow
    {
        sql!("INSERT INTO users (id, name, email) VALUES (1, 'Alice', 'alice@example.com')")?
            .execute(&pool)
            .await?;

        let user: User = sql_as!("SELECT id, name, email FROM users WHERE id = 1")?
            .fetch_one(&pool)
            .await?;

        println!("Basic FromRow: {user:?}");
    }

    // Test simple FromRow with common features
    {
        sql!(
            "INSERT INTO simple_users 
              (id, full_name, bio, street, city)
              VALUES (1, 'John Doe', 'Software Developer', '123 Main St', 'Springfield')"
        )?
        .execute(&pool)
        .await?;

        let simple_user: SimpleUser = sql_as!(
            "SELECT id, full_name, bio, street, city 
             FROM simple_users WHERE id = 1"
        )?
        .fetch_one(&pool)
        .await?;

        println!("Simple FromRow with common features: {simple_user:?}");

        // Verify flattening worked
        assert_eq!(simple_user.address.street, "123 Main St");
        assert_eq!(simple_user.address.city, "Springfield");
        assert_eq!(simple_user.full_name, "John Doe");
        assert_eq!(simple_user.bio, Some("Software Developer".to_string()));
    }

    // Test optional flattened struct (None case)
    {
        sql!(
            "INSERT INTO user_addresses 
              (id, name, billing_street, billing_city, shipping_street, shipping_city)
              VALUES (1, 'Jane Smith', '789 Bill St', 'Bill City', NULL, NULL)"
        )?
        .execute(&pool)
        .await?;

        let user_with_addr: UserWithAddresses = sql_as!(
            "SELECT id, name, billing_street, billing_city, shipping_street, shipping_city
             FROM user_addresses WHERE id = 1"
        )?
        .fetch_one(&pool)
        .await?;

        println!("User with optional address (None case): {user_with_addr:?}");

        // Verify billing address is present but shipping is None
        assert_eq!(user_with_addr.billing_address.street, "789 Bill St");
        assert_eq!(user_with_addr.billing_address.city, "Bill City");
        assert!(user_with_addr.shipping_address.is_none());
    }

    // Test optional flattened struct (Some case)
    {
        sql!("INSERT INTO user_addresses 
              (id, name, billing_street, billing_city, shipping_street, shipping_city)
              VALUES (2, 'Bob Wilson', '321 Pay Ave', 'Payment Town', '654 Ship Rd', 'Shipping City')")?
            .execute(&pool)
            .await?;

        let user_with_addr: UserWithAddresses = sql_as!(
            "SELECT id, name, billing_street, billing_city, shipping_street, shipping_city
             FROM user_addresses WHERE id = 2"
        )?
        .fetch_one(&pool)
        .await?;

        println!("User with optional address (Some case): {user_with_addr:?}");

        // Verify both addresses are present
        assert_eq!(user_with_addr.billing_address.street, "321 Pay Ave");
        assert_eq!(user_with_addr.billing_address.city, "Payment Town");
        assert!(user_with_addr.shipping_address.is_some());
        if let Some(shipping_addr) = &user_with_addr.shipping_address {
            assert_eq!(shipping_addr.street, "654 Ship Rd");
            assert_eq!(shipping_addr.city, "Shipping City");
        }
    }

    println!("All FromRow derive features working correctly!");

    Ok(())
}
