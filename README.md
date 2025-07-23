<b>Hello curious person - musq is not yet ready for use! Please check back later.</b>

<h1 align="center">Musq</h1>

Musq is an async SQLite crate library for Rust.

# Rows

## #[derive(FromRow)]

The `FromRow` derive macro automatically generates an implementation of the `FromRow` trait for structs, enabling type-safe deserialization from SQL query results. This allows you to map database rows directly to Rust structs.

### Basic Usage

```rust
#[derive(musq::FromRow)]
struct User {
    id: i32,
    name: String,
    email: String,
}

// Query directly into your struct
let user: User = query_as("SELECT id, name, email FROM users WHERE id = ?")
    .bind(1)
    .fetch_one(&mut conn)
    .await?;
```

### Field Attributes

#### `#[musq(rename = "column_name")]`
Maps a struct field to a database column with a different name:

```rust
#[derive(FromRow)]
struct User {
    id: i32,
    #[musq(rename = "full_name")]
    name: String,
}
```

#### `#[musq(rename_all = "case_style")]` (struct-level)
Automatically converts all field names to a specific case style. Supported values: `snake_case` (default), `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE`, `kebab-case`, `verbatim`:

```rust
#[derive(FromRow)]
#[musq(rename_all = "camelCase")]
struct UserPost {
    user_id: i32,  // maps to "userId" column
    post_title: String,  // maps to "postTitle" column
}
```

#### `#[musq(default)]`
Uses `Default::default()` if the column is missing:

```rust
#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
    #[musq(default)]
    bio: Option<String>,  // Will be None if column missing
}
```

#### `#[musq(flatten)]`
Embeds another struct that implements `FromRow`:

```rust
#[derive(FromRow)]
struct Address {
    street: String,
    city: String,
    country: String,
}

#[derive(FromRow)]
struct User {
    id: i32,
    name: String,
    #[musq(flatten)]
    address: Address,  // Uses Address::from_row()
}
```

#### `#[musq(flatten, prefix = "prefix_")]`
Adds a prefix to column names when using nested structures:

```rust
#[derive(FromRow)]
struct User {
    id: i32,
    #[musq(flatten, prefix = "billing_")]
    billing_address: Address,  // Looks for "billing_street", "billing_city", etc.
    #[musq(flatten, prefix = "shipping_")]
    shipping_address: Address,
}
```

#### `#[musq(skip)]`
Always uses `Default::default()`, ignoring database columns:

```rust
#[derive(FromRow)]
struct User {
    name: String,
    #[musq(skip)]
    cached_data: Vec<String>,  // Always empty
}
```

#### `#[musq(try_from = "database_type")]`
Converts from a database type using `TryFrom`:

```rust
#[derive(FromRow)]
struct User {
    id: i32,
    #[musq(try_from = "i64")]
    score: u32,  // Converts i64 from DB to u32
}
```

### Complex Example

```rust
#[derive(FromRow)]
#[musq(rename_all = "camelCase")]
struct ComplexUser {
    id: i32,
    #[musq(rename = "full_name")]
    display_name: String,
    #[musq(default)]
    bio: Option<String>,
    #[musq(flatten)]
    address: Address,
    #[musq(flatten, prefix = "work_")]
    work_address: Address,
    #[musq(skip)]
    metadata: HashMap<String, String>,
    #[musq(try_from = "i64")]
    score: u32,
}
```

# Types

Types are discrete values that can be stored in a table column or appear in SQL expressions. Supported types implement
one or both of the `Encode` and `Decode` traits. `Encode` is used to convert a Rust value into a SQLite value, and
`Decode` is used to convert a SQLite value into a Rust value.

## Built-in type support

`Encode` and `Decode` are implemented for a set of standard types.

| Rust type                             | SQLite type(s)      |
|---------------------------------------|---------------------|
| `bool`                                | BOOLEAN             |
| `i8`, `i16`, `i32`, `i64`             | INTEGER             |
| `u8`, `u16`, `u32`                    | INTEGER             |
| `f32`, `f64`                          | REAL                |
| `&str`, `String`, `Arc<String>`       | TEXT                |
| `&[u8]`, `Vec<u8>`, `Arc<Vec<u8>`     | BLOB                |
| `time::PrimitiveDateTime`             | DATETIME            |
| `time::OffsetDateTime`                | DATETIME            |
| `time::Date`                          | DATE                |
| `time::Time`                          | TIME                |
| `bstr::BString`                       | BLOB                |


## Deriving types

You can derive `Encode` and `Decode` for a set of common custom type formats, or derive both at once with the `Codec`
derive.

<table>
<tr>
<td>

```rust
#[derive(musq::Codec)]
enum Foo {OneTwo, ThreeFour}
```

Enum stored as a string in snake case (the default): "one_two", "three_four".

</td>

<td>

```rust
#[derive(musq::Codec)]
#[musq(rename_all = "lower_case")]
enum Foo {OneTwo, ThreeFour}
```

Enum stored as a lowercase string: "onetwo", "threefour".

</td>

</tr>

<tr>

<td>

```rust
#[derive(musq::Codec)]
#[musq(repr = "i32")]
enum Foo {One, Two}
```

Enum stored as an **i32**: 0, 1.

</td>

<td>

```rust
#[derive(musq::Codec)]
struct Foo(i32)
```

A ["newtype"](https://doc.rust-lang.org/rust-by-example/generics/new_types.html) struct stored as an **i32**.

</td>

</tr>
</table>


## #[derive(Json)]

The `musq::Json` derive implements `Encode` and `Decode` for any type that implements `serde::Serialize` and
`serde::Deserialize`.

```rust
#[derive(musq::Json, serde::Serialize, serde::Deserialize)]
struct Foo {
    id: i32,
    name: String,
}
```


# Handling large blobs

Musq fans out inserts into a pool of workers, so it must be able to share query arguments between threads. Say we're
trying to construct an insert as follows:

```rust
query("INSERT INTO docs (txt) VALUES (?)").bind(s)
```

If `s` is a `&str` reference, Musq has to clone the value into an owned structure so it can control the lifetime and
thread sharing. This is usually fine, but if `s` is large, we can avoid the copy by passing an owned `String` or an
`Arc<String>` instead. The same idea holds for the reference `&[u8]` and its counterparts `Vec<u8>` and `Arc<Vec<u8>>`.

When fetching large blobs you can decode directly into an `Arc<Vec<u8>>` to
reduce copying and easily share the data:

```rust
let blob: Arc<Vec<u8>> = query_scalar("SELECT data FROM blob_test")
    .fetch_one(&mut conn)
    .await?;
```

## Named parameters

Musq supports the standard SQLite parameter syntax with `:name` and `@name` in
addition to positional placeholders. Values can be supplied positionally using
[`bind`](#) or directly by name with [`bind_named`]:

```rust
query("SELECT :foo, @bar")
    .bind_named(":foo", 1)
    .bind_named("@bar", 2);
```

If the same name appears multiple times it is bound from the first matching
value.

Named parameters can be mixed freely with positional placeholders and used in
normal SQL statements:

```rust
query("INSERT INTO users (id, name) VALUES (:id, ?)")
    .bind_named("id", 5_i32)
    .bind("Bob");

let (name,): (String,) = query_as("SELECT name FROM users WHERE id = :id")
    .bind_named("id", 5_i32)
    .fetch_one(&mut conn)
    .await?;
assert_eq!(name, "Bob");
```

# Development


## Why?

Musq is a SQLite-focused fork of SQLx. The aims are to simplify and clean up the codebase, strip out un-needed features and complexity, add new features, improve testing and ergonomics, and support WASM.


## Profiling

Run the benchmarks with profiling enabled:

```sh
cargo bench --bench benchmark -- --profile-time 10
```

The resulting flamegraphs are in `./targets/criterion/*/profile`. At the moment, the benchmarks are only supported on
Linux.
