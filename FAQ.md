SQLx Frequently Asked Questions
===============================

### How can I bind an array to a `VALUES()` clause? How can I do bulk inserts?

Like the above, SQLx currently does not support this in the general case right now but will in the future.

However, **Postgres** also has a feature to save the day here! You can pass an array to `UNNEST()` and
it will treat it as a temporary table:

```rust
let foo_texts: Vec<String> = vec![/* ... */];

sqlx::query!(
    // because `UNNEST()` is a generic function, Postgres needs the cast on the parameter here
    // in order to know what type to expect there when preparing the query
    "INSERT INTO foo(text_column) SELECT * FROM UNNEST($1::text[])",
    &foo_texts[..]
)
    .execute(&db)
    .await?;
```

`UNNEST()` can also take more than one array, in which case it'll treat each array as a column in the temporary table:

```rust
// this solution currently requires each column to be its own vector
// in the future we're aiming to allow binding iterators directly as arrays
// so you can take a vector of structs and bind iterators mapping to each field
let foo_texts: Vec<String> = vec![/* ... */];
let foo_bools: Vec<bool> = vec![/* ... */];
let foo_ints: Vec<i64> = vec![/* ... */];

sqlx::query!(
    "
        INSERT INTO foo(text_column, bool_column, int_column)
        SELECT * FROM UNNEST($1::text[], $2::bool[], $3::int8[])
    ",
    &foo_texts[..],
    &foo_bools[..],
    &foo_ints[..]
)
    .execute(&db)
    .await?;
```

Again, even with comma-expanded lists in the future this will likely still be the most performant way to run bulk inserts
with Postgres--at least until we get around to implementing an interface for `COPY FROM STDIN`, though
this solution with `UNNEST()` will still be more flexible as you can use it in queries that are more complex
than just inserting into a table.

Note that if some vectors are shorter than others, `UNNEST` will fill the corresponding columns with  `NULL`s
to match the longest vector.

For example, if `foo_texts` is length 5, `foo_bools` is length 4, `foo_ints` is length 3, the resulting table will
look like this:

| Row # | `text_column`  | `bool_column`  | `int_column`  |
| ----- | -------------- | -------------- | ------------- |
| 1     | `foo_texts[0]` | `foo_bools[0]` | `foo_ints[0]` |
| 2     | `foo_texts[1]` | `foo_bools[1]` | `foo_ints[1]` |
| 3     | `foo_texts[2]` | `foo_bools[2]` | `foo_ints[2]` |
| 4     | `foo_texts[3]` | `foo_bools[3]` | `NULL`        |
| 5     | `foo_texts[4]` | `NULL`         | `NULL`        |

See Also:
* [Postgres Manual, Section 7.2.1.4: Table Functions](https://www.postgresql.org/docs/current/queries-table-expressions.html#QUERIES-TABLEFUNCTIONS)
* [Postgres Manual, Section 9.19: Array Functions and Operators](https://www.postgresql.org/docs/current/functions-array.html)

----
### How do I compile with the macros without needing a database, e.g. in CI?

The macros support an offline mode which saves data for existing queries to a `.sqlx` directory,
so the macros can just read those instead of talking to a database.

See the following:

* [the docs for `query!()`](https://docs.rs/sqlx/0.5.5/sqlx/macro.query.html#offline-mode-requires-the-offline-feature)
* [the README for `sqlx-cli`](sqlx-cli/README.md#enable-building-in-offline-mode-with-query)

To keep `.sqlx` up-to-date you need to run `cargo sqlx prepare` before every commit that
adds or changes a query; you can do this with a Git pre-commit hook:

```shell
$ echo "cargo sqlx prepare > /dev/null 2>&1; git add .sqlx > /dev/null" > .git/hooks/pre-commit
```

Note that this may make committing take some time as it'll cause your project to be recompiled, and
as an ergonomic choice it does _not_ block committing if `cargo sqlx prepare` fails.

We're working on a way for the macros to save their data to the filesystem automatically which should be part of SQLx 0.7,
so your pre-commit hook would then just need to stage the changed files. This can be enabled by creating a directory
and setting the `SQLX_OFFLINE_DIR` environment variable to it before compiling.
Additionally, if you're not using Cargo or have a nonstandard setup, you may want to set the `SQLX_TMP`
variable in order to store temporary query files somewhere that isn't picked up by git.
These files should get cleaned up automatically, but they may not if there's a failure. For example:

```shell
$ mkdir .sqlx
$ export SQLX_OFFLINE_DIR="./.sqlx"`
$ # Optional and only useful if using a nonstandard setup, ensures temp files won't get picked up by git on failure
$ mkdir ./my-custom-target/sqlx
$ export SQLX_TMP="./my-custom-target/sqlx-tmp"
$ cargo check
```

However, this behaviour is not considered stable and it is still recommended to use `cargo sqlx prepare`.

----

### How do the query macros work under the hood?

The macros work by talking to the database at compile time. When a(n) SQL client asks to create a prepared statement
from a query string, the response from the server typically includes information about the following:

* the number of bind parameters, and their expected types if the database is capable of inferring that
* the number, names and types of result columns, as well as the original table and columns names before aliasing

In MySQL/MariaDB, we also get boolean flag signaling if a column is `NOT NULL`, however
in Postgres and SQLite, we need to do a bit more work to determine whether a column can be `NULL` or not.

After preparing, the Postgres driver will first look up the result columns in their source table and check if they have
a `NOT NULL` constraint. Then, it will execute `EXPLAIN (VERBOSE, FORMAT JSON) <your query>` to determine which columns
come from half-open joins (LEFT and RIGHT joins), which makes a normally `NOT NULL` column nullable. Since the
`EXPLAIN VERBOSE` format is not stable or completely documented, this inference isn't perfect. However, it does err on
the side of producing false-positives (marking a column nullable when it's `NOT NULL`) to avoid errors at runtime.

If you do encounter false-positives please feel free to open an issue; make sure to include your query, any relevant
schema as well as the output of `EXPLAIN (VERBOSE, FORMAT JSON) <your query>` which will make this easier to debug.

The SQLite driver will pull the bytecode of the prepared statement and step through it to find any instructions
that produce a null value for any column in the output.

---
### Why can't SQLx just look at my database schema/migrations and parse the SQL itself?

Take a moment and think of the effort that would be required to do that.

To implement this for a single database driver, SQLx would need to:

* know how to parse SQL, and not just standard SQL but the specific dialect of that particular database
* know how to analyze and typecheck SQL queries in the context of the original schema
* if inferring schema from migrations it would need to simulate all the schema-changing effects of those migrations

This is effectively reimplementing a good chunk of the database server's frontend,

_and_ maintaining and ensuring correctness of that reimplementation,

including bugs and idiosyncrasies,

for the foreseeable future,

for _every_ database we intend to support.

Even Sisyphus would pity us.

----

### Why does my project using sqlx query macros not build on docs.rs?

Docs.rs doesn't have access to your database, so it needs to be provided prepared queries in a `.sqlx` directory and be instructed to set the `SQLX_OFFLINE` environment variable to true while compiling your project. Luckily for us, docs.rs creates a `DOCS_RS` environment variable that we can access in a custom build script to achieve this functionality.

To do so, first, make sure that you have run `cargo sqlx prepare` to generate a `.sqlx` directory in your project.

Next, create a file called `build.rs` in the root of your project directory (at the same level as `Cargo.toml`). Add the following code to it:
```rs
fn main() {
    // When building in docs.rs, we want to set SQLX_OFFLINE mode to true
    if std::env::var_os("DOCS_RS").is_some() {
        println!("cargo:rustc-env=SQLX_OFFLINE=true");
    }
}
```
