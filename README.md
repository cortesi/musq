# Musq

![Discord](https://img.shields.io/discord/1381424110831145070?style=flat-square&logo=rust&link=https%3A%2F%2Fdiscord.gg%2FfHmRmuBDxF)
[![Crates.io](https://img.shields.io/crates/v/musq.svg)](https://crates.io/crates/musq)
[![Documentation](https://docs.rs/libruskel/badge.svg)](https://docs.rs/musq)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


**Musq is an asynchronous SQLite toolkit for Rust.**

It provides a set of tools to help you build applications that interact with a SQLite database,
with a strong focus on performance, correctness, and ergonomics.

-----

## Quickstart

Here's a brief example of using `musq` to connect to a database, run a query, and map the
result to a struct using the `sql!` and `sql_as!` macros.

```rust
use musq::{FromRow, Musq, sql, sql_as};

#[derive(Debug, FromRow)]
struct User {
    id: i32,
    name: String,
}

#[tokio::main]
async fn main() -> musq::Result<()> {
    // Create an in-memory database pool
    let pool = Musq::new().open_in_memory().await?;

    // Create a table
    sql!("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);")?
        .execute(&pool)
        .await?;

    // Insert a user
    let id = 1;
    let name = "Alice";
    sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
        .execute(&pool)
        .await?;

    // Fetch the user and map it to our struct
    let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
        .fetch_one(&pool)
        .await?;

    println!("Fetched user: {user:?}");

    Ok(())
}
```

-----

## Community

Want to contribute? Have ideas or feature requests? Come tell us about it on
[Discord](https://discord.gg/fHmRmuBDxF). 


## Core Features

### Connection Pooling

Use `Musq::open()` or `Musq::open_in_memory()` to create a `Pool`, which manages multiple
connections for you. Queries can be executed directly on the pool.

```rust
use musq::Musq;

let _pool = Musq::new()
    .max_connections(10)
    .open_in_memory()
    .await?;

// `pool` can now be shared across your application
```

### Querying

The recommended way to build queries in `musq` is with the `sql!` and `sql_as!` macros. For
dynamic queries, you can combine queries created with `sql!` using `Query::join`.

#### `sql!` and `sql_as!` Macros

These macros offer a flexible, `format!`-like syntax for building queries with positional or
named arguments, and even dynamic identifiers.

```rust
use musq::{sql, sql_as, FromRow};

#[derive(FromRow, Debug)]
struct User { 
    id: i32, 
    name: String 
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
```

The `sql!` macro also supports dynamic identifiers and lists for `IN` clauses:

```rust
let table_name = "users";
let user_ids = vec![1, 2, 3];
let columns = ["id", "name"];

// Dynamic table and column identifiers
let users: Vec<User> = sql_as!(
    "SELECT {idents:columns} FROM {ident:table_name} WHERE id IN ({values:user_ids})"
)?.fetch_all(&pool).await?;
```

#### Dynamic Query Composition

For dynamic queries, `musq` encourages a compositional approach. Start with a base query from
`sql!` and join additional clauses as needed using `Query::join`.

```rust
use musq::sql;

let mut query = sql!("SELECT * FROM products WHERE 1 = 1")?;

if let Some(category) = &params.category {
    query = query.join(sql!("AND category = {category}")?);
}

if let Some(min_price) = params.min_price {
    query = query.join(sql!("AND price >= {min_price}")?);
}
```

For the most complex scenarios, you can drop down to the `QueryBuilder` for fine-grained
control over the generated SQL.

-----

## Type Handling (`Encode` and `Decode`)

Musq uses the `Encode` and `Decode` traits to convert between Rust types and SQLite types.

### Built-in Types

Support for common Rust types is included out-of-the-box.

| Rust type                           | SQLite type(s) |
| ----------------------------------- | -------------- |
| `bool`                              | BOOLEAN        |
| `i8`, `i16`, `i32`, `i64`           | INTEGER        |
| `u8`, `u16`, `u32`                  | INTEGER        |
| `f32`, `f64`                        | REAL           |
| `&str`, `String`, `Arc<String>`     | TEXT           |
| `&[u8]`, `Vec<u8>`, `Arc<Vec<u8>>` | BLOB           |
| `time::PrimitiveDateTime`           | DATETIME       |
| `time::OffsetDateTime`              | DATETIME       |
| `time::Date`                        | DATE           |
| `time::Time`                        | TIME           |
| `bstr::BString`                     | BLOB           |

**Note on Large Values:** When passing large string or byte values to queries, consider using
owned types like `String`, `Vec<u8>`, or `Arc<T>` to avoid unnecessary copies. Similarly, you
can decode large blobs directly into `Arc<Vec<u8>>` for efficient, shared access to the
data.

**Note on `bstr`:** Use `bstr::BString` when you need to handle byte data from a `BLOB` column
that is text-like but may not contain valid UTF-8. It provides string-like operations without
enforcing UTF-8 validity.

### Derivable Traits for Custom Types

You can easily implement `Encode` and `Decode` for your own types using derive macros.

#### `#[derive(musq::Codec)]`

The `Codec` derive implements both `Encode` and `Decode`. It's suitable for simple enums and
newtype structs.

**For Enums (as TEXT):**
Stores the enum as a snake-cased string (e.g., `"open"`, `"closed"`).

```rust
#[derive(musq::Codec, Debug, PartialEq)]
enum Status {
    Open,
    Closed,
}
```

**For Enums (as INTEGER):**
Stores the enum as its integer representation.

```rust
#[derive(musq::Codec, Debug, PartialEq)]
#[musq(repr = "i32")]
enum Priority {
    Low = 1,
    Medium = 2,
    High = 3,
}
```

**For Newtype Structs:**
Stores the newtype as its inner value.

```rust
#[derive(musq::Codec, Debug, PartialEq)]
struct UserId(i32);
```

#### `#[derive(musq::Json)]`

Stores any `serde`-compatible type as a JSON string in a `TEXT` column.

```rust
#[derive(musq::Json, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Metadata {
    tags: Vec<String>,
    version: i32,
}
```

-----

## Mapping Rows to Structs with `#[derive(FromRow)]`

The `FromRow` derive macro provides powerful options for mapping query results to your structs.

### Basic Usage

```rust
#[derive(musq::FromRow, Debug)]
struct User {
    id: i32,
    name: String,
    email: String,
}
```

### Field Attributes

  - `#[musq(rename = "column_name")]`: Maps a field to a column with a different name.

  - `#[musq(rename_all = "...")]`: (On struct) Converts all field names to a specific case
    style (e.g., `"camelCase"`, `"snake_case"`).

  - `#[musq(default)]`: Uses the field type's `Default::default()` value if the column is missing
    from the result set.

  - `#[musq(flatten)]`: Embeds another struct that also implements `FromRow`. This is useful for
    mapping columns to a nested struct, especially from `JOIN`s.

    If the flattened field is wrapped in an `Option`, it will be `None` if and only if **all**
    columns for the nested struct are `NULL`.

    ```rust
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
    ```

  - `#[musq(flatten, prefix = "prefix_")]`: Adds a prefix to all column names when
    flattening a nested struct. This is useful for avoiding name collisions and can be
    combined with `Option`.

    ```rust
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
    ```

  - `#[musq(skip)]`: Always uses `Default::default()` for the field, ignoring the database.

  - `#[musq(try_from = "database_type")]`: Converts a column from a specific database type
    using `TryFrom`.

### Example

```rust
#[derive(FromRow, Debug)]
struct Address {
    street: String,
    city: String,
}

#[derive(FromRow, Debug)]
struct User {
    id: i32,
    full_name: String,
    
    #[musq(default)]
    bio: Option<String>,
    
    // Looks for `street` and `city`.
    #[musq(flatten)]
    address: Address,
}
```

-----

## Helpers

### `insert_into` Builder

For programmatic `INSERT` statements, the `insert_into` builder provides a convenient fluent
API.

```rust
use musq::insert_into;

// Construct the query
let insert_query = insert_into("users")
    .value("id", 4)
    .value("name", "Bob")
    .query()?;

insert_query.execute(&pool).await?;

// The builder can also execute directly on any executor (pool, connection, transaction, etc.)
insert_into("users")
    .value("id", 5)
    .value("name", "Carol")
    .execute(&pool)
    .await?;
```

-----


## Development

Just like whales once used to be a land-dwelling quadrupeds, Musq started as a
focused fork of [SQLx](https://github.com/launchbadge/sqlx).
