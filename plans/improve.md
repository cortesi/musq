# Musq Improvement Report

Date: 2026-08-29. Base revision: `main` at `45de554`.

This report records recommended improvements to Musq. It covers structure,
elegance, correctness, the public API, and recent SQLite releases. Each
recommendation is one section with evidence, the proposed change, and an
effort estimate. Sections are grouped by theme and ordered by value inside each
theme. The "Implementation Checklist" at the end is the live execution
record.

Compatibility policy: there are no backward-compatibility constraints. Every
change in this report is a clean break. No section keeps a deprecated alias,
a transition default, or a compatibility shim. Public signatures, defaults,
and error variants change in place, and the README changes with them.

Baseline gates at the base revision:

- `cargo test`: all targets pass (343 tests across unit, integration, macro,
  and trybuild suites).
- `cargo clippy --tests --examples --benches`: no warnings.
- `cargo doc --no-deps`: no warnings.
- `cargo outdated --root-deps-only --workspace`: only patch-level updates,
  except `libsqlite3-sys 0.38.2` (still bundles SQLite 3.53.2), `darling
  0.24`, and `syn 3.0`.

Observed behavior at the base revision:

- The bundled SQLite build includes the `memdb` VFS. A pool opened with
  `.vfs("memdb").open("/name")` shares one in-memory database across all pool
  connections without shared-cache mode.
- A deferred transaction that reads and then writes after another writer
  committed fails with `SQLITE_BUSY_SNAPSHOT` in about 30 microseconds. The
  configured `busy_timeout` does not apply to this case.
- A syntax error renders as
  `error returned from database (primary: Error, extended: Unknown(1)): near
  "FORM": syntax error`. No byte offset is reported.

Environment facts as of 2026-08-29:

- SQLite upstream: 3.53.4 (2026-07-24) is the latest release. 3.54.0 is not
  released. 3.51 added `carray` and `percentile` to the amalgamation behind
  compile flags, `sqlite3_db_status64`, `jsonb_each()`, and
  `PRAGMA wal_checkpoint=NOOP`. 3.53 added `SQLITE_DBCONFIG_FP_DIGITS`,
  `SQLITE_LIMIT_PARSER_DEPTH`, `ALTER TABLE ... SET/DROP NOT NULL`,
  `ADD/DROP CONSTRAINT CHECK`, `REINDEX EXPRESSIONS`, `json_array_insert()`,
  and 17-digit default float text rendering.
- `libsqlite3-sys 0.38.2` (2026-08-08) bundles SQLite 3.53.2. Its feature
  list is unchanged from 0.38.1. There is still no `array`/`carray` feature.
  The bundled build does not define `SQLITE_ENABLE_SETLK_TIMEOUT`.
- `sqlite-vec` stable is still 0.1.9. The 0.1.10 line is alpha only.

Summary of recommendations, ordered by priority:

| # | Section | Priority | Effort |
|---|---------|----------|--------|
| 1 | Real `BEGIN` semantics with `Immediate` as the default | High | Small |
| 2 | Replace shared-cache pools with `memdb` and delete unlock-notify | High | Medium |
| 3 | Add a `Configuration` error variant and stop using `Protocol` for user errors | High | Small |
| 4 | One `SqliteError` type with error offset and a plain `SQLITE_ERROR` code | High | Small |
| 5 | Query cancellation with `sqlite3_interrupt` and a rebuilt worker shared state | High | Large |
| 6 | Make raw-pointer FFI wrappers `unsafe fn` | High | Medium |
| 7 | Fix post-finalize pointer use and never panic in `StatementHandle::drop` | High | Small |
| 8 | Use `sqlite3_changes64` | Medium | Small |
| 9 | Remove redundant `unsafe impl Send/Sync` and the empty `Drop` for `Connection` | Medium | Small |
| 10 | Make `Connection::close` consume the connection | Medium | Small |
| 11 | Disable double-quoted strings and untrusted schema by default | Medium | Small |
| 12 | Expose SQLite transaction state and autocommit | Low | Small |
| 13 | Use `SQLITE_OPEN_EXRESCODE` at open time | Low | Small |
| 14 | Serialize, deserialize, and backup-to-path snapshots | Medium | Small / Medium / Medium |
| 15 | User-defined scalar functions and collations | Medium | Large |
| 16 | Commit, rollback, and update hooks | Low | Medium |
| 17 | Use `AsyncFnOnce` for `Connection::transaction` | Medium | Small |
| 18 | Collapse `QueryExecutor` impls and drop `async-trait` | Medium | Small |
| 19 | Remove `Prepared` and `Statement`; keep `prepare` as validation | Medium | Small |
| 20 | Render an empty `{values:}` list as `IN ()` instead of an error | Medium | Small |
| 21 | Remove `Musq::filename`, align `analysis_limit` and `optimize_on_close` | Low | Small |
| 22 | Remove enum-mode `Default` impls | Low | Small |
| 23 | Replace the `Null` type alias with a unit struct | Low | Small |
| 24 | Remove `expr::jsonb_text` and the stale `hb` reference | Low | Small |
| 25 | Simplify `Arguments::bind` named-parameter branches | Low | Small |
| 26 | Share one SQL scanner between the numeric and rename passes | Low | Small |
| 27 | Reduce the five `emit_query_event_*` functions to one macro | Low | Small |
| 28 | Store column names once and validate TEXT once behind a newtype | Low | Small |
| 29 | Drop `Error::TypeNotFound` and `Column`, fold declared type into names | Low | Small |
| 30 | Remove dead code behind `#[allow(dead_code)]` | Low | Small |
| 31 | Trim dependencies and feature flags | Medium | Medium |
| 32 | Remove the unused `musq-test` crate and the root `examples/` indirection | Medium | Small |
| 33 | Move the bundled SQLite version to one source of truth | Low | Small |
| 34 | Remove stale SQLx-era documentation | Low | Small |
| 35 | Keyword handling in `{upsert:...}` exclude lists | Low | Small |

## A. Correctness And Runtime Behavior

## 1. Real `BEGIN` Semantics With `Immediate` As The Default

Problem. Every Musq transaction starts as `SAVEPOINT _musq_savepoint_0`
(`crates/musq/src/transaction.rs:133`). A top-level savepoint is a deferred
transaction. A deferred transaction that reads and then writes fails with
`SQLITE_BUSY_SNAPSHOT` as soon as another connection has committed in
between. SQLite does not call the busy handler for this upgrade, so
`Musq::busy_timeout` does not help. Measured: about 30 microseconds under WAL
mode with a 300 ms busy timeout. This is the most
common SQLite concurrency trap for applications with a connection pool.

Two further defects sit in the same code:

- `commit_ansi_transaction_sql` always emits `RELEASE SAVEPOINT`
  (`transaction.rs:136-139`), and the worker runs it for every positive
  depth (`worker.rs:240-248`). After a real `BEGIN`, `RELEASE` of an inner
  savepoint does not commit. The commit and rollback SQL must change with
  the begin SQL.
- `Transaction::commit` and `Transaction::rollback` take `&mut self`
  (`transaction.rs:57-64`). The value still dereferences to `Connection`
  afterward, and a second `commit` reaches the worker as a depth-zero no-op.

Change.

- Add `TransactionBehavior { Deferred, Immediate, Exclusive }`.
- Make `Immediate` the default for `begin()`. Most explicit transactions in
  application code write, and `Immediate` removes the snapshot trap for them.
  Add `Musq::default_transaction_behavior(TransactionBehavior)` for
  applications that want `Deferred` read-only transactions, and
  `Pool::begin_with(behavior)` and `Connection::begin_with(behavior)` for a
  per-call choice.
