<b>Hello curious person - musq is not yet ready for use! Please check back later.</b>

<h1 align="center">Musq</h1>

Musq is an async SQLite crate library for Rust.

# Rows


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