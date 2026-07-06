# Modern SQLite Capabilities Roadmap

Musq now depends on `libsqlite3-sys 0.38.1`, whose bundled SQLite is 3.53.2.
Upstream SQLite is currently 3.53.3. Musq is also pinned to the latest stable
`sqlite-vec` release, 0.1.9; the newer 0.1.10-alpha.4 line is prerelease and
did not build locally because its crate package references a missing DiskANN C
source file.

Sources checked:

- SQLite 3.53.0 release log: https://sqlite.org/releaselog/3_53_0.html
- SQLite 3.53.3 release log: https://sqlite.org/releaselog/3_53_3.html
- SQLite ALTER TABLE docs: https://sqlite.org/lang_altertable.html
- SQLite JSON docs: https://sqlite.org/json1.html
- SQLite REINDEX docs: https://sqlite.org/lang_reindex.html
- SQLite VACUUM docs: https://sqlite.org/lang_vacuum.html
- SQLite runtime version APIs: https://sqlite.org/c3ref/libversion.html
- SQLite compile-option APIs: https://sqlite.org/c3ref/compileoption_get.html
- SQLite carray table-valued function: https://sqlite.org/carray.html
- SQLite carray bind APIs: https://sqlite.org/c3ref/carray_bind.html
- SQLite carray compile option: https://sqlite.org/compile.html#enable_carray
- SQLite database status APIs: https://sqlite.org/c3ref/db_status.html
- SQLite WAL checkpoint API: https://sqlite.org/c3ref/wal_checkpoint_v2.html
- SQLite database configuration options:
  https://sqlite.org/c3ref/c_dbconfig_defensive.html
- SQLite runtime limits: https://sqlite.org/c3ref/limit.html
- sqlite-vec v0.1.9 release: https://github.com/asg017/sqlite-vec/releases/tag/v0.1.9
- sqlite-vec v0.1.10-alpha.4 release:
  https://github.com/asg017/sqlite-vec/releases/tag/v0.1.10-alpha.4
- Local crate sources under `~/.cargo/registry/src/.../libsqlite3-sys-0.38.1`
- Local crate sources under `~/.cargo/registry/src/.../sqlite-vec-0.1.9`
- Local crate sources under `~/.cargo/registry/src/.../sqlite-vec-0.1.10-alpha.4`

1. Stage One: Version And Capability Policy

Define what SQLite capability level Musq promises, and keep the dependency
upgrade path explicit.

Stage status: locked in for the current bundled runtime. Musq now supports only
the SQLite release bundled by `libsqlite3-sys`; older/system SQLite libraries
are outside the support policy.

1. [x] Add a small capability probe test around `SELECT sqlite_version()` and
   `PRAGMA compile_options` so the test suite documents the active SQLite runtime.
2. [x] Track the next `libsqlite3-sys` release that bundles SQLite 3.53.3 or newer.
   The current dependency is up to date; after the next crate release lands,
   rerun the dependency refresh and the full test suite.
3. [x] Keep `sqlite-vec = "0.1.9"` until a stable successor exists or the alpha
   packaging issue is fixed. Use `cargo outdated --root-deps-only --workspace
   --aggressive` as the reminder.
4. [x] Decide whether Musq supports older system SQLite libraries when
   `LIBSQLITE3_SYS_USE_PKG_CONFIG` is set. The supported runtime is bundled
   SQLite only.
5. [x] Document the minimum tested SQLite version in the README and crate docs.

2. Stage Two: SQL Feature Proofs

Cover SQLite 3.53 SQL features that already work through Musq's existing query
surface before adding new public API.

Decision: modern SQL-language support stays on Musq's existing query surface
until a broader migration, JSON path, trigger, or backup API gives it a more
specific home. Stage Two records the high-value bundled SQLite 3.53 SQL features
as regression tests.

1. [x] Add `crates/musq/tests/sqlite_modern_sql.rs` with `ALTER TABLE` coverage
   for `ALTER COLUMN ... SET NOT NULL`, duplicate `SET NOT NULL`, `DROP NOT NULL`,
   `ADD CONSTRAINT ... CHECK`, and `DROP CONSTRAINT`. Include a failing insert
   that proves the added `CHECK` constraint is enforced.
2. [x] Add a `REINDEX EXPRESSIONS` smoke test over an expression index in the same
   integration file.
3. [x] Add `json_array_insert()` and `jsonb_array_insert()` tests that verify text
   JSON output and JSONB round-tripping through `json(...)`. Keep these as SQL
   function coverage through `query_scalar`.

3. Stage Three: Connection Diagnostics And Control

