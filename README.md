<h1 align="center">muSQLite</h1>

muSQLite is an async SQLite crate library for Rust.

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