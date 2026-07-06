# Musq

![Discord](https://img.shields.io/discord/1381424110831145070?style=flat-square&logo=rust&link=https%3A%2F%2Fdiscord.gg%2FfHmRmuBDxF)
[![Crates.io](https://img.shields.io/crates/v/musq.svg)](https://crates.io/crates/musq)
[![Documentation](https://docs.rs/musq/badge.svg)](https://docs.rs/musq)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Musq is an asynchronous SQLite toolkit for Rust.**

Musq bundles its own SQLite, runs each connection on a dedicated worker thread
behind an async API, enables foreign key enforcement by default, and ships with
[sqlite-vec](https://github.com/asg017/sqlite-vec) vector search built in.

## Quickstart

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

## Opening a database

`Musq` is the options builder. `open()` and `open_in_memory()` return a `Pool`;
queries execute directly on the pool, on a `Connection`, a `PoolConnection`, or
a `Transaction` — all interchangeably.

<!-- snips: crates/musq/examples/readme_snippets.rs#open -->
```rust
use musq::{JournalMode, Musq};

let pool = Musq::new()
    .max_connections(10)
    .create_if_missing(true)
    .journal_mode(JournalMode::Wal)
    .open("app.db")
    .await?;
```

Defaults: foreign keys on, busy timeout 5s, 10 pool connections, journal mode
left unchanged (set `JournalMode::Wal` explicitly for WAL). Any pragma can be
set with `.pragma(key, value)`.

## Queries

`sql!` builds a query from a `format!`-like string. `sql_as!` does the same and
maps rows to a `FromRow` type. Interpolated values are always bound as
parameters, never spliced into the SQL.

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

sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
    .execute(&pool)
    .await?;

let user: User = sql_as!("SELECT id, name FROM users WHERE id = {id}")?
    .fetch_one(&pool)
    .await?;
```

Placeholders:

| Placeholder                       | Expansion                                                     |
| --------------------------------- | ------------------------------------------------------------- |
| `{expr}`, `{}`                    | one bound parameter                                           |
| `{values:list}`                   | `?, ?, ?` — one parameter per element, for `IN (...)`         |
| `{ident:expr}`                    | one quoted identifier                                         |
| `{idents:list}`                   | comma-separated quoted identifiers                            |
| `{insert:values}`                 | `(col, ...) VALUES (?, ...)`                                  |
| `{set:values}`                    | `col = ?, ...`                                                |
| `{where:values}`                  | `col = ? AND ...`; `col IS NULL` for NULLs; `1=1` when empty  |
| `{upsert:values, exclude: a, b}`  | `col = excluded.col, ...` for `ON CONFLICT ... DO UPDATE SET` |
| `{raw:expr}`                      | verbatim SQL; taints the query                                |

<!-- snips: crates/musq/examples/readme_snippets.rs#sql_in -->
```rust
let table_name = "users";
let user_ids = vec![1, 2, 3];
let columns = ["id", "name"];

let users: Vec<User> = sql_as!(
    "SELECT {idents:columns} FROM {ident:table_name} WHERE id IN ({values:user_ids})"
)?
.fetch_all(&pool)
.await?;
```

Run queries with `.execute()`, `.fetch_one()`, `.fetch_optional()`,
`.fetch_all()`, or `.fetch()` (a `Stream`). Compose dynamic queries with
`Query::join`, or drop down to `QueryBuilder` for full control.

## Values

`Values` is an insertion-ordered column/value map consumed by the `{insert:}`,
`{set:}`, `{where:}`, and `{upsert:}` placeholders. Build one with the
`values!` macro or fluently with `Values::new().val(k, v)?`. Each value is
encoded immediately, so construction returns `Result`.

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

`Option::None` encodes as SQL `NULL` everywhere (`{where:}` renders it as
`col IS NULL`), and `musq::Null` is an untyped NULL literal:

<!-- snips: crates/musq/examples/readme_snippets.rs#values-null -->
```rust
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
```

DB-side computed values come from `musq::expr` (`now_rfc3339_utc`, `jsonb`,
`jsonb_text`, `jsonb_serde`, `raw`):

<!-- snips: crates/musq/examples/readme_snippets.rs#values-expr -->
```rust
use musq::expr;
let changes = values! {
    "updated_at": expr::now_rfc3339_utc(),
    "payload": expr::jsonb(r#"{"event":"hello"}"#),
}?;

sql!("UPDATE events SET {set:changes} WHERE id = 1")?
    .execute(&pool)
    .await?;
```

`expr::raw(...)` taints the resulting query; prefer the curated helpers.

## Transactions

`Pool::begin` and `Connection::begin` return a `Transaction`. Calling `begin`
on an existing transaction creates a savepoint. Dropping an uncommitted
transaction rolls it back. `Connection::transaction` runs a closure and
commits or rolls back based on its result.

<!-- snips: crates/musq/examples/readme_snippets.rs#transaction -->
```rust
let mut tx = pool.begin().await?;
sql!("INSERT INTO users (id, name) VALUES ({id}, {name})")?
    .execute(&tx)
    .await?;
tx.commit().await?;
```

## Types

The `Encode` and `Decode` traits convert between Rust and SQLite types:

| Rust type                          | SQLite type |
| ---------------------------------- | ----------- |
| `bool`                             | BOOLEAN     |
| `i8`, `i16`, `i32`, `i64`          | INTEGER     |
| `u8`, `u16`, `u32`                 | INTEGER     |
| `f32`, `f64`                       | REAL        |
| `&str`, `String`, `Arc<String>`    | TEXT        |
| `&[u8]`, `Vec<u8>`, `Arc<Vec<u8>>` | BLOB        |
| `bstr::BString`                    | BLOB        |
| `time::OffsetDateTime`             | DATETIME    |
| `time::PrimitiveDateTime`          | DATETIME    |
| `time::Date`                       | DATE        |
| `time::Time`                       | TIME        |
| `VecF32`, `VecInt8`, `VecBit`      | BLOB        |

`Option<T>` maps `None` to `NULL`. For large strings and blobs, prefer owned
or shared types (`String`, `Vec<u8>`, `Arc<T>`) to avoid copies; blobs can be
decoded directly into `Arc<Vec<u8>>`. `bstr::BString` handles text-like BLOBs
that may not be valid UTF-8.

### Derived types

`#[derive(musq::Codec)]` implements both `Encode` and `Decode` for enums and
newtype structs (`Encode` and `Decode` can also be derived individually).

Enums store as snake-cased strings (`"open"`, `"closed"`) by default:

<!-- snips: crates/musq/examples/readme_snippets.rs#text_enum -->
```rust
#[derive(musq::Codec, Debug, PartialEq)]
enum Status {
    Open,
    Closed,
}
```

Or as integers with `repr`:

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

Newtype structs store as their inner value:

<!-- snips: crates/musq/examples/readme_snippets.rs#newtype -->
```rust
#[derive(musq::Codec, Debug, PartialEq)]
struct UserId(i32);
```

`#[derive(musq::Json)]` stores any serde-compatible type as JSON text:

<!-- snips: crates/musq/examples/readme_snippets.rs#json -->
```rust
#[derive(musq::Json, serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Metadata {
    tags: Vec<String>,
    version: i32,
}
```

## Row mapping

`#[derive(FromRow)]` maps columns to fields by name. Attributes:

- `#[musq(rename = "...")]` — map a field to a differently named column
- `#[musq(rename_all = "...")]` — on the struct; case-convert all field names
- `#[musq(default)]` — use `Default::default()` when the column is absent
- `#[musq(skip)]` — always use `Default::default()`
- `#[musq(try_from = "T")]` — decode as `T`, then convert with `TryFrom`
- `#[musq(deserialize_with = "path")]` — decode with a custom
  `fn(prefix: &str, row: &Row) -> Result<T>`
- `#[musq(flatten)]`, `#[musq(flatten, prefix = "...")]` — embed a nested
  `FromRow` struct, optionally prefixing its column names; an `Option` nested
  struct is `None` iff all of its columns are NULL

<!-- snips: crates/musq/examples/readme_snippets.rs#flatten -->
```rust
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
```

## Vector search

The default `vec` feature registers sqlite-vec on every connection and
provides the `VecF32`, `VecInt8`, and `VecBit` types. `VecF32` binds directly
to vector functions and `vec0` tables; `VecInt8` and `VecBit` must be wrapped
in SQL as `vec_int8(?)` and `vec_bit(?)`.

```toml
musq = { version = "0.0.4", default-features = false }  # opt out of vec
```

See the end-to-end example: `cargo run -p musq --example vec`.

## SQLite runtime

Musq supports exactly the SQLite release bundled by its `libsqlite3-sys`
dependency — currently SQLite 3.53.2 via `libsqlite3-sys 0.38.1`. Linking
against older or system SQLite libraries is not supported; leave
`LIBSQLITE3_SYS_USE_PKG_CONFIG` and `SQLITE3_*` environment variables unset.

Runtime introspection and control:

- `runtime_info()` — SQLite version, source ID, and compile options
- `db_status(kind, reset)` — per-connection status counters (page cache,
  lookaside, schema, statements, ...)
- `wal_checkpoint(schema, mode)` — run or inspect WAL checkpoints

## Community

Questions, ideas, feature requests: [Discord](https://discord.gg/fHmRmuBDxF).

Just like whales once used to be land-dwelling quadrupeds, Musq started life
as a focused fork of [SQLx](https://github.com/launchbadge/sqlx).