Add a small typed control surface for SQLite runtime inspection, per-connection
status counters, WAL checkpointing, and connection-open limits.

Stage recommendation: implement this as connection-first API. `Connection` and
`PoolConnection` represent one live SQLite handle and get the full API. `Pool`
gets convenience methods only where acquiring a representative connection has
database-level semantics.

1. [x] Add `SqliteRuntimeInfo { version, version_number, source_id,
   compile_options }`. Expose `Connection::runtime_info()` and
   `Pool::runtime_info()`. Collect the text fields through Musq's existing SQL
   path with `sqlite_version()`, `sqlite_source_id()`, and
   `PRAGMA compile_options`; use `sqlite3_libversion_number()` for the numeric
   version.
2. [x] Add private worker commands for handle-scoped control operations. Keep
   every API that receives a live `sqlite3*` on the existing connection worker
   thread, beside the current `Prepare`, `Execute`, transaction, and shutdown
   commands.
3. [x] Add `Connection::db_status(kind, reset_highwater) -> Result<DbStatus>`,
   where `DbStatus { current: i64, highwater: i64 }` and `DbStatusKind` covers
   the bundled `sqlite3_db_status64()` counters: `LookasideUsed`, `CacheUsed`,
   `SchemaUsed`, `StatementUsed`, `LookasideHit`, `LookasideMissSize`,
   `LookasideMissFull`, `CacheHit`, `CacheMiss`, `CacheWrite`,
   `DeferredForeignKeys`, `CacheUsedShared`, `CacheSpill`, and
   `TempBufferSpill`. Leave pool-wide aggregation to callers by acquiring the
   connections they want to inspect.
4. [x] Add `Connection::wal_checkpoint(schema, mode) -> Result<WalCheckpoint>`,
   backed by `sqlite3_wal_checkpoint_v2()`. Support `WalCheckpointMode` values
   `Passive`, `Full`, `Restart`, `Truncate`, and `Noop`, and return
   `WalCheckpoint { log_frames: Option<i32>, checkpointed_frames: Option<i32> }`
   because SQLite may report `-1` when the database is not in WAL mode. Add
   `Pool::wal_checkpoint(schema, mode)` by acquiring one connection; the
   checkpoint acts on the database, not on a private connection statistic.
5. [x] Add `Musq::floating_point_text_digits(u8)` backed by
   `SQLITE_DBCONFIG_FP_DIGITS` during connection establishment. Accept values
   `4..=23`; the default remains SQLite's bundled default of 17 digits. Use this
   for applications that need reproducible text rendering of floating-point
   values across SQLite upgrades.
6. [x] Add `Musq::parser_depth_limit(u32)` backed by
   `sqlite3_limit(SQLITE_LIMIT_PARSER_DEPTH, limit)` during connection
   establishment. Accept positive limits and expose
   `Connection::parser_depth_limit() -> Result<u32>` for diagnostics. Use this
   as an opt-in defense for applications that execute externally shaped SQL.
7. [x] Cover the stage with integration tests: runtime-info contents, at least
   one `db_status` counter before and after a simple query, WAL `Noop` status on
   a file-backed WAL database, range validation for `floating_point_text_digits`,
   and a parser-depth limit failure on a deeply nested expression.

4. Stage Four: Array Binding With carray

Stage decision: defer implementation until `SQLITE_ENABLE_CARRAY` is available
through a future `libsqlite3-sys/array` Cargo feature. The interface is worth
having for large, length-varying `IN` predicates because it keeps SQL text
stable, preserves prepared-statement cache reuse, and avoids host-parameter
explosion. The current `libsqlite3-sys 0.38.1` crate exposes the carray FFI
declarations and bundled SQLite source, but its manifest has no `array` feature
that compiles the amalgamation with `SQLITE_ENABLE_CARRAY`. Relying on ambient
`LIBSQLITE3_FLAGS=SQLITE_ENABLE_CARRAY` is not a shippable public API contract
for Musq.

Implementation trigger: a future `libsqlite3-sys` release publishes an `array`
feature that compiles SQLite with `SQLITE_ENABLE_CARRAY`. When that trigger
exists, ship the API below behind a Musq `carray` feature and include that
feature in Musq's defaults.

1. [ ] Add `carray = ["libsqlite3-sys/array"]` to `crates/musq/Cargo.toml` and
   include it in `default`. Extend the SQLite capability tests so
   `ENABLE_CARRAY` is required whenever the feature is active.