- Emit this SQL by depth:

  | Depth | Begin | Commit | Rollback |
  |-------|-------|--------|----------|
  | 0 → 1 | `BEGIN {DEFERRED\|IMMEDIATE\|EXCLUSIVE}` | `COMMIT` | `ROLLBACK` |
  | n → n+1, n ≥ 1 | `SAVEPOINT _musq_savepoint_n` | `RELEASE SAVEPOINT _musq_savepoint_n` | `ROLLBACK TO SAVEPOINT _musq_savepoint_n; RELEASE SAVEPOINT _musq_savepoint_n` |

  Extend `Command::Begin` with the behavior so the worker emits the right
  statement.
- Change `commit` and `rollback` to consume `self`.
- Tests: assert `sqlite3_get_autocommit` is true after a top-level commit
  and after a top-level rollback under each behavior; assert nested savepoint
  commit and rollback leave the outer transaction open; assert a second
  connection's write inside an `Immediate` transaction fails with
  `PrimaryErrCode::Busy` after the busy timeout, not `BusySnapshot`, and that
  a `Deferred` read-then-write reproduces `BusySnapshot`.

Effort: small. Priority: high.

## 2. Replace Shared-Cache Pools With `memdb` And Delete Unlock-Notify

Problem. `Musq::open_in_memory` uses `SQLITE_OPEN_MEMORY` plus
`SQLITE_OPEN_SHAREDCACHE` on a `file:musq-in-memory-N` name
(`crates/musq/src/musq.rs:593-597`). Shared-cache mode is documented by
SQLite as an obsolete feature that is discouraged. It introduces
`SQLITE_LOCKED_SHAREDCACHE` table-level locks, which is the only reason Musq
carries the `unlock_notify` machinery (`crates/musq/src/sqlite/statement/
unlock_notify.rs`, the `unlock_notify` feature of `libsqlite3-sys`, and the
retry loops in `StatementHandle::step` and `ConnectionHandle::exec`).

The same retry loops also treat file-lock `SQLITE_BUSY` as an unlock-notify
case (`crates/musq/src/sqlite/statement/handle.rs:245-270`).
`sqlite3_unlock_notify` knows only shared-cache blockers, so for a file lock
SQLite invokes the callback at once. The loop then resets and re-steps up to
`DEFAULT_MAX_RETRIES` times with no delay, after SQLite's own busy handler
has already waited the full `busy_timeout`. The result is five wasted steps
and a misleading `Error::UnlockNotify` in place of the real `SQLITE_BUSY`.
The `SQLITE_LOCKED` and `SQLITE_BUSY` arms are byte-for-byte duplicates.

The bundled SQLite build includes the `memdb` VFS. A pool opened with
`.vfs("memdb").open("/name")` shares one database across all of its
connections, and `PRAGMA journal_mode` reports `memory`. Multiple connections to one `memdb`
database use normal file-style locking, so `busy_timeout` applies.

Change, as one commit.

- Make `configure_in_memory` set the filename to `/musq-in-memory-N`, call
  `.vfs("memdb")`, and not set `in_memory`. `EstablishParams::from_options`
  already builds the `file:` URI and sets `SQLITE_OPEN_URI` when a `vfs` is
  present (`establish.rs:89-102`). Do not pre-bake a `file:...?vfs=memdb`
  string, because a pre-baked string never receives `SQLITE_OPEN_URI` and a
  second option would produce `file:file:...?...?...`.
- Remove `Musq::shared_cache`, the `shared_cache` field, the `in_memory`
  field, and `SQLITE_OPEN_MEMORY` handling.
- Remove the `unlock_notify` module, the `Error::UnlockNotify` variant,
  `DEFAULT_MAX_RETRIES`, both retry arms in `StatementHandle::step`, the
  retry loop in `ConnectionHandle::exec`, and the `unlock_notify` cargo
  feature of `libsqlite3-sys`. Let `ffi::step` return any code other than
  `SQLITE_ROW` and `SQLITE_DONE` as an error.
- Update `crates/musq/tests/sqlite_capabilities.rs:26-37` so it no longer
  requires `ENABLE_UNLOCK_NOTIFY`.
- Keep `tests/in_memory_settings.rs`, `tests/concurrent.rs`, and
  `tests/connection_flows.rs::retry_on_busy_lock` as the proof that
  in-memory pools still share state and that SQLite's busy handler still
  does the waiting.

Note: `memdb` uses the `memory` journal, so lock tests on in-memory pools do
not prove WAL file behavior. The §1 tests use a file database.

Effort: medium. Priority: high.

## 3. Add A `Configuration` Error Variant

Problem. `Error::Protocol` is documented as "a programming error in Musq or
something corrupted with the connection" (`crates/musq/src/error.rs:74-79`).
The same variant is used for ordinary user mistakes:

- `floating_point_text_digits` and `parser_depth_limit` range checks
  (`crates/musq/src/sqlite/connection/establish.rs:177-220`).
- `max_connections(0)` (`crates/musq/src/pool/mod.rs:68`).
- empty `push_values` and `push_idents` lists
  (`crates/musq/src/query_builder.rs:82-102`).
- numeric placeholders in composed fragments, missing bind values, and
  `WAL checkpoint schema contains nul bytes`.

Callers cannot tell a configuration error from a driver bug, and the display
text ("encountered unexpected or invalid data") is wrong for all of these.

Change. Add `Error::Configuration(String)` for builder and option validation
and `Error::Query(String)` for query-composition and bind-count errors.
Reserve `Protocol` for real invariant violations. Move every existing
`Protocol` call site that reports a user mistake to the new variants in the
same commit.

Effort: small. Priority: high.

## 4. One `SqliteError` Type With Error Offset And A Plain Code

Problem.

- `Error::Sqlite { primary, extended, message }`
  (`crates/musq/src/error.rs:61`) duplicates the fields of `SqliteError`
  (`sqlite/error.rs:378`). `Error::into_sqlite_error` and
  `From<SqliteError>` copy fields between them, and `is_busy` and
  `is_unique_violation` exist on both.
- `SqliteError::new` reads only the extended code and message
  (`sqlite/error.rs:380-404`). A plain `SQLITE_ERROR` (code 1, no extended
  part) renders as `extended: Unknown(1)`, which is the most common error a
  user sees.
- `sqlite3_error_offset` (SQLite 3.38) is available in the bundled bindings
  but unused. Syntax errors from `sql!` cannot point at the failing token.

Change, as one commit.

- Make the variant `Error::Sqlite(#[from] SqliteError)` and delete the copy
  helpers. `Error::sqlite_codes` becomes `self.as_sqlite().map(..)`.
- Add `offset: Option<usize>` to `SqliteError`, filled from
  `sqlite3_error_offset` when it is not `-1`.
- Make the extended code `Option<ExtendedErrCode>`. `None` means SQLite
  reported only the primary code. Update `Display` so a bare syntax error
  reads `SQLITE_ERROR at byte 9: near "FORM": syntax error`.
- Add a test that asserts the offset for a known syntax error.

Effort: small. Priority: high.

## 5. Query Cancellation And A Rebuilt Worker Shared State

Problem, part one. A long-running statement cannot be stopped. Dropping a
`fetch` stream stops delivery only after the worker produces its next row. A
`CREATE INDEX`, `VACUUM`, or a slow join keeps the worker thread and the
database busy. `sqlite3_interrupt` is thread-safe and is the standard answer.

