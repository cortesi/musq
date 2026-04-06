# sqlite-vec Support (Feature Gated)

Add native [sqlite-vec](https://github.com/asg017/sqlite-vec) vector support to musq with an
opt-in `vec` feature flag to avoid build/install issues on unsupported platforms. When enabled,
musq provides `float32`, `int8`, and `bit` vector types, sqlite-vec registration, and full tests.

## Approach

- Feature-gated: sqlite-vec is compiled and linked only when the `vec` feature is enabled
- No generic extension-loader abstraction in this change
- New `types/vec.rs` with `VecF32`, `VecInt8`, and `VecBit`, compiled only with `vec`
- Explicit subtype semantics for `int8` and `bit` in SQL usage/documentation

## Key constraints

- The default build must remain unchanged when `vec` is disabled.
- sqlite-vec treats values without subtype as `float32`; `int8`/`bit` require subtype-aware SQL
  wrappers (for example `vec_int8(?)`, `vec_bit(?)`) when used with sqlite-vec functions/tables.
- `VecF32` decoding must be non-panicking and return `DecodeError` for malformed input.
- Extension registration must be idempotent and must propagate registration failures clearly.

1. Stage One: Cargo Feature and Conditional Wiring

Add a dedicated `vec` Cargo feature and wire sqlite-vec only behind that feature.

1. [ ] In `crates/musq/Cargo.toml`, add:
       optional dependencies `sqlite-vec` and `bytemuck`,
       a `features` section with `vec = ["dep:sqlite-vec", "dep:bytemuck"]`,
       and keep `default` free of `vec`.
2. [ ] In `crates/musq/src/sqlite/ffi.rs`, add a safe wrapper for `sqlite3_auto_extension`.
3. [ ] Add feature-gated `register_vec()` in `crates/musq/src/sqlite/ffi.rs` that:
       performs the `sqlite3_vec_init` function-pointer cast in one place,
       registers once via `OnceLock<Result<(), Error>>` (or equivalent), and
       returns the stored result on every call.
4. [ ] Map non-`SQLITE_OK` registration codes into a structured Musq error
       (`Error::Sqlite` with unknown code fields and explicit message).
5. [ ] In `crates/musq/src/sqlite/connection/establish.rs`, call `register_vec()` behind
       `#[cfg(feature = "vec")]` before `sqlite3_open_v2`.
6. [ ] Ensure `vec`-disabled builds do not reference sqlite-vec symbols or bytemuck.

2. Stage Two: Vector Type Implementations

Implement Rust wrappers and `Encode`/`Decode` behavior in `crates/musq/src/types/vec.rs`.

1. [ ] Add:
       `pub struct VecF32(pub Vec<f32>);`
       `pub struct VecInt8(pub Vec<i8>);`
       `pub struct VecBit(pub Vec<u8>);`
2. [ ] Implement `Encode` for all three types as `Value::Blob`.
3. [ ] Implement `Decode` for all three types with `compatible!(..., SqliteDataType::Blob)`.
4. [ ] `VecF32` decode: reject byte lengths not divisible by 4 and decode with a non-panicking
       path (`chunks_exact(4)` + `f32::from_ne_bytes`, or `try_cast_slice` with proper error map).
5. [ ] `VecInt8` decode: preserve raw bit patterns when converting bytes to signed `i8` values.
6. [ ] `VecBit` decode: preserve raw packed bytes exactly.
7. [ ] Add concise module docs in `types/vec.rs` describing SQL subtype requirements for
       `VecInt8`/`VecBit`.

3. Stage Three: Type Registration and Public API

Wire new types into the crate surface and docs under `vec`.

1. [ ] In `crates/musq/src/types/mod.rs`, add `#[cfg(feature = "vec")] pub mod vec;`.
2. [ ] Update the type table docs in `crates/musq/src/types/mod.rs` to include vector types and
       clearly mark them as requiring the `vec` feature.
3. [ ] Re-export vector types in `crates/musq/src/lib.rs` under `#[cfg(feature = "vec")]`:
       `pub use crate::types::vec::{VecBit, VecF32, VecInt8};`
4. [ ] Add public usage guidance (doc comments) that:
       plain `?` works for `VecF32`,
       `VecInt8` requires `vec_int8(?)`,
       `VecBit` requires `vec_bit(?)`.
5. [ ] Add `doc(cfg(feature = "vec"))` annotations where appropriate for generated docs.

4. Stage Four: Tests

Add unit and integration tests that validate extension loading, correctness, and SQL semantics.

1. [ ] Unit tests in `crates/musq/src/types/vec.rs`:
       round-trip encode/decode for all types,
       empty `VecF32` round-trip,
       malformed `VecF32` byte length returns `DecodeError`,
       full-range `VecInt8` values round-trip,
       `VecBit` byte payload is preserved exactly.
2. [ ] Add `crates/musq/tests/vec.rs` gated by `#![cfg(feature = "vec")]`:
       verify extension availability with `SELECT vec_version()`,
       verify both direct connection (`Connection::connect_with`) and pooled
       connection (`Musq::new().open_in_memory()`) can use sqlite-vec,
       create a `vec0` table and run a float32 KNN query (`MATCH` + `ORDER BY distance`),
       verify scalar functions `vec_distance_l2` and `vec_distance_cosine`,
       verify `FromRow` with a struct containing `VecF32`.
3. [ ] Add integration coverage for subtype-sensitive behavior:
       `vec_type(vec_int8(?))` reports int8 for `VecInt8`,
       `vec_type(vec_bit(?))` reports bit for `VecBit`,
       plain `vec_type(?)` with `VecInt8` does not report int8
       (documents wrapper requirement and prevents accidental regressions).
4. [ ] Keep explicit query patterns (not `test_type!`) since vector equality is not suited to
       SQL `is` comparisons.
5. [ ] Confirm the existing non-`vec` test suite still runs without adding vec-only dependencies
       into the default path.

5. Stage Five: Validation and Finish

Run required checks for both configurations.

1. [ ] Run `cargo fmt`.
2. [ ] Run `cargo clippy --fix --allow-dirty --tests --examples --benches`.
3. [ ] Run `cargo test`.
4. [ ] Run `cargo clippy --fix --allow-dirty --tests --examples --benches --features vec`.
5. [ ] Run `cargo test --features vec`.
6. [ ] Fix all warnings/errors surfaced by these commands.

## File Changes Summary

| File | Change |
|------|--------|
| `crates/musq/Cargo.toml` | Add optional `sqlite-vec`/`bytemuck` deps and `vec` feature |
| `crates/musq/src/sqlite/ffi.rs` | Add auto-extension wrapper + feature-gated `register_vec` |
| `crates/musq/src/sqlite/connection/establish.rs` | Call `register_vec` under `cfg(feature = "vec")` |
| `crates/musq/src/types/vec.rs` | New feature-gated vector types, impls, docs, and unit tests |
| `crates/musq/src/types/mod.rs` | Feature-gated `vec` module and updated docs |
| `crates/musq/src/lib.rs` | Feature-gated re-export of vector types |
| `crates/musq/tests/vec.rs` | Feature-gated integration tests for sqlite-vec + vector behavior |
