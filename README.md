<h1 align="center">musqlite</h1>

SQLx is an async, pure Rust<sub>†</sub> SQL crate featuring compile-time checked queries without a DSL.

-   **Truly Asynchronous**. Built from the ground-up using async/await for maximum concurrency.

-   **Compile-time checked queries** (if you want). See [SQLx is not an ORM](#sqlx-is-not-an-orm).

<small><small>

---

-   Cross-platform. Being native Rust, SQLx will compile anywhere Rust is supported.

-   Built-in connection pooling with `sqlx::Pool`.

-   Row streaming. Data is read asynchronously from the database and decoded on demand.

-   Automatic statement preparation and caching. When using the high-level query API (`sqlx::query`), statements are
    prepared and cached per connection.

-   Simple (unprepared) query execution including fetching results into the same `Row` types used by
    the high-level API. Supports batch execution and returns results from all statements.

-   Asynchronous notifications using `LISTEN` and `NOTIFY` for [PostgreSQL].

-   Nested transactions with support for save points.

-   `Any` database driver for changing the database driver at runtime. An `AnyPool` connects to the driver indicated by the URL scheme.

#### Cargo Feature Flags

-   `runtime-tokio`: Use the `tokio` runtime.

-   `any`: Add support for the `Any` database driver, which can proxy to a database driver at runtime.

-   `macros`: Add support for the `query*!` macros, which allows compile-time checked queries.

-   `uuid`: Add support for UUID (in Postgres).

-   `chrono`: Add support for date and time types from `chrono`.

-   `time`: Add support for date and time types from `time` crate (alternative to `chrono`, which is preferred by `query!` macro, if both enabled)

-   `bstr`: Add support for `bstr::BString`.

-   `ipnetwork`: Add support for `INET` and `CIDR` (in postgres) using the `ipnetwork` crate.

-   `json`: Add support for `JSON` and `JSONB` (in postgres) using the `serde_json` crate.

## SQLx is not an ORM!

SQLx supports **compile-time checked queries**. It does not, however, do this by providing a Rust
API or DSL (domain-specific language) for building queries. Instead, it provides macros that take
regular SQL as input and ensure that it is valid for your database. The way this works is that
SQLx connects to your development DB at compile time to have the database itself verify (and return
some info on) your SQL queries. This has some potentially surprising implications:

-   Since SQLx never has to parse the SQL string itself, any syntax that the development DB accepts
    can be used (including things added by database extensions)
-   Due to the different amount of information databases let you retrieve about queries, the extent of
    SQL verification you get from the query macros depends on the database

**If you are looking for an (asynchronous) ORM,** you can check out [`ormx`] or [`SeaORM`], which is built on top
of SQLx.

[`ormx`]: https://crates.io/crates/ormx
[`SeaORM`]: https://github.com/SeaQL/sea-orm

## Usage

### Querying

In SQL, queries can be separated into prepared (parameterized) or unprepared (simple). Prepared queries have their
query plan _cached_, use a binary mode of communication (lower bandwidth and faster decoding), and utilize parameters
to avoid SQL injection. Unprepared queries are simple and intended only for use where a prepared statement
will not work, such as various database commands (e.g., `PRAGMA` or `SET` or `BEGIN`).

SQLx supports all operations with both types of queries. In SQLx, a `&str` is treated as an unprepared query,
and a `Query` or `QueryAs` struct is treated as a prepared query.

```rust
// low-level, Executor trait
conn.execute("BEGIN").await?; // unprepared, simple query
conn.execute(sqlx::query("DELETE FROM table")).await?; // prepared, cached query
```

We should prefer to use the high-level `query` interface whenever possible. To make this easier, there are finalizers
on the type to avoid the need to wrap with an executor.

```rust
sqlx::query("DELETE FROM table").execute(&mut conn).await?;
sqlx::query("DELETE FROM table").execute(&pool).await?;
```

The `execute` query finalizer returns the number of affected rows, if any, and drops all received results.
In addition, there are `fetch`, `fetch_one`, `fetch_optional`, and `fetch_all` to receive results.

The `Query` type returned from `sqlx::query` will return `Row<'conn>` from the database. Column values can be accessed
by ordinal or by name with `row.get()`. As the `Row` retains an immutable borrow on the connection, only one
`Row` may exist at a time.

The `fetch` query finalizer returns a stream-like type that iterates through the rows in the result sets.

```rust
// provides `try_next`
use futures::TryStreamExt;

let mut rows = sqlx::query("SELECT * FROM users WHERE email = ?")
    .bind(email)
    .fetch(&mut conn);

while let Some(row) = rows.try_next().await? {
    // map the row into a user-defined domain type
    let email: &str = row.try_get("email")?;
}
```

To assist with mapping the row into a domain type, one of two idioms may be used:

```rust
let mut stream = sqlx::query("SELECT * FROM users")
    .map(|row: PgRow| {
        // map the row into a user-defined domain type
    })
    .fetch(&mut conn);
```

```rust
#[derive(sqlx::FromRow)]
struct User { name: String, id: i64 }

let mut stream = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ? OR name = ?")
    .bind(user_email)
    .bind(user_name)
    .fetch(&mut conn);
```

Instead of a stream of results, we can use `fetch_one` or `fetch_optional` to request one required or optional result
from the database.