Problem, part two. `WorkerSharedState::conn` is a
`tokio::sync::Mutex<ConnectionState>`
(`crates/musq/src/sqlite/connection/worker.rs:56`). The worker thread calls
`try_lock` once at startup and holds the guard for its whole life
(`worker.rs:157`). The only other reader is `Transaction::fmt`, which calls
`try_lock` and therefore always prints `transaction_depth: "<locked>"`
(`crates/musq/src/transaction.rs:84`). The mutex adds no synchronization and
the debug output is always wrong.

Two constraints shape the design:

- `sqlite3_interrupt` on a closed handle is undefined behavior. A plain
  `AtomicPtr` that the worker clears at shutdown still races: a caller can
  load the pointer before the clear and call `interrupt` after the close.
- SQLite rolls back an explicit transaction when an interrupted write fails
  inside it. The worker changes `transaction_depth` only for explicit
  transaction commands (`worker.rs:210-295`), so an interrupted write leaves
  the tracked depth wrong.

Change, as one commit.

- Give `ConnectionState` to the worker thread by value. Replace the tokio
  mutex with a `WorkerSharedState { cached_statements_size: AtomicUsize,
  transaction_depth: AtomicUsize, db: std::sync::Mutex<Option<NonNull<sqlite3>>>
  }`. The worker publishes the pointer after open and takes the exclusive
  lock to set `None` before `sqlite3_close`. `interrupt` takes the lock, and
  calls `sqlite3_interrupt` only while it holds `Some`. The worker never
  holds the lock while it steps a statement, so `interrupt` cannot deadlock.
  Clear the pointer on every worker exit path, including panics, with a drop
  guard.
- Add `Connection::interrupt(&self)` and `PoolConnection::interrupt`.
  Document that any statement in flight fails with `SQLITE_INTERRUPT`, that
  an interrupted write inside a transaction rolls that transaction back, and
  that the next statement runs normally.
- After any `SQLITE_INTERRUPT`, the worker reads `sqlite3_get_autocommit`.
  If SQLite is in autocommit, the worker resets `transaction_depth` to zero
  and marks the connection so the next `Commit` or `Rollback` command returns
  `Error::TransactionAborted` instead of running SQL at depth zero.
- Add `Musq::statement_timeout(Duration)` implemented with
  `sqlite3_progress_handler` on the worker thread. The handler returns
  non-zero when the deadline passes, which SQLite treats like an interrupt.
  Reset the deadline per statement.
- Update `Transaction::fmt` to read the atomic depth.
- Tests: interrupt a recursive CTE and assert `PrimaryErrCode::Interrupt`;
  interrupt a write inside an explicit transaction and assert the follow-up
  `commit` returns `TransactionAborted`; race `interrupt` against `close` in
  a loop and assert no crash; hit `statement_timeout` inside a transaction.

Effort: large. Priority: high.

## 6. Make Raw-Pointer FFI Wrappers `unsafe fn`

Problem. Every function in `crates/musq/src/sqlite/ffi.rs` is a safe `pub fn`
that takes a raw `*mut sqlite3` or `*mut sqlite3_stmt`, with a `# Safety`
section that lists caller obligations (for example `ffi::step` at line 688).
A safe function with unchecked pointer preconditions is unsound by
definition. The `# Safety` docs describe obligations the type system does not
enforce. The unsafety is not removed, only relocated to the 28 call sites
outside `ffi.rs` that already wrap calls in `unsafe` blocks or comments.

Change.

- Mark each raw-pointer wrapper `unsafe fn` and keep the `# Safety` section.
- Make `ConnectionHandle` and `StatementHandle` the safe boundary. Their
  methods own a valid `NonNull` pointer, so they can call the wrappers inside
  one `unsafe` block each with a one-line justification.
- Keep `libversion_number`, `register_vec`, and `auto_extension` safe because
  they take no pointers.
- Remove the `pub` visibility on `ffi` items that only `sqlite` uses.
- Do this after §7 so the wrappers are correct before they are re-marked.

Effort: medium. Priority: high.

## 7. Fix Post-Finalize Pointer Use And Never Panic In `Drop`

Problem. Two defects in statement finalization:

- `ffi::finalize` calls `sqlite3_db_handle(stmt)` after `sqlite3_finalize`
  returned an error (`crates/musq/src/sqlite/ffi.rs:713-720`). SQLite frees
  the statement in `finalize`, so this reads a freed pointer.
- `StatementHandle::drop` does the same through `self.db_handle()` in its
  error log (`crates/musq/src/sqlite/statement/handle.rs:292-296`), and it
  panics on `SQLITE_MISUSE` (`handle.rs:292`). A panic inside `Drop` during
  unwinding aborts the process. Statement handles are dropped inside
  `StatementCache::clear`, which runs on worker shutdown and on
  `ConnectionState::drop`, so an already-failing path can escalate to an
  abort.

Change. Capture the database pointer with `sqlite3_db_handle` before
`sqlite3_finalize`. Build the `SqliteError` from that pointer. Never touch
the statement after finalization. In `Drop`, log at `error` level and return.
No `panic!` and no `debug_assert!` in `Drop`.

Effort: small. Priority: high.

## 8. Use `sqlite3_changes64`

Problem. `StatementHandle::changes` calls `sqlite3_changes` and casts the
`i32` to `u64` (`crates/musq/src/sqlite/statement/handle.rs:58`). A statement
that changes more than 2^31 rows reports a negative value cast to a huge
unsigned number. `sqlite3_changes64` exists since 3.37 and is in the bundled
bindings.

Change. Add `ffi::changes64` and use it.

Effort: small. Priority: medium.

## 9. Remove Redundant `unsafe impl` Blocks And The Empty `Drop`

Problem.

- `unsafe impl Sync for Connection` (`crates/musq/src/sqlite/connection/
  mod.rs:79`) and `unsafe impl Sync for ConnectionWorker` (`worker.rs:49`)
  each carry a comment that says every field is already `Sync`. If that is
  true the impls are redundant. If it becomes false, the impls silently hide a
  real `!Sync` field.
- `unsafe impl Send for Row` and `unsafe impl Sync for Row`
  (`crates/musq/src/row.rs:31-32`) are guarded by a comment about statement
  handles. `Row` holds `Box<[Value]>`, `Arc<Vec<Column>>`, and
  `Arc<HashMap<..>>`, which are all `Send + Sync`. The comment is stale.
- `impl Drop for Connection` is an empty body with a comment
  (`mod.rs:442-448`). An empty `Drop` prevents destructuring and adds a
  drop-glue frame for no effect.

Change. Delete all four impls. Add `fn assert_send_sync<T: Send + Sync>()`
checks in the unit test module of each affected file so the compiler enforces
the property instead of an `unsafe` promise.

Effort: small. Priority: medium.

## 10. Make `Connection::close` Consume The Connection

Problem. `Connection::close(&self)` (`mod.rs:138`) shuts the worker down but
leaves the `Connection` usable. Every later call returns
`Error::WorkerCrashed`, which is a wrong description of a normal state.

Change. Change the signature to `close(self)`. `PoolConnection::close` already
consumes `self` and can call the new form. After the change no caller can
observe a normally closed connection, so `WorkerCrashed` keeps its meaning
for unexpected command-channel loss and no new variant is needed.

Effort: small. Priority: medium.

## B. Modern SQLite Features

## 11. Disable Double-Quoted Strings And Untrusted Schema By Default