2. [ ] Add `crates/musq/src/types/carray.rs` and export the public wrappers as
   `musq::{CArray, CArrayText, CArrayBlob}`. `CArray<T>` supports exactly
   `i32`, `i64`, and `f64` through a sealed `CArrayElement` trait. It stores
   `Arc<[T]>` and exposes `new`, `as_slice`, `len`, `is_empty`, `From<Vec<T>>`,
   `From<Box<[T]>>`, and `From<Arc<[T]>>`.
3. [ ] Add `CArrayText` for text arrays and `CArrayBlob` for blob arrays.
   `CArrayText::new` accepts string-like iterators, validates interior NUL bytes
   once, and stores `Arc<[CString]>`. `CArrayBlob::new` accepts byte-like
   iterators and stores `Arc<[Bytes]>`. Both wrappers expose `len` and
   `is_empty`; empty arrays are valid and produce an empty carray table.
4. [ ] Add an internal `Value::CArray(CArrayValue)` variant plus `Encode`
   implementations for `CArray<i32>`, `CArray<i64>`, `CArray<f64>`,
   `CArrayText`, and `CArrayBlob`. Numeric arrays bind with
   `sqlite3_carray_bind_v2()` and `SQLITE_STATIC` against the `Arc` slice.
   Text and blob arrays bind with `SQLITE_TRANSIENT` from temporary pointer and
   `repr(C)` iovec arrays so SQLite owns the copied scan data immediately.
5. [ ] Add `StatementHandle::bind_carray` and the corresponding FFI wrapper in
   `crates/musq/src/sqlite/ffi.rs`. Convert SQLite carray bind failures through
   the existing `SqliteError` path. Keep carray values inside `Arguments` so the
   existing `ExecuteIter` ownership, statement reset, and `clear_bindings()`
   lifecycle owns every array for the whole statement execution.
6. [ ] Add `expr::carray(&array) -> Expr` and
   `QueryBuilder::push_carray(&array) -> Result<()>`. Both helpers emit the
   one-argument SQLite form `carray(?)`, which is SQLite's recommended form.
   Direct use also works with `query("... IN carray(?)").bind(&array)`.
7. [ ] Document `push_values` as the simple path for short one-off lists and
   `carray` as the path for large, length-varying, or frequently reused list
   predicates. The value proposition is stable SQL text and parameter-limit
   avoidance; no runtime-speedup promise is attached to the API.
8. [ ] Cover the feature with integration tests for numeric, text, and blob
   arrays; empty arrays; `WHERE id IN carray(?)`; statement reuse with different
   array lengths; text NUL validation; and `ENABLE_CARRAY` in
   `runtime_info().compile_options`.

5. Stage Five: sqlite-vec Coverage

Strengthen support for the stable `sqlite-vec` feature set before adopting the
ANN prerelease line.

1. [ ] Add tests for `vec0` tables with metadata columns, auxiliary columns, and
   partition key columns using plain SQL.
2. [ ] Add a regression test for deleting from a `vec0` table with long metadata
   text. This is the bug fixed by sqlite-vec 0.1.9.
3. [ ] Add a filtered KNN example combining `WHERE embedding MATCH ?` with
   metadata or partition predicates.
4. [ ] Decide whether vector wrappers should remain simple owned vectors or gain
   optional dimension-aware constructors for earlier client-side validation.
5. [ ] Track the sqlite-vec 0.1.10 alpha line. Adopt IVF, DiskANN, or rescore APIs
   after the crate builds from crates.io and the release is stable.

6. Stage Six: Future Review Triggers

Keep lower-priority SQLite 3.53 capabilities tied to the project feature that
would make each one relevant.

1. [ ] Revisit QRF and CLI result formatting if Musq grows its own interactive
   shell or reporting layer.
2. [ ] Revisit `SQLITE_PREPARE_FROM_DDL` if Musq starts implementing SQLite
   virtual tables.
3. [ ] Revisit the new session extension APIs when Musq has a sync/changeset
   feature story and can justify the `session` and `buildtime_bindgen` path in
   `libsqlite3-sys`.
4. [ ] Revisit WASM `opfs-wl` when Musq has a browser or WASI target plan.
5. [ ] Keep expression-index health covered by SQL-level tests until Musq grows a
   database-maintenance API.

7. Stage Seven: Validation Plan

Use the repo's normal Rust gates after any implementation stage.

Stage One, Stage Two, and Stage Three validation status: complete.

1. [x] Run `cargo clippy --fix --allow-dirty --tests --examples --benches`.
2. [x] Run `cargo fmt`.
3. [x] Run `cargo test`.
4. [x] Run `cargo outdated --root-deps-only --workspace --aggressive --exit-code 0`
   and explain any intentional prerelease holdbacks.
5. [x] Run `git diff --check` and inspect the final status before committing.
