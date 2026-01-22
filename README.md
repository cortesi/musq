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

<!-- snips: crates/musq/examples/readme_quickstart.rs -->
```rust
//! Quickstart example from the README.

use musq::{FromRow, Musq, sql, sql_as, values};

/// User record fetched from the database.
#[derive(Debug, FromRow)]
pub struct User {
    /// User ID.
    pub id: i32,
    /// User name.
    pub name: String,
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
    let user_values = values! { "id": id, "name": name }?;
    sql!("INSERT INTO users {insert:user_values}")?
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

-----

## Core Features

### Connection Pooling

Use `Musq::open()` or `Musq::open_in_memory()` to create a `Pool`, which manages multiple
connections for you. Queries can be executed directly on the pool.

<!-- snips: crates/musq/examples/readme_snippets.rs#pool -->
```rust
use musq::Musq;
let pool = Musq::new().max_connections(10).open_in_memory().await?;
```

### Querying

The recommended way to build queries in `musq` is with the `sql!` and `sql_as!` macros. For
dynamic queries, you can combine queries created with `sql!` using `Query::join`.

#### `sql!` and `sql_as!` Macros

These macros offer a flexible, `format!`-like syntax for building queries with positional or
named arguments, and even dynamic identifiers.

<!-- snips: crates/musq/examples/readme_snippets.rs#sql_basic -->
```rust
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
```

The `sql!` macro also supports dynamic identifiers and lists for `IN` clauses:

<!-- snips: crates/musq/examples/readme_snippets.rs#sql_in -->
```rust
let table_name = "users";
let user_ids = vec![1, 2, 3];
let columns = ["id", "name"];

// Dynamic table and column identifiers
let users: Vec<User> = sql_as!(
    "SELECT {idents:columns} FROM {ident:table_name} WHERE id IN ({values:user_ids})"
)?
.fetch_all(&pool)
.await?;
```


#### Query Composition with `Values`

It turns out that a very large portion of SQL query composition can be done by
treating a key/value map specially based on the query context. Musq provides a
`Values` type for this, along with a set of placeholder variants to cover all
cases where SQL uses key/value pairs.

##### The `values!` macro

`values!` builds a `Values` collection from literal column names and any `Encode`-able values (or
expression fragments from `musq::expr`). It
encodes each value immediately and returns `Result<Values>`, so construction can fail and you can
use `?` to surface encoding errors early. Keys must be string literals; for dynamic keys, use the
fluent builder. `Values` preserves insertion order, so the order you write is retained.

Because `values!` uses the same `Encode` implementations as regular query arguments, blob storage
behavior is unchanged. Types like `Vec<u8>`, `&[u8]`, `Arc<Vec<u8>>`, and `bstr::BString` continue
to map to SQLite `BLOB` when used inside `values!`.

It can be constructed with the `values!` macro:

<!-- snips: crates/musq/examples/readme_snippets.rs#values-macro -->
```rust
let vals = values! {
    "id": 1,
    "name": "Alice",
    "status": "active"
}?;
```

Or the fluent builder interface on the `Values` type:

<!-- snips: crates/musq/examples/readme_snippets.rs#values-fluent -->
```rust
let vals = Values::new()
    .val("id", 1)?
    .val("name", "Alice")?
    .val("status", "active")?;
```


The `sql!` macro provides special placeholder types for `Values`:

  * `{insert:values}`: Expands to `(col1, col2) VALUES (?, ?)` for `INSERT`
    statements.
  * `{set:values}`: Expands to `col1 = ?, col2 = ?` for `UPDATE` statements.
  * `{where:values}`: Expands to `col1 = ? AND col2 = ?`. If a value is `NULL` it
    expands to `col IS NULL` (without binding a parameter). If `values` is empty,
    it expands to `1=1`.
  * `{upsert:values, exclude: id, created_at}`: For `ON CONFLICT ... DO UPDATE SET`,
    expands to `col1 = excluded.col1, ...`, with an option to exclude certain
    keys from the update.

##### Computed values (expressions)

For some schemas you may want DB-side computed values (for example, a consistent `updated_at`
timestamp inside a transaction, or JSONB encoding via `jsonb(...)`). Musq supports this by allowing
expression fragments inside `Values` (via `musq::expr`).

<!-- snips: crates/musq/examples/readme_snippets.rs#values-expr -->
```rust
let changes = values! {
    "updated_at": musq::expr::now_rfc3339_utc(),
    "payload": musq::expr::jsonb(r#"{"event":"hello"}"#),
}?;

sql!("UPDATE events SET {set:changes} WHERE id = 1")?
    .execute(&pool)
    .await?;
```

Expressions created via `expr::raw(...)` taint the resulting query. Prefer the curated helpers when
possible.


<!-- snips: crates/musq/examples/readme_snippets.rs#values -->
```rust
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
```

For the most complex scenarios, you can drop down to the `QueryBuilder` for fine-grained
control over the generated SQL.

-----


## Types

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

Musq implements `Encode` for `Option<T>` where `T` implements `Encode`.
This makes it easy to work with nullable database columns:

<!-- snips: crates/musq/examples/readme_snippets.rs#values-optional -->
```rust
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
```

When you use `Some(value)`, it encodes the inner value normally. When you use `None`, it encodes as SQL `NULL`. This works seamlessly with all operations:

- **INSERT**: `None` values become `NULL` in the database
- **UPDATE**: `None` values set columns to `NULL`  
- **WHERE**: `None` values generate `column = NULL` (which is always false in SQL - use explicit `IS NULL` to match NULL values)
- **UPSERT**: `None` values participate in conflict resolution as `NULL`

Musq also provides a `Null` constant that you can use directly in `values!`
blocks without specifying a type:

<!-- snips: crates/musq/examples/readme_snippets.rs#values-null -->
```rust
use musq::{Null, values};
let user_data = values! {
    "name": "Alice",
    "email": Null,           // No type annotation needed
}?;
```

**Note on Large Values:** When passing large string or byte values to queries, consider using
owned types like `String`, `Vec<u8>`, or `Arc<T>` to avoid unnecessary copies. Similarly, you
can decode large blobs directly into `Arc<Vec<u8>>` for efficient, shared access to the
data.

**Note on `bstr`:** Use `bstr::BString` when you need to handle byte data from a `BLOB` column
that is text-like but may not contain valid UTF-8. It provides string-like operations without
enforcing UTF-8 validity.

### Custom Types

You can easily implement `Encode` and `Decode` for your own types using derive
macros.

#### `#[derive(musq::Codec)]`

The `Codec` derive implements both `Encode` and `Decode` for simple enums and
newtype structs.

#### Enums as strings

Stores the enum as a snake-cased string (e.g., `"open"`, `"closed"`).

<!-- snips: crates/musq/examples/readme_snippets.rs#text_enum -->
```rust
#[derive(musq::Codec, Debug, PartialEq)]
enum Status {
    Open,
    Closed,
}
```

#### Enums as integers

Stores the enum as its integer representation.

<!-- snips: crates/musq/examples/readme_snippets.rs#num_enum -->
```rust
#[derive(musq::Codec, Debug, PartialEq)]
#[musq(repr = "i32")]
enum Priority {
    Low = 1,
    Medium = 2,
    High = 3,
}
```

#### Newtype structs

Stores the newtype as its inner value.

<!-- snips: crates/musq/examples/readme_snippets.rs#newtype -->
```rust
#[derive(musq::Codec, Debug, PartialEq)]
struct UserId(i32);
```

#### JSON-encoded structs

Stores any `serde`-compatible type as a JSON string in a `TEXT` column.

<!-- snips: crates/musq/examples/readme_snippets.rs#json -->
```rust
#[derive(musq::Json, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Metadata {
    tags: Vec<String>,
    version: i32,
}
```

-----

## Mapping Rows to Structs

The `FromRow` derive macro provides powerful options for mapping query results
to your structs.

### Basic Usage

<!-- snips: crates/musq/examples/readme_snippets.rs#fromrow_basic -->
```rust
#[derive(FromRow, Debug)]
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

    <!-- snips: crates/musq/examples/readme_snippets.rs#fromrow_fields -->
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

    <!-- snips: crates/musq/examples/readme_snippets.rs#fromrow_flatten -->
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


-----


## Development

Just like whales once used to be land-dwelling quadrupeds, Musq started life as a
focused fork of [SQLx](https://github.com/launchbadge/sqlx).