Problem. The bundled build enables double-quoted string literals (DQS), the
legacy misfeature where `"nonexistent_column"` silently becomes the string
`'nonexistent_column'`. Musq quotes identifiers with double quotes
(`quote_identifier`), so a typo in a `{ident:}` placeholder or a hand-written
query can produce a wrong result instead of an error. SQLite recommends that
new applications disable DQS. Related per-connection switches that Musq does
not expose: `SQLITE_DBCONFIG_DEFENSIVE`, `SQLITE_DBCONFIG_TRUSTED_SCHEMA`,
`SQLITE_DBCONFIG_ENABLE_ATTACH_CREATE`, `SQLITE_DBCONFIG_ENABLE_ATTACH_WRITE`,
and `SQLITE_DBCONFIG_ENABLE_COMMENTS`. All constants are present in the
bundled bindings.

Change.

- Turn `SQLITE_DBCONFIG_DQS_DDL` and `SQLITE_DBCONFIG_DQS_DML` off on every
  connection at establish time. Add `Musq::double_quoted_strings(bool)` for
  the rare legacy schema that needs them back on. Add a test that asserts
  `SELECT "no_such_column"` fails.
- Turn `SQLITE_DBCONFIG_TRUSTED_SCHEMA` off by default and add
  `Musq::trusted_schema(bool)`. This interacts with §15: with trusted schema
  off, SQLite refuses non-innocuous user functions inside schema expressions
  such as indexes, views, and triggers.
- Add `Musq::defensive(bool)`, default off, for applications that open
  untrusted files.
- `sqlite3_db_config` is variadic and its operations use different argument
  layouts. Replace `ffi::db_config_fp_digits` with one
  `ffi::db_config_int(db, op: DbConfigIntOp, value) -> Result<i32>` wrapper
  where `DbConfigIntOp` is a private enum that lists only the operations with
  the `(int, int*)` layout: `FP_DIGITS`, `DQS_DDL`, `DQS_DML`,
  `TRUSTED_SCHEMA`, `DEFENSIVE`, `ENABLE_FKEY`, `ENABLE_TRIGGER`, and
  `ENABLE_VIEW`.
- State the new defaults in the README "Opening a database" section next to
  the foreign-key default.

Effort: small. Priority: medium.

## 12. Expose SQLite Transaction State And Autocommit

Problem. `ConnectionState::transaction_depth` is a counter that Musq updates
by hand. Any user statement such as `BEGIN`, `COMMIT`, or `ROLLBACK` executed
through `query()` desynchronizes it. `sqlite3_txn_state` (3.34) reports
`NONE`, `READ`, or `WRITE` for the actual connection state, and
`sqlite3_get_autocommit` reports whether an explicit transaction is open.
Neither reports savepoint nesting, so they complement the counter instead
of replacing it.

Change. Add `Connection::transaction_state() -> Result<TxnState>` and
`Connection::is_autocommit() -> Result<bool>` as worker commands. Use
`is_autocommit` in §5 to reconcile the counter after an interrupt, and in a
debug assertion at `Commit` and `Rollback` so a mismatch between the counter
and SQLite's outer state is caught in tests.

Effort: small. Priority: low.

## 13. Use `SQLITE_OPEN_EXRESCODE`

Problem. `EstablishParams::establish` calls `sqlite3_extended_result_codes`
after open (`establish.rs:147`). `SQLITE_OPEN_EXRESCODE` (3.37) sets the
same behavior through the open flags and also applies to errors raised by
`sqlite3_open_v2` itself.

Change. Add the flag to `open_flags` and delete `ffi::extended_result_codes`.

Effort: small. Priority: low.

## 14. Serialize, Deserialize, And Backup-To-Path Snapshots

Problem. `Pool::vacuum_into` gives a file snapshot. Two common needs remain:
copying a database to or from a `Vec<u8>` for tests, fixtures, and transport,
and an incremental online backup that yields between pages. `sqlite3_serialize`,
`sqlite3_deserialize`, and `sqlite3_backup_*` are bundled.

Each Musq connection owns one worker thread. The backup API needs both the
source and destination handles on one thread, and it reserves the destination
handle from `backup_init` to `backup_finish`. A `backup_to(&Connection)` that
spans two workers has no ownership protocol, so it is out of scope.

Change, as three changes.

- Serialize (small). `Connection::serialize(schema) -> Result<Vec<u8>>` as a
  worker command. Copy the SQLite buffer into a `Vec` and free it with
  `sqlite3_free`.
- Deserialize (medium). `Connection::deserialize(schema, bytes, mode)` as a
  worker command. Allocate the buffer with `sqlite3_malloc64`, copy the bytes
  in, and pass `SQLITE_DESERIALIZE_FREEONCLOSE` so SQLite owns it. `mode` is
  `ReadOnly` or `Resizable`. Before the call, refuse when
  `transaction_depth > 0` or when SQLite is not in autocommit, and clear the
  statement cache because cached statements bind to the old schema. Reject a
  WAL-mode image, which `deserialize` does not support.
- Backup to path (medium). `Connection::backup_to_path(path, pages_per_step)`
  as a worker command. The worker opens the destination with `sqlite3_open_v2`
  on its own thread, runs `backup_step` in a loop, and calls `backup_finish`
  on every exit path including cancellation. Report progress through the
  returned `BackupReport { pages, remaining }`. Reject a destination path
  equal to the source file.
- Tests: round-trip a database through bytes; refuse deserialize inside a
  transaction; back up a file database and open the copy.

Effort: small, medium, medium. Priority: medium.

## 15. User-Defined Scalar Functions And Collations

Problem. Musq exposes no way to register Rust functions or collations.
`sqlite3_create_function_v2` and `sqlite3_create_collation_v2` are bundled.
Applications need this for custom ranking, `REGEXP`, normalized text compare,
and for `vec0` metadata predicates.

Change.

- Add `Musq::function(name, n_args, FunctionFlags, impl Fn(&[Value]) ->
  Result<Value> + Send + Sync + 'static)`. Register on every connection at
  establish time so pooled connections behave the same.
- `FunctionFlags` carries `deterministic`, `direct_only`, and `innocuous`.
  Default to `direct_only`. With §11's trusted-schema default off, only an
  `innocuous` function may appear in an index, view, or trigger, and the
  caller must ask for that flag explicitly.
- Add `Musq::collation(name, impl Fn(&str, &str) -> Ordering + Send + Sync +
  'static)`.
- Map a Rust `Err` to `sqlite3_result_error` with the error text. Wrap every
  C entry point in `catch_unwind` and convert a panic into
  `sqlite3_result_error("musq: function panicked")`. Free the boxed closure
  through the `xDestroy` callback.
- Keep aggregate and window functions out of scope until a caller needs them.
- Tests: a deterministic function in a `WHERE` clause; a function that
  returns `Err`; a function that panics; an `innocuous` function in an index
  with trusted schema off; a collation in `ORDER BY`; registration on every
  pool connection.

Effort: large. Priority: medium.

## 16. Commit, Rollback, And Update Hooks

Problem. Change-notification is a frequent request for cache invalidation and
live queries. `sqlite3_update_hook`, `sqlite3_commit_hook`, and
`sqlite3_rollback_hook` are bundled and are per-connection.

Change. Add `Connection::on_update(impl Fn(UpdateEvent) + Send + 'static)`
that delivers `{ op, database, table, rowid }`. Deliver through an unbounded
`flume` channel with `try_send` so the SQLite callback is constant-time and
never blocks. Count dropped events if the receiver is gone. Provide the same
for commit and rollback. Wrap each callback in `catch_unwind`. Document the
per-connection semantics for pools, that a hook must not run SQL on the same
connection, and that the commit hook cannot veto a commit in this API.

