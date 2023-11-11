<h1 align="center">muSQLite</h1>

muSQLite is an async SQLite crate library for Rust.



# Derives

## Type

<table>
<tr>
<td>

```rust
#[derive(musqlite::Type)]
enum Foo {One, Two}
```

Maps to underlying string: "One", "Two".

</td>

<td>

```rust
#[derive(musqlite::Type)]
#[musqlite(rename_all = "lower_case")]
enum Foo {One, Two}
```

Maps to underlying string: "one", "two".

</td>

<td>

```rust
#[derive(musqlite::Type)]
#[musqlite(repr = "i32")]
enum Foo {One, Two}
```

Maps to underlying *i32*: 0, 1.

</td>
</tr>
</table>


### Why?

muSQLite started as a fork of sqlx, focused just on SQLite. The aims are to simplify and clean up the codebase, strip
out un-needed features, add new features, improve testing and ergonomics, and support WASM.


# Development


## Profiling

Run the benchmarks with profiling enabled:

```sh
cargo bench --bench benchmark -- --profile-time 10
```

The resulting flamegraphs are in `./targets/criterion/*/profile`. At the moment, the benchmarks are only supported on
Linux.