<b>Hello curious person - musq is not yet ready for use! Please check back later.</b>

<h1 align="center">Musq</h1>

Musq is an async SQLite crate library for Rust.



# Derives

## Types

Types are values that can be stored in a column in database. The following built-in types are supported:

| Rust type                             | SQLite type(s)      |
|---------------------------------------|---------------------|
| `bool`                                | BOOLEAN             |
| `i8`                                  | INTEGER             |
| `i16`                                 | INTEGER             |
| `i32`                                 | INTEGER             |
| `i64`                                 | BIGINT, INT8        |
| `u8`                                  | INTEGER             |
| `u16`                                 | INTEGER             |
| `u32`                                 | INTEGER             |
| `f32`                                 | REAL                |
| `f64`                                 | REAL                |
 | `&str`, [`String`]                    | TEXT                |
| `&[u8]`, `Vec<u8>`                    | BLOB                |
| `time::PrimitiveDateTime`             | DATETIME            |
| `time::OffsetDateTime`                | DATETIME            |
| `time::Date`                          | DATE                |
| `time::Time`                          | TIME                |
| [`Json<T>`]                           | TEXT                |
| `serde_json::JsonValue`               | TEXT                |
| `&serde_json::value::RawValue`        | TEXT                |
| `bstr::BString`                       | BLOB                |


You can also derive custom types.


<table>
<tr>
<td>
</td>
</tr

</table>

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

A struct stored as an **i32**.

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