Effort: medium. Priority: low.

## C. Public API

## 17. Use `AsyncFnOnce` For `Connection::transaction`

Problem. `Connection::transaction` requires
`for<'c> F: FnOnce(&'c mut Transaction<&'a mut Self>) -> BoxFuture<'c,
StdResult<R, E>> + Send + Sync` (`mod.rs:265-273`). Callers must write
`Box::pin(async move { ... })` and the closure must be `Sync`. The workspace
uses edition 2024 and Rust 1.99. Async closures are stable since 1.85.

Change. Change the bound to `F: AsyncFnOnce(&mut Transaction<&mut Self>) ->
StdResult<R, E>`. Add `Pool::transaction` with the same shape. Update the
README with a plain `pool.transaction(async |tx| { ... }).await?` example,
including one that borrows a local across the `await`.

Effort: small. Priority: medium.

## 18. Collapse `QueryExecutor` Impls And Drop `async-trait`

Problem. `query::QueryExecutor` (`crates/musq/src/query.rs:199`) has five
hand-written impls (`&Pool`, `&Connection`, `&PoolConnection`,
`&Transaction<C>`, `&mut Transaction<C>`) with identical bodies apart from the
`&Pool` case. It uses `#[async_trait]` and hand-written `Pin<Box<dyn Future>>`
signatures. The trait is public but not sealed and not re-exported at the
crate root. The `Execute` trait is sealed, but its `&str` impl
(`crates/musq/src/executor.rs:38`) has no caller.

Change.

- Add a sealed `AsConnection` trait implemented for `Connection`,
  `PoolConnection`, and `Transaction<C>`. Write blanket impls
  `impl<T: AsConnection> QueryExecutor for &T` and `for &mut T`, plus one
  impl for `&Pool`. The `&mut` form keeps `execute(&mut tx)` working, which
  `crates/stresstest/src/main.rs:230-243` uses.
- Replace `#[async_trait]` with native `async fn` in trait plus
  `-> impl Future<Output = ..> + Send` where the `Send` bound matters. Remove
  the `async-trait` dependency.
- Seal `QueryExecutor` and re-export it at the crate root so the bound on
  `Query::execute` is nameable.
- Delete `impl Execute for &str`.

Effort: small. Priority: medium.

## 19. Remove `Prepared` And `Statement`; Keep `prepare` As Validation

Problem. `Statement` is a `String` (`crates/musq/src/sqlite/statement/
mod.rs:22`). `Prepared` wraps that `String`. `Query::statement` is
`Either<String, Statement>` (`query.rs:26`), which is `Either<String,
String>`. `Connection::prepare` calls `prepare_with`, whose doc says "without
caching" while the worker inserts into the cache (`mod.rs:353-371`). Every
connection prepares and caches statements automatically, so `Prepared` adds
no capability. It costs six `query_statement_*` constructors, an `either`
match in every `sql()` call, and a stale `#[allow(clippy::rc_buffer)]`.

The worker-side `prepare` does have one observable effect: it compiles every
statement in the text and warms the cache without executing anything. That
is a useful "validate this SQL" operation.

Change. Delete `Statement`, `Prepared`, `prepare_with`, and the
`query_statement_*` functions. Store `sql: String` in `Query`. Keep
`Connection::prepare(&self, sql: &str) -> Result<()>` with the
`Command::Prepare` worker path, documented as syntax validation and cache
warming. Remove the `rc_buffer` allow.

Effort: small. Priority: medium.

## 20. Render An Empty `{values:}` List As `IN ()`

Problem. `QueryBuilder::push_values` returns an error for an empty iterator
(`query_builder.rs:82`), and `sql!` rejects literal empty arrays at compile
time (`crates/musq-macros/src/sql.rs:16`). A `WHERE id IN ({values:ids})`
with a runtime-empty `ids` fails at execution. SQLite accepts `IN ()` and
evaluates it as false, and `NOT IN ()` as true, which are the correct set
semantics.

Change. Emit nothing for an empty list so the SQL becomes `IN ()`. Keep the
compile-time check for literal empty arrays because that is always a mistake.
Document in the README placeholder table that an empty list yields `IN ()`
(false) and `NOT IN ()` (true for every row), so a `DELETE ... WHERE id NOT
IN ({values:keep})` with an empty `keep` deletes every row. Add a test that
runs `SELECT 1 WHERE 1 IN ()` through the macro and gets no rows, and one for
`NOT IN ()`.

Effort: small. Priority: medium.

## 21. Trim The `Musq` Builder Surface

Problem.

- `Musq::filename` (`musq.rs:238`) is public, but `open(path)` overwrites it.
  `Musq::new().filename("a").open("b")` opens `b`.
- `analysis_limit(Option<u32>)` (`musq.rs:523`) takes an `Option`, while
  `optimize_on_close(bool, impl Into<Option<u32>>)` (`musq.rs:500`) takes a
  flag plus an `Into<Option>`. The two neighbors have different shapes.
- `Musq::new` docs say "See the source of this method for the current
  defaults" (`musq.rs:161`).

Change. Make `filename` `pub(crate)`. Replace `optimize_on_close(bool,
impl Into<Option<u32>>)` with `optimize_on_close(bool)` and make
`analysis_limit(u32)` the single limit that both `ANALYZE` and
`PRAGMA optimize` use. Delete the `OptimizeOnClose` enum. Write the defaults
table into the `Musq::new` docs.

Effort: small. Priority: low.

## 22. Remove Enum-Mode `Default` Impls

Problem. `enum_mode!` generates `Default` for every mode. `JournalMode::
default()` is `Wal` and `Synchronous::default()` is `Full`, but Musq does not
set either pragma by default. A caller who writes
`.journal_mode(JournalMode::default())` gets WAL, which the README says Musq
avoids by default.

Change. Remove the `default` clause from `enum_mode!` and the generated
`Default` impls. A caller who wants a mode names it.

Effort: small. Priority: low.

## 23. Replace The `Null` Type Alias With A Unit Struct

Problem. `pub type Null = Option<bool>` plus
`#[allow(non_upper_case_globals)] pub const Null: Null = None`
(`crates/musq/src/encode.rs:50-54`). The alias leaks `Option<bool>` into the
public API and needs a lint allow.

Change. Define `pub struct Null;` with `impl Encode for Null`. The
`values!{"x": Null}` call site is unchanged.

Effort: small. Priority: low.

## 24. Remove `expr::jsonb_text` And The Stale `hb` Reference

Problem. `expr::jsonb_text` is a one-line alias of `expr::jsonb`
(`crates/musq/src/expr.rs:89`). `expr::now_rfc3339_utc` docs say "intended to
match hb's documented storage format" (`expr.rs:61`). `hb` is not this
project.

Change. Delete `jsonb_text`. Reword the `now_rfc3339_utc` doc to describe the
format it emits.

Effort: small. Priority: low.

## D. Structure And Elegance

## 25. Simplify `Arguments::bind`

Problem. The `$`, `:`, and `@` branches in `Arguments::bind`
(`crates/musq/src/sqlite/arguments.rs:104-150`) repeat the same twelve lines
three times. The `atoi` crate is used once here for `$NNN`.

Change. Split the parameter name once into `(prefix, rest)`, handle the
numeric `$NNN` and `?NNN` cases first, then run one shared named-lookup path.
Use `rest.parse::<usize>()` and drop `atoi`.

