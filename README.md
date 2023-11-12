<b>Hello curious person - musq is not yet ready for use! Please check back later.</b>

<h1 align="center">Musq</h1>

Musq is an async SQLite crate library for Rust.



# Derives

## Types

Types are discrete values that can be stored in a table column or appear in SQL expressions. The following built-in
types are supported:

| Rust type                             | SQLite type(s)      |
|---------------------------------------|---------------------|
| `bool`                                | BOOLEAN             |
| `i8`, `i16`, `i32`, `i64`             | INTEGER             |
| `u8`, `u16`, `u32`                    | INTEGER             |
| `f32`, `f64`                          | REAL                |
| `&str`, `String`                      | TEXT               |
| `&[u8]`, `Vec<u8>`                    | BLOB                |
| `time::PrimitiveDateTime`             | DATETIME            |
| `time::OffsetDateTime`                | DATETIME            |
| `time::Date`                          | DATE                |
| `time::Time`                          | TIME                |
| `Json<T>`                             | TEXT                |
| `serde_json::JsonValue`               | TEXT                |
| `&serde_json::value::RawValue`        | TEXT                |
| `bstr::BString`                       | BLOB                |


You can also derive custom types.

<table>
<tr>
<td>

```rust
#[derive(musq::Type)]
enum Foo {One, Two}
```

Enum stored as a string: "One", "Two".

</td>

<td>

```rust
#[derive(musq::Type)]
#[musq(rename_all = "lower_case")]
enum Foo {One, Two}
```

Enum stored as a string: "one", "two".

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


### Why?

Musq SQLite-focused fork of sqlx. The aims are to simplify and clean up the codebase, strip out un-needed features, add
new features, improve testing and ergonomics, and support WASM.


# Development


## Profiling

Run the benchmarks with profiling enabled:

```sh
cargo bench --bench benchmark -- --profile-time 10
```

The resulting flamegraphs are in `./targets/criterion/*/profile`. At the moment, the benchmarks are only supported on
Linux.