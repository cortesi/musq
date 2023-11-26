<b>Hello curious person - musq is not yet ready for use! Please check back later.</b>

<h1 align="center">Musq</h1>

Musq is an async SQLite crate library for Rust.

# Rows


# Types

Types are discrete values that can be stored in a table column or appear in SQL expressions.

## Built-in Types

The following built-in types are supported:

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


## #[derive(Json)]

You can derive a JSON type with the `musq::Json` derive, as long as the type implements `serde::Serialize` and
`serde::Deserialize`. JSON types are stored as TEXT.

```rust
#[derive(musq::Json, serde::Serialize, serde::Deserialize)]
struct Foo {
    id: i32,
    name: String,
}
```

## #[derive(Type)]

You can derive custom types with the `musq::Type` derive.

<table>
<tr>
<td>

```rust
#[derive(musq::Type)]
enum Foo {OneTwo, ThreeFour}
```

Enum stored as a string in snake case (the default): "one_two", "three_four".

</td>

<td>

```rust
#[derive(musq::Type)]
#[musq(rename_all = "lower_case")]
enum Foo {OneTwo, ThreeFour}
```

Enum stored as a lowercase string: "onetwo", "threefour".

</td>

</tr>

<tr>

<td>

```rust
#[derive(musq::Type)]
#[musq(repr = "i32")]
enum Foo {One, Two}
```

Enum stored as an **i32**: 0, 1.

</td>

<td>

```rust
#[derive(musq::Type)]
struct Foo(i32)
```

A ["newtype"](https://doc.rust-lang.org/rust-by-example/generics/new_types.html) struct stored as an **i32**.

</td>

</tr>
</table>


# Handling large blobs

Musq fans out inserts into a pool of workers, so it must be able to share query arguments between threads. Say we're
trying to construct an insert as follows:

```rust
query("INSERT INTO docs (txt) VALUES (?)").bind(s)
```

If `s` is a `&str` reference, Musq has to clone the value into an owned structure so it can control the lifetime and
thread sharing. This is usually fine, but if `s` is large, we can avoid the copy by passing an owned `String` or an
`Arc<String>` instead. The same idea holds for the reference `&[u8]` and its counterparts `Vec<u8>` and `Arc<Vec<u8>>`.

FIXME: Add note on efficiently querying large blobs

# Development


## Why?

Musq is a SQLite-focused fork of sqlx. The aims are to simplify and clean up the codebase, strip out un-needed features and complexity, add new features, improve testing and ergonomics, and support WASM.


## Profiling

Run the benchmarks with profiling enabled:

```sh
cargo bench --bench benchmark -- --profile-time 10
```

The resulting flamegraphs are in `./targets/criterion/*/profile`. At the moment, the benchmarks are only supported on
Linux.