Effort: small. Priority: low.

## 26. Share One SQL Scanner

Problem. `contains_numeric_parameter` (`query_builder.rs:404`) and
`rewrite_named_parameters` (`query_builder.rs:496`) each contain a full copy
of the same five-state string/comment scanner. A future fix to quoting rules
must be made twice.

Change. Write one `fn scan_sql(sql, |token: SqlToken| ...)` that yields
`Text`, `Placeholder { prefix, name }`, and `Numeric` spans outside strings
and comments. Implement both passes on top of it. Add a small table test for
the scanner alone.

Effort: small. Priority: low.

## 27. Collapse The Five `emit_query_event_*` Functions

Problem. `logger.rs:92-186` has five functions that differ only in the
`Level` literal because `tracing::event!` needs a constant level.

Change. Replace them with a local `macro_rules! emit_at { ($lvl:expr, ...) }`
invoked from a single `match`. Also remove the duplicated
`increment_rows_returned`/`inc_rows_returned` pair by implementing `QueryLog`
directly on the struct methods.

Effort: small. Priority: low.

## 28. Store Column Names Once And Validate TEXT Once Behind A Newtype

Problem.

- `Row::column_names()` sorts a `Vec` from the `HashMap` on every call
  (`row.rs:157`). `Row::get_value_idx` scans the map to find a name for the
  error message.
- `Row::current` validates UTF-8 for every TEXT column (`row.rs:95`), then
  `Value::text()` (`value.rs:116`) and `Value::bind` (`value.rs:149`) validate
  again on every decode and bind. The repeated validation is required today
  because `Value::Text { value: Bytes }` is a public constructor and external
  code can supply arbitrary bytes.

Change. Keep `Arc<[Arc<str>]>` (index order) beside the name-to-index map in
`CompoundStatement`. Change `Value::Text { value: Bytes }` to
`Value::Text { value: Text }` where `Text` is a newtype with private
validated bytes, `Text::new(String)`, `Text::from_utf8(Bytes) -> Result`,
`From<&str>`, `From<String>`, and `as_str(&self) -> &str`. `Row::current`
validates once and constructs through a crate-private unchecked path.
`Value::text()` and `Value::bind` become infallible on the text path.

Effort: small. Priority: low.

## 29. Fold `Column` And `Error::TypeNotFound` Away

Problem. `Column` has one field, `type_info` (`crates/musq/src/column.rs:7`).
`SqliteDataType::from_str` returns `Error::TypeNotFound` (`type_info.rs:117`),
but the only caller, `StatementHandle::column_decltype` (`handle.rs:84`),
discards the error with `.ok()`. The public `Error::TypeNotFound` and
`Error::UnknownColumnType` variants cannot reach a user through any current
path except `Row::current` for an unknown column code, which SQLite never
returns.

Change. Replace `Column` with `Vec<(Arc<str>, SqliteDataType)>` inside
`CompoundStatement`. Make `SqliteDataType::from_str` return `Option`. Remove
`Error::TypeNotFound` and `Error::UnknownColumnType`, and map an unknown
column code to `Error::Protocol` because it is a driver invariant.

Effort: small. Priority: low.

## 30. Remove Dead Code Behind `#[allow(dead_code)]`

Problem. `worker.rs:342` (`is_shutdown` returns a constant `false`),
`worker.rs:59` (`Command` enum), `worker.rs:438` (`oneshot_cmd`),
`mod.rs:353` (`prepare_with`), and a
`// removed executor trait implementation module` comment (`mod.rs:39`) are
leftovers. The `TODO` at `pool/connection.rs:82` is stale.

Change. Delete `is_shutdown`, the stale comments, and the `TODO`. Remove the
`dead_code` allows on `Command` and `oneshot_cmd`; both have live callers.
§19 handles `prepare_with` and the `rc_buffer` allow.

Effort: small. Priority: low.

## E. Dependencies And Tooling

## 31. Trim Dependencies And Feature Flags

Problem. `crates/musq/Cargo.toml` pulls in more than the code uses:

- `tokio = { features = ["full"] }` (line 20). The library uses `sync`,
  `time`, `rt` (`Handle::try_current`), and `macros` (`select!`). `full`
  adds `net`, `fs`, `process`, `signal`, and `io-util` to every consumer.
- `either` with `features = ["serde"]` (line 29). No `Either` is serialized.
- `serde` with `features = ["rc"]` (line 38). No `Arc`/`Rc` field is
  serialized. `SqliteDataType` derives `Serialize`/`Deserialize` with no
  caller, which is the only reason `serde` is a non-optional dependency.
- `time` with `features = ["serde"]`. No use in this crate.
- `atoi` (§25), `async-trait` (§18), `futures-executor` (one `block_on`
  that a std `mpsc::sync_channel(0)` ack replaces), and `sqlformat`
  (pretty-prints SQL only for the `debug` log line).
- `libsqlite3-sys` enables `pkg-config` and `vcpkg` features while
  `build.rs` refuses any external SQLite. The two features can go.

Change. Set the minimal `tokio` features, drop the unused feature flags,
and remove `atoi`, `async-trait`, `futures-executor`, and `sqlformat`. Log
the raw SQL text. Make `time`, `bstr`, and `serde_json` optional behind
`time`, `bstr`, and `json` features, enabled by default beside `vec`, so a
consumer that stores only integers and text can turn them off. Remove the
`Serialize`/`Deserialize` derives from `SqliteDataType` so `serde` is only
needed by the `json` feature. The optional features touch the public types
table, the `Json` derive, examples, and the README, so run this after §18 and
§19 have settled the API.

Effort: medium. Priority: medium.

## 32. Remove `musq-test` And The Root `examples/` Indirection

Problem.

- `crates/musq-test` duplicates `crates/musq/tests/support/mod.rs` line for
  line. No crate depends on it. It is compiled by every `cargo test`.
- `examples/vec.rs` and `examples/custom_type.rs` live at the workspace root,
  which is a virtual workspace with no package. `crates/musq/examples/vec.rs`
  is an `include!("../../../examples/vec.rs")` shim (`vec.rs:3`).
  `examples/custom_type.rs` constructs `Value::Text { value: v, .. }` with a
  `String`, which does not compile against the current `Bytes` field, and
  nothing compiles it.

Change. Delete `crates/musq-test`. Move `examples/vec.rs` into
`crates/musq/examples/vec.rs` and delete the shim. Fix `custom_type.rs` and
move it into the crate examples so it compiles under `cargo test`.

Effort: small. Priority: medium.

## 33. One Source Of Truth For The Bundled SQLite Version

Problem. The bundled version string is repeated in `README.md:397`,
`crates/musq/src/lib.rs:7`, and
`crates/musq/tests/sqlite_capabilities.rs:12`. The next `libsqlite3-sys`
bump needs three edits, and the docs drift when one is missed.

Change. Have `build.rs` read `SQLITE_VERSION` from the `libsqlite3-sys`
amalgamation through `DEP_SQLITE3_INCLUDE` and export it as
`musq::BUNDLED_SQLITE_VERSION`. Make the capability test compare
`sqlite_version()` against that constant. Have the README and crate docs
state "the SQLite release bundled by the pinned `libsqlite3-sys`" and point at
`runtime_info()` and the constant instead of a literal version.

Effort: small. Priority: low.

## F. Documentation

## 34. Remove Stale SQLx-Era Documentation

Problem. Several doc comments still describe a network database server:

- `Connection::close` talks about a "database server", "TCP keepalive
  timeout", and "connection limit or quota" (`mod.rs:118-136`).
- `Pool::close` mentions "connection limits being exhausted" (`pool/mod.rs:
  205`).
- `Musq::journal_mode` references `musq-cli` (`musq.rs:280`), which does not
  exist.
- `Musq::serialized` tells users to open an issue at
  `launchbadge/sqlx/issues` (`musq.rs:394`).
- Every `PrimaryErrCode` and `ExtendedErrCode` variant is documented as
  "SQLite error code variant." (`sqlite/error.rs:17` onward), which satisfies
  the `missing_docs` lint without informing the reader.

Change. Rewrite the close docs for a single-process embedded database.
Delete the `musq-cli` sentence and the SQLx link. Give the error-code
variants their SQLite meaning in one short line each, or generate the enums
with a macro that carries the doc string next to the constant.

Effort: small. Priority: low.

## 35. Keyword Handling In `{upsert:...}` Exclude Lists

Problem. `UpsertArgs::parse` (`crates/musq-macros/src/sql.rs:127-190`)
special-cases nineteen Rust keywords one `if` at a time so that a column named
`type` can appear in `exclude:`. Any keyword not in the list still fails.

Change. Parse each list element with `syn::Ident::parse_any` (from
`syn::ext::IdentExt`), which accepts keywords in one call. This removes the
whole chain. Also replace the `Span::call_site()` errors at `sql.rs:239` and
`sql.rs:257` with the span of the format literal so compile errors point at
the string.

Effort: small. Priority: low.

## Implementation Checklist

This checklist is the live execution record for the plan. Each item is one
coherent change that leaves the workspace consistent. Tick an item only after
its code, tests, and docs are in and the stage gate has passed. Add discovered
work as new items under the stage where it belongs and note it under
"Checklist Adjustments". A fresh agent must be able to continue from this
list without chat history.

Stage gate, run at the end of every stage and before every commit:

1. `cargo clippy --fix --allow-dirty --tests --examples --benches`, then
   review and keep only intended fixes; no warnings remain.
2. `cargo fmt`.
3. `cargo test`.
4. `cargo test -p musq --no-default-features`.
5. `cargo doc --no-deps` with no warnings.
6. `git diff --check`.

Every step changes signatures and defaults in place. No step adds a
deprecation shim or a transition default.

### Stage 1: Bug Fixes And Small Corrections

1. [x] §7. In `ffi::finalize`, capture `sqlite3_db_handle` before
   `sqlite3_finalize`; never read the statement after. In
   `StatementHandle::drop`, remove the `panic!`, the post-finalize
   `db_handle()` read, and log at `error` level only. Done when a unit test
   drops a handle whose finalize fails and the process neither panics nor
   reads freed memory (run once under `RUSTFLAGS=-Zsanitizer=address` on
   nightly or under valgrind and record the result here).
   ASan: `RUSTFLAGS="-Z sanitizer=address" cargo +nightly test -p musq --lib
   --target aarch64-apple-darwin -- finalize_after_failed_step
   drop_after_failed_step` passed (2 tests, 2026-08-29). Compiling C sources
   with Apple Clang `-fsanitize=address` failed to link against rustc's ASan
   runtime (`___asan_version_mismatch_check_apple_clang_2100`); the recorded
   run instruments Rust only.
2. [x] §4. Make `Error::Sqlite(#[from] SqliteError)`. Delete
   `Error::into_sqlite_error`, the duplicated `is_busy` /
   `is_unique_violation`, and the field-copy `From`. Change
   `SqliteError.extended` to `Option<ExtendedErrCode>`. Add
   `SqliteError.offset: Option<usize>` from `sqlite3_error_offset`. Update
   `Display`. Done when `tests/error.rs` asserts `extended == None` and the
   byte offset for `SELECT * FORM t`.
3. [x] §3. Add `Error::Configuration(String)` and `Error::Query(String)`.
   Move every `Protocol` call site listed in §3 to the right variant. Done
   when `grep -rn "Error::Protocol" crates/musq/src` shows only invariant
   violations and the `tests/sqlite_connection_control.rs`
   `assert_protocol_contains` helper is renamed to match.
   Remaining `Error::Protocol` sites: `row.rs` (null TEXT pointer),
   `worker.rs` (invalid parser-depth limit from SQLite), and
   `establish.rs` (SQLite did not honor `FP_DIGITS`).
4. [x] §8. Add `ffi::changes64`; use it in `StatementHandle::changes`. Delete
   `ffi::changes`.
5. [x] §9. Delete `unsafe impl Sync for Connection`, `unsafe impl Sync for
   ConnectionWorker`, `unsafe impl Send/Sync for Row`, and the empty
   `impl Drop for Connection`. Add `assert_send_sync::<T>()` checks in each
   file's test module.
6. [x] §10. Change `Connection::close(&self)` to `close(self)`. Update
   `PoolConnection::close`, `Idle::close`, and `Floating::close`.
7. [x] Stage gate.

### Stage 2: Transactions

1. [x] §1. Add `TransactionBehavior { Deferred, Immediate, Exclusive }`
   with `Immediate` as default. Add `Musq::default_transaction_behavior`,
   `Pool::begin_with`, `Connection::begin_with`. Extend `Command::Begin`.
2. [x] §1. Replace `begin_ansi_transaction_sql`,
   `commit_ansi_transaction_sql`, and `rollback_ansi_transaction_sql` with
   the depth matrix in §1 (`BEGIN`/`COMMIT`/`ROLLBACK` at depth 0→1,
   `SAVEPOINT`/`RELEASE`/`ROLLBACK TO` deeper). Make `commit` and `rollback`
   consume `self`.
3. [x] §1 tests in `tests/transaction_signatures.rs` and a new
   `tests/transaction_behavior.rs`: autocommit true after top-level commit
   and rollback under each behavior; nested savepoint commit and rollback
   leave the outer transaction open; second-connection write inside an
   `Immediate` transaction fails with `PrimaryErrCode::Busy` after the busy
   timeout; `Deferred` read-then-write reproduces `BusySnapshot`. Use a
   file database in WAL mode.
4. [x] README: update the Transactions section for `Immediate` default,
   `begin_with`, and consuming `commit`/`rollback`.
5. [x] Stage gate.

### Stage 3: SQLite Locking Layer

1. [x] §2. Change `configure_in_memory` to `/musq-in-memory-N` plus
   `.vfs("memdb")`. Remove `shared_cache`, `in_memory`, and
   `SQLITE_OPEN_MEMORY`. Done when `tests/in_memory_settings.rs` and
   `tests/concurrent.rs` pass and `PRAGMA compile_options` is not consulted
   for `ENABLE_UNLOCK_NOTIFY`.
2. [x] §2. Delete `sqlite/statement/unlock_notify.rs`,
   `Error::UnlockNotify`, `DEFAULT_MAX_RETRIES`, both retry arms in
   `StatementHandle::step`, the retry loop in `ConnectionHandle::exec`, and
   the `unlock_notify` feature in `crates/musq/Cargo.toml`. Make `ffi::step`
   return any code other than `ROW`/`DONE` as an error. Update
   `tests/sqlite_capabilities.rs`.
3. [x] §6. Mark every raw-pointer `ffi` function `unsafe fn`. Move the
   `unsafe` blocks into `ConnectionHandle` and `StatementHandle` methods
   with one-line justifications. Reduce `ffi` item visibility to
   `pub(super)`.
   Visibility is `pub(in crate::sqlite)` so connection and statement
   submodules can call the wrappers. `libversion_number`, `register_vec`,
   and `auto_extension` stay safe.
4. [x] Stage gate. Also run `tests/connection_flows.rs::retry_on_busy_lock`
   and confirm it passes through SQLite's busy handler alone.

### Stage 4: Public API Tightening

1. [x] §18. Add sealed `AsConnection`; blanket `QueryExecutor for &T` and
   `&mut T`; one impl for `&Pool`. Replace `#[async_trait]` with native
   `async fn` in trait. Seal and re-export `QueryExecutor`. Delete
   `impl Execute for &str`. Remove `async-trait` from `Cargo.toml`.
2. [x] §19. Delete `Statement`, `Prepared`, `prepare_with`, the
   `query_statement_*` functions, and the `rc_buffer` allow. Store
   `sql: String` in `Query`. Keep `Connection::prepare(&str) -> Result<()>`
   as validation. Remove `either` from `Query`.
3. [ ] §17. Change `Connection::transaction` to `AsyncFnOnce`. Add
   `Pool::transaction`. README example with a borrowed local across `await`.
4. [ ] §20. Empty `push_values` renders nothing (`IN ()`). Keep the
   compile-time empty-literal check. README placeholder table note on
   `IN ()` and `NOT IN ()`. Tests for both.
5. [ ] §22. Remove the `default` clause from `enum_mode!`.
6. [ ] §21. `filename` to `pub(crate)`; `optimize_on_close(bool)`;
   `analysis_limit(u32)`; delete `OptimizeOnClose`; defaults table in
   `Musq::new` docs.
7. [ ] §23. `pub struct Null;` with `Encode`; remove the alias and the lint
   allow.
8. [ ] §24. Delete `expr::jsonb_text`; reword `now_rfc3339_utc` docs.
9. [ ] Stage gate. Run `cargo run -p musq --example readme_snippets` and
   `readme_quickstart` to confirm the README compiles.

### Stage 5: Connection Configuration And Worker State

1. [ ] §11. Add private `DbConfigIntOp` enum and `ffi::db_config_int`.
   Delete `db_config_fp_digits`. Turn `DQS_DDL`, `DQS_DML`, and
   `TRUSTED_SCHEMA` off at establish. Add `Musq::double_quoted_strings`,
   `Musq::trusted_schema`, `Musq::defensive`. Test that
   `SELECT "no_such_column"` fails. README defaults note.
2. [ ] §5. Give `ConnectionState` to the worker by value. New
   `WorkerSharedState { cached_statements_size, transaction_depth,
   db: Mutex<Option<NonNull<sqlite3>>> }` with a drop guard that clears `db`
   on every worker exit path. `Transaction::fmt` reads the atomic.
3. [ ] §5. Add `Connection::interrupt` and `PoolConnection::interrupt`.
   After any `SQLITE_INTERRUPT`, reconcile `transaction_depth` through
   `sqlite3_get_autocommit`; add `Error::TransactionAborted`.
4. [ ] §5. Add `Musq::statement_timeout(Duration)` through
   `sqlite3_progress_handler`.
5. [ ] §5 tests in a new `tests/interrupt.rs`: interrupt a recursive CTE;
   interrupted write inside a transaction makes `commit` return
   `TransactionAborted`; `interrupt` racing `close` in a 1000-iteration loop
   does not crash; `statement_timeout` fires inside a transaction.
6. [ ] §12. Add `Connection::transaction_state()` and
   `Connection::is_autocommit()` worker commands. Debug-assert the counter
   against `is_autocommit` at `Commit` and `Rollback`.
7. [ ] §13. Add `SQLITE_OPEN_EXRESCODE` to open flags; delete
   `ffi::extended_result_codes`.
8. [ ] Stage gate.

### Stage 6: New Capability

1. [ ] §14a. `Connection::serialize(schema) -> Result<Vec<u8>>`.
2. [ ] §14b. `Connection::deserialize(schema, bytes, DeserializeMode)` with
   `sqlite3_malloc64` + `FREEONCLOSE`, refusal inside a transaction, cache
   clear, WAL-image rejection. Round-trip test.
3. [ ] §14c. `Connection::backup_to_path(path, pages_per_step) ->
   Result<BackupReport>` on the source worker, `backup_finish` on every
   exit path, same-file rejection. Test backs up a file DB and opens the
   copy.
4. [ ] §15. `Musq::function(name, n_args, FunctionFlags, f)` and
   `Musq::collation(name, f)`, registered at establish. `FunctionFlags {
   deterministic, direct_only, innocuous }` with `direct_only` default.
   `catch_unwind` at every C entry point; `xDestroy` frees the closure.
   Tests listed in §15.
5. [ ] §16. `Connection::on_update`, `on_commit`, `on_rollback` through an
   unbounded `flume` channel with `try_send` and a dropped-event counter.
   `catch_unwind` in each callback. Docs on per-connection semantics.
6. [ ] Stage gate.

### Stage 7: Dependencies, Structure, And Docs

1. [ ] §31. Minimal `tokio` features; drop `either/serde`, `serde/rc`,
   `time/serde`; remove `atoi`, `futures-executor`, `sqlformat`; drop
   `pkg-config` and `vcpkg` from `libsqlite3-sys`. Make `time`, `bstr`, and
   `json` default-on features. Remove serde derives from `SqliteDataType`.
   Run `cargo test -p musq --no-default-features` and
   `cargo test -p musq --features vec` as part of the gate.
2. [ ] §32. Delete `crates/musq-test`. Move `examples/vec.rs` and a fixed
   `examples/custom_type.rs` into `crates/musq/examples/`; delete the
   `include!` shim and the root `examples/` directory.
3. [ ] §33. `build.rs` exports `BUNDLED_SQLITE_VERSION` from
   `DEP_SQLITE3_INCLUDE`; capability test uses it; README and `lib.rs` stop
   naming a literal version.
4. [ ] §25. One shared named-lookup path in `Arguments::bind`.
5. [ ] §26. One `scan_sql` tokenizer under both `contains_numeric_parameter`
   and `rewrite_named_parameters`, with a table test.
6. [ ] §27. One `emit_at!` macro in `logger.rs`; merge the duplicated
   row-counter methods.
7. [ ] §28. `Arc<[Arc<str>]>` name list in `CompoundStatement`; `Text`
   newtype with private validated bytes in `Value::Text`.
8. [ ] §29. Delete `Column`, `Error::TypeNotFound`,
   `Error::UnknownColumnType`; `SqliteDataType::from_str` returns `Option`.
9. [ ] §30. Delete `is_shutdown`, stale comments, the `TODO`, and the
   remaining `dead_code` allows.
10. [ ] §34. Rewrite the close docs; delete `musq-cli` and SQLx references;
    document each error-code variant.
11. [ ] §35. `Ident::parse_any` in `UpsertArgs::parse`; format-literal spans
    for `sql!` errors.
12. [ ] Stage gate. Also run `cargo outdated --root-deps-only --workspace`
    and record the result here.

### Checklist Adjustments

Record added, removed, split, reordered, or deferred items here with the
date and reason.

- 2026-08-29: Stage 2 added `Connection::is_autocommit()` so the §1 tests
  can read `sqlite3_get_autocommit`. Stage 5 §12 still adds
  `transaction_state()` and the commit/rollback debug asserts.
- 2026-08-29: Stage 2 removed the `SQLITE_BUSY` unlock-notify retry in
  `StatementHandle::step` so a blocked Immediate writer returns
  `PrimaryErrCode::Busy`. Stage 3 still deletes remaining unlock-notify.
