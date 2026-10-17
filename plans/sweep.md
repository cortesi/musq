# Musq Clear-Wins And Deslop Sweep

Date: 2026-09-12. Base revision: `main` at `fca14b9`.

This is a combined clear-wins and deslop survey. It records improvements that
can be implemented without further design work, plus the strongest structural
simplifications. No code was modified during the survey.

Method: direct reading of every source file in `crates/musq/src` and
`crates/musq-macros/src`, plus four focused audits of the integration tests,
the macro crate, build/bench/docs/tooling, and stale references/dead code. Each
finding below cites the source that proves it. A previous improvement plan,
`plans/improve.md`, is fully implemented; nothing here repeats it.

Assumptions: the repository treats pre-1.0 API changes as free, as stated in
`plans/improve.md`. Findings that need a product decision are collected under
"Decisions Needed" and are not counted as clear wins.

Baseline facts:

- 264 default-feature tests (204 explicit plus macro-generated cases) and 38
  trybuild cases live under `crates/musq/tests`.
- `api/` snapshots are generated but untracked (`git status` shows `?? api/`).
- There is no CI configuration.
- `cargo xtask tidy` is broken: it passes
  `--config-path ./rustfmt-nightly.toml`, and that file was deleted in
  `d8ebc44`.

Summary:

| Section | Items | Priority |
| --- | --- | --- |
| A. Correctness | C1-C6 | High |
| B. Documentation and stale references | C7-C12 | Medium |
| C. Dead code and cleanup | C13-C19 | Medium |
| D. Dependencies and packaging | C20-C23 | Medium |
| E. Deslop (structural simplification) | C24-C33 | Medium |
| F. Test suite | C34-C39 | Medium / Low |
| G. Examples, benches, stress test | C40-C42 | Low |
| Decisions needed | D1-D3 | — |

## Changes

### A. Correctness

#### C1: Clear the published SQLite pointer before the handle closes

`WorkerSession` declares `_clear_db: ClearDbOnDrop` last
(`crates/musq/src/sqlite/connection/worker.rs:333-347`). Rust drops struct
fields in declaration order, so on any panic or early-return path `conn`
drops first (closing the handle through `ConnectionHandle::drop`) and
`ClearDbOnDrop` clears the published pointer afterward. During that window,
`WorkerSharedState::interrupt` can hold the mutex, observe `Some`, and call
`sqlite3_interrupt` on a closed handle, violating the safety comment at
`worker.rs:151-158`. The normal `shutdown` path clears the pointer explicitly
before `close` (`worker.rs:693-703`), which is why this only shows on unwind.
Fix: declare `_clear_db` as the first field, or clear in an explicit
`Drop for WorkerSession` (which runs before fields drop). Add a unit test that
drops a session and asserts the published pointer is `None` while the handle
is still open.

#### C2: `Json` and `Decode` derives generate invalid generics

`json.rs:18-22` appends `'r` to the end of the generic parameter list, so any
generic type produces `impl<T, 'r> ...`, which is not valid Rust. The test
named `Generic` in `crates/musq/tests/trybuild/pass_json.rs:8` has no type
parameters and does not cover the case. Separately, `decode.rs:28,87,136` and
`row.rs:53,230` insert a fixed lifetime (`'r` or `'a`) at position zero
without checking for a collision with a user-declared lifetime. Fix: add one
shared helper in `core.rs` that selects a fresh lifetime (reuse `'r` when
free, otherwise `'r_1`, ...) and inserts it after the existing lifetimes;
use it from `json`, `decode`, and `row`. Add a trybuild pass case with
`struct Foo<T>(T)` and one with a user lifetime.

#### C3: `sql!` panics on an invalid named placeholder

`sql.rs:283` constructs `syn::Ident::new(&name, ...)` for the bare `{name}`
shorthand. A format string such as `sql!("SELECT {1abc}")` makes the
procedural macro panic instead of emitting a spanned error. Fix: parse the
name with `syn::parse_str::<syn::Ident>` (or `Ident::parse_any`) and return a
`syn::Error` carrying the format-literal span. Add a fail trybuild case.

#### C4: `sql!` misparses `==` as a named argument

`SqlMacroInput::parse` detects named arguments with
`input.peek(Ident) && input.peek2(Token![=])` (`sql.rs:55`). `Token![=]`
peeks the first byte of `==`, so `sql!("...", a == b)` is read as a named
argument and then fails to parse. Fix: exclude `Token![==]` from the peek.
Add a pass trybuild case that passes a comparison expression.

#### C5: `FromRow` applies `rename_all` to explicit renames and mismatches its docs

`row.rs:63-68` and `row.rs:133-138` build the column name by taking `rename`
if present and then always passing it through `container.rename_all`. Serde
semantics (and the trait docs) make an explicit `rename` win over
`rename_all`. The docs in `from_row.rs:52-70` claim field names are verbatim
by default and list accepted values (`lowercase`, `UPPERCASE`, `camelCase`,
`PascalCase`, `SCREAMING_SNAKE_CASE`, `kebab-case`) that darling rejects; the
actual default is `SnakeCase` (`core.rs:40-58,125-126`) and the accepted
spellings are `snake_case`, `lower_case`, `upper_case`, `screaming_snake_case`,
`kebab_case`, `camel_case`, `pascal_case`, and `verbatim`. Fix: apply
`rename_all` only when `rename` is absent; set the `RowContainer` default to
`Verbatim` (keep `SnakeCase` for `TypeContainer`, which documents snake-cased
enum names); update the docs. Add unit tests for explicit rename plus
`rename_all`, and for a raw identifier such as `r#type`.

#### C6: Build guard accepts a value that breaks the build

`build.rs:64-67` treats `""` and `false` as "disabled" for
`LIBSQLITE3_SYS_USE_PKG_CONFIG`, but `libsqlite3-sys` 0.38.1 enables the
non-bundled path for any value other than exactly `"0"` (its `build.rs:53`,
`is_some_and(|s| s != "0")`). Setting `LIBSQLITE3_SYS_USE_PKG_CONFIG=false`
therefore passes the guard and then selects `build_linked`, which never emits
`DEP_SQLITE3_INCLUDE`; musq then panics with "DEP_SQLITE3_INCLUDE is missing"
instead of the intended linkage-policy message. Fix: match the upstream
off-value (`!= "0"`) for that variable, or treat all four variables
consistently and keep the explicit `DEP_SQLITE3_INCLUDE` error as a
diagnostic. Note the other three variables only affect `build_linked`, so
their hard failure is advisory; document that.

### B. Documentation And Stale References

#### C7: Remove a duplicated `backup_to_path` paragraph from `on_update`

`crates/musq/src/sqlite/connection/mod.rs:225-230` contains the
`backup_to_path` summary immediately above the `on_update` docs (231-237), so
rustdoc shows the backup text as part of `on_update`. The same paragraph is
already correct at `mod.rs:301-306`. Delete lines 225-230.

#### C8: Fix two `Pool` doc errors

- `pool/mod.rs:89`: `acquire` says a timeout returns `Error::PoolClosed`;
  the timeout path returns `Error::PoolTimedOut` (`pool/inner.rs:256,276`).
- `pool/mod.rs:289`: `num_idle` says "connections active and idle (not in
  use)"; it returns idle connections only (`PoolStats::num_idle` at
  `pool/mod.rs:63` is correct).

#### C9: Remove references to a nonexistent `query!()` macro

`query.rs:168` and `query.rs:176` tell readers to use `query!()`, which does
not exist in this crate; the entry points are `sql!` and `sql_as!`.

#### C10: Delete stale SQLx, editor, and dead-type comments

- `pool/connection.rs:150-152`: a `launchbadge/sqlx/issues/1396` link and a
  2022 IntelliJ Rust note. Keep the short rationale, drop the link.
- `musq.rs:206`: cite the SQLite WAL documentation already linked at
  `musq.rs:201` instead of a SQLx pull-request comment.
- `sqlite/connection/handle.rs:7-9`: the type doc refers to a
  `ConnectionHandleRef` that no longer exists.
- `sqlite/connection/handle.rs:18-28`: the comment claims SQLite refuses
  work unless `SQLITE_CONFIG_MULTITHREAD` conditions hold and that "we refuse
  to work" if not; no such check exists. Rewrite it to describe the actual
  `SQLITE_OPEN_NOMUTEX`/`SQLITE_OPEN_FULLMUTEX` choice.
- `tests/sqlite.rs:763,787,813`: regression tests carry SQLx issue URLs.
  Rewrite as descriptions of the regression.

#### C11: Fix `FromRow` docs

- "satifisfy" and a reference to a nonexistent `type::Type` trait at
  `from_row.rs:131-132`.
- Replace the incorrect `rename_all` value list and default (C5) in the same
  doc block.

#### C12: Fix README runtime-introspection bullets

`README.md:439-444` lists `runtime_info()`, `db_status(kind, reset)`, and
`wal_checkpoint(schema, mode)` as if they were free functions. They are
methods, `db_status` exists on `Connection` only (not `Pool`), and
`wal_checkpoint` exists on both. Make the signatures and owners explicit.
Also update the README `musq = { version = "0.0.4" }` line if a release is cut
(see C22).

### C. Dead Code And Cleanup

#### C13: Delete unused `DbConfigIntOp` variants

`ffi.rs:113-121` defines `EnableFkey`, `EnableTrigger`, and `EnableView` with
`#[allow(dead_code)]`; they are only referenced by the `as_raw` match and are
never constructed. Foreign keys are set through the pragma path. Delete the
variants, their match arms, and the allows. If they are meant to be exposed,
wire them to builder options instead of leaving them dead.

#### C14: Delete dead items in `musq-macros/src/core.rs`

- `core.rs:15-17`: `span_err` is re-exported `pub(crate)` but never imported;
  the `#[allow(unused)]` hides it. Delete the re-export.
- `core.rs:19,35`: the `#[allow(unused)]` on `assert_errors_with` and its
  re-export is stale; only test modules import it. Gate the re-export with
  `#[cfg(test)]` and drop the allows.
- `core.rs:286-292`: `check_repr_enum_attrs` can never fail because it is
  only called when `attrs.repr` is `Some` (`core.rs:334-337`). Delete the
  function and the call.
- `core.rs:106-109` (`JsonContainer._data`), `core.rs:255-257`
  (`TypeVariant._fields`), and `core.rs:275-283`
  (`TypeField.ident`/`_rename`) are never read. Delete the skipped fields;
  also decide whether `#[musq(rename = ...)]` on a tuple newtype should be an
  error rather than silently accepted.
- `core.rs:350-360`: the test uses bare `#[rename(bar)]`, which darling
  ignores; it does not test rename.
- `decode.rs:178-181`: stale "rust spuriously detects this import" comment and
  `#[allow(unused_imports)]` on a glob import that is used.

#### C15: Remove redundant work and dead branches

- `query.rs:435,456` and `values.rs:26`: explicit `drop(value)`/`drop(self)`
  calls are redundant; the value drops at end of scope anyway.
- `query_builder.rs:235-237`: the final `if first { return Err(...) }` in
  `push_upsert` is unreachable. `values.keys().all(...)` at 216 already
  returns when every key is excluded, and a non-empty remainder always emits
  at least one clause.

#### C16: Remove packaging leftovers

- `crates/musq-macros/Cargo.toml:11` excludes `tests_disabled/**`, which no
  longer exists.
- `crates/musq-macros/wip/.gitignore` and `crates/musq/wip/.gitignore` are
  empty self-ignoring directories.

#### C17: Remove `xtask` and its Cargo alias

`crates/xtask/src/main.rs:50-59` runs `cargo +nightly fmt -- --config-path
./rustfmt-nightly.toml`, but that config was deleted; `Task::Test`
(`main.rs:78-87`) duplicates `ncode test`. Per the repository Rust
conventions, remove an `xtask` that only duplicates Ncode. Delete the crate,
the `crates/*` member entry (implicit), and the `xtask` alias in
`.cargo/config.toml`, then update `AGENTS.md` if it mentions xtask (it does
not today).

#### C18: Small hardening fixes

- `statement_cache.rs:31`: `.unwrap()` after `insert`/`get_mut` is an
  invariant; use `expect("statement was just inserted")`.
- `function.rs:353-357`: the fallback is
  `CString::new(message).unwrap_or_else(|_| CString::new("musq function
  error").unwrap())`; use a `c"musq function error"` literal and drop the
  second unwrap.

#### C19: Allow and visibility cleanup

- `logger.rs:49`: `private_level_filter_to_levels` is `pub` inside a private
  module and only called at `logger.rs:205`; make it private.
- `tests/sqlite_capabilities.rs:3`: `#![cfg_attr(not(test), allow(
  missing_docs))]` is inert because integration tests compile with
  `cfg(test)`, and the file has a `//!` doc; delete it.
- `tests/logger.rs:1`: `#![allow(dead_code)]` is unexplained; narrow it to
  the module items that are genuinely unused when the file is compiled as a
  test binary.

### D. Dependencies And Packaging

#### C20: Replace the deprecated `tempdir` dev-dependency

`crates/musq/Cargo.toml:73` uses `tempdir = "0.3.7"`, which is deprecated and
pulls in `rand 0.4.6`. It is used in six test files:
`sqlite_connection_control.rs:11`, `vacuum_into.rs:10`, `error.rs:153`,
`snapshot.rs:6`, `transaction_behavior.rs:12`, `sqlite.rs:869`. `tempfile` is
already in the dependency graph through `stresstest`. Replace with
`tempfile = "3"` and update the call sites (`TempDir::new("x")` becomes
`tempfile::tempdir()`).

#### C21: Trim unused `futures-util` features

`crates/musq/Cargo.toml:38-42` enables `alloc`, `sink`, and `io`. Source and
tests import only `FutureExt`, `StreamExt`, `TryFutureExt`, `TryStreamExt`,
`future`, and `pin_mut`, none of which need `sink` or `io`. Remove those two
features and verify with `cargo test -p musq --all-features`.

#### C22: Align Cargo metadata and ship the license

- Workspace `Cargo.toml:27` declares `musq = { path = "crates/musq" }` while
  `musq-macros` on line 28 carries `version = "0.0.4"`; add the same version
  for symmetry and to guard against accidental path-only resolution.
- `cargo package -p musq --list` omits `LICENSE`; the crate sets
  `license = "MIT"` but no `license-file`. Add
  `license-file = "../../LICENSE"` to `crates/musq/Cargo.toml` and
  `crates/musq-macros/Cargo.toml` so the package ships it.
- The tree is `0.0.4`, published as `v0.0.4` on 2026-07-06, with 47 commits
  since. Bump the workspace version before the next release and update the
  README/trybuild fixture versions accordingly.

#### C23: Track the API snapshots

`api/musq.rs` and `api/musq-macros.rs` are generated by `ncode api` /
ruskel-snapshot and are currently untracked. Project conventions require API
captures to be checked in and versioned with source so diffs review API
changes. Add them to git and keep the pre-commit hook or `ncode api` refresh
in the loop. The captures currently propagate the incorrect `rename_all` doc
text fixed by C5, so regenerate after C5.

### E. Deslop

Ten structural simplifications. Each removes code without changing behavior.

#### C24: Generate the tuple `FromRow` impls

`from_row.rs:244-441` unrolls 16 separate macro invocations whose argument
lists grow by one line each. Replace with one recursive macro that emits all
arities 1 through 16, and a single runtime index in the expansion:

```rust
fn from_row(_prefix: &str, row: &'r Row) -> Result<Self> {
    let mut idx = 0usize;
    Ok(($({ let v = row.get_value_idx(idx)?; idx += 1; v },)+))
}
```

Tuple operands evaluate left to right. This removes roughly 300 lines.

#### C25: Generate the integer type impls

`types/int.rs` and `types/uint.rs` repeat the same `Encode`/`Decode` bodies for
each integer width, and `types/vec.rs:36-102` repeats the blob encode pair for
`VecF32`/`VecInt8`/`VecBit`. Add file-local `macro_rules!` helpers in the
style of the existing `compatible!` macro. Keep the distinct `u64`/`usize`
range checks and the `i8`/`i16` narrowing decode.

#### C26: Collapse the duplicated `QueryExecutor` impls

`query.rs:255-278` (`impl QueryExecutor for &T`) and `query.rs:280-303`
(`for &mut T`) have identical bodies apart from the `Send`/`Sync` bound on
`T`. Emit both from one private macro, or define a private method on the
sealed trait and have both impls delegate. The `&Pool` impl stays separate.

#### C27: Deduplicate the macro crate entry points and generics handling

`musq-macros/src/lib.rs:29-75` repeats parse / expand / convert-to-compile-
error five times. Add one `fn derive(input, expand_fn)` helper. While there,
add the shared fresh-lifetime helper from C2 in `core.rs` and use it from
`encode`, `decode`, `json`, and `row`. Also standardize the
attribute/doc-comment ordering on the proc-macro functions
(`lib.rs:25-28,80-89`).

#### C28: Consolidate placeholder handling in `sql.rs`

`sql.rs:193-223` has four structurally identical arms for `values`, `insert`,
`set`, and `idents`. Collapse them into one match arm that parses the
expression, runs `ensure_non_empty`, and pushes the segment. In the same
pass, map `syn::parse_str` failures to the format-literal span so diagnostics
point at the string (currently 269-274 and 305-316 use `Span::call_site()`),
and extend `ensure_non_empty` to unwrap `Expr::Paren`/`Expr::Group`.

#### C29: Consolidate `FromRow` expansion in `row.rs`

`row.rs` computes each field's column name twice (reads at 63-68 and null
checks at 133-138) and repeats the same lifetime-insertion block in the named
and tuple paths (39-54 and 215-231). Extract a column-name helper and a
generics helper from C27/C2. Also decode `try_from` fields once: current code
at 91-93 calls `row.get_value` for the raw `Value` and again for the decoded
type; decode the raw value once and run `Decode` on it, mapping failures to
the same `ColumnDecode` error. Fully qualify generated `format!`,
`Default::default`, `String::new`, and `Ok` (`row.rs:91-100,108,113-118`;
`decode.rs:148`).

#### C30: Replace `BoxFuture` return wrappers with `async fn`

`Transaction::begin`/`begin_with` (`transaction.rs:84-107`),
`Connection::begin`/`begin_with` (`connection/mod.rs:162-181`), and
`Connection::prepare` (`connection/mod.rs:539-541`) hand-write boxed futures
only because the crate predates async fn in traits. Edition 2024 supports
plain `async fn`. Converting these removes `BoxFuture` from the public
`transaction` path and the `futures_core::future::BoxFuture` import.

#### C31: Remove `PoolConnection` delegation and a dead result

`pool/connection.rs:102-115` defines `interrupt` and `interrupt_handle` that
only forward through `Deref`; callers can use the `Connection` methods
directly. `Floating::return_to_pool` (`pool/connection.rs:254-262`) returns a
`bool` that the only caller (`pool/connection.rs:147-163`) discards; change
it to return `()` and delete the discard. Also delete the commented-out
queue-length note at `pool/mod.rs:291-293` if `num_idle` no longer needs it.

#### C32: Deduplicate `Query`/`Map` bind methods

`query.rs:353-383` and `query.rs:431-467` implement the same
`try_bind`/`bind`/`try_bind_named`/`bind_named` shape twice, once for `Query`
and once for `Map`. A private macro or a small private trait with the
`arguments` accessor as the only difference removes the copy.

#### C33: Delete print-only and duplicated macro unit tests

`encode.rs:127-152`, `decode.rs:176-211`, `row.rs:274-307`, and
`json.rs:44-56` contain inline test modules that largely duplicate
`musq-macros/src/tests/*`; several only `println!` the expansion and assert
nothing. Keep the unique cases in `src/tests`, give them real assertions
(`assert!` on tokens), and delete the inline copies. `tests/encode.rs` and
`tests/decode.rs` are near-identical 44-line suites and can share one
parameterized macro.

### F. Test Suite

The suite has broad coverage but also duplication, timing dependence, and
gaps in the public API. Findings were collected file by file.

#### C34: Remove exact duplicates and compile-only tests

- `tests/sqlite.rs:56-80`: two identical `fetch_one` blocks; delete the
  second.
- `tests/sqlite.rs:503-536`: `it_caches_statements` runs the same loop twice
  and its comment describes `.persistent(false)`, which no longer exists.
- `tests/transaction_signatures.rs:9-11`: two functions with identical
  signatures; delete one.
- `tests/executor.rs:9-30` and `:33-55`: `setup_test_db`/`setup_test_pool`
  are identical apart from the return type; merge.
- `tests/sqlite_capabilities.rs` duplicates the runtime-identity assertions
  in `tests/sqlite_connection_control.rs:16-39`; keep one home.
- `tests/vec.rs:14-22,24-33`: direct versus pooled `vec_version` tests differ
  only in the executor; run both cases through a loop.
- `tests/sql_macros.rs:262-274`: `create_simple_query` and friends execute
  nothing and their results are dropped; delete or document as a
  compile-only matrix.
- `tests/concurrent.rs:230-232`: `_test_query` is constructed and never
  used, so the test's stated "arguments are not consumed" purpose is not
  exercised.

#### C35: De-flake and speed up timing-dependent tests

- `tests/sqlite.rs:765-785`: `concurrent_resets_dont_segfault` spawns 1000
  inserts, never awaits the task, and asserts nothing.
- `tests/sqlite.rs:807`: a one-second sleep waits for the worker to die; use
  the join handle or a channel.
- `tests/sqlite.rs:818-821`: `issue_1467` defaults to 10,000 transactions
  despite a "keep this test fast" comment; reduce and gate the stress count
  behind an env var, then assert final row counts.
- `tests/sqlite.rs:189-194`: `it_fetches_in_loop` opens 1000 connections.
- `tests/concurrent.rs:184,201`: 1-2 ms sleeps make reader/writer ordering
  timing-dependent.
- `tests/concurrent.rs:309-341`: `spawn_blocking` plus
  `Handle::current().block_on` can deadlock or panic under a current-thread
  runtime; use `tokio::spawn`.
- `tests/concurrent.rs:86,208` and `tests/pool_executor.rs:179`:
  `join_all(handles).await` discards join results, so task panics are
  swallowed; unwrap or collect the results.
- `tests/connection_flows.rs:44-55`: asserts elapsed wall time;
  `tests/interrupt.rs` uses 20 ms races plus a 1000-iteration race loop;
  `tests/pool_close.rs` polls `stats()` every 10 ms; `tests/hooks.rs:24,60,68`
  uses one-second receives. Replace sleeps with notifications where possible
  and guard the remaining race stress behind a stress flag.
- `tests/row_materialization_alloc.rs:144-164`: a `<= 2x` global allocator
  heuristic is fragile; loosen or record a baseline.
- `tests/renamed_dependency.rs:14-26`: runs `cargo check` inside every test
  run; move to CI or gate it.

#### C36: Add coverage for untested public API

The following public items have no integration coverage today (from
`api/musq.rs`): `Arguments::add`/`add_named` and the `query_with`,
`query_as_with`, `query_scalar_with` constructors; `Pool::try_begin`,
`Pool::try_acquire`'s success path, and `Pool::close_event`;
`Connection::transaction` and `Connection::dropped_hook_events`;
`PoolConnection::close`; `QueryBuilder::push_bind`, `push_set`, `push_where`,
`push_query`, and `try_push_query`; `Query::map`/`Map::map`/`Map::try_map`;
`Value::{int,double,blob,text,type_info,is_null}`; `Text` methods;
`SqliteError::{codes,is_busy,is_unique_violation}` directly; `Conditions`,
`Expr`, `expr::raw`, and `expr::jsonb_serde`; `UpdateOp::{Update,Delete}`;
`WalCheckpointMode` values other than `Noop`; `DeserializeMode::ReadOnly`
success; and most `Musq` builder options (`defensive`, `optimize_on_close`,
`thread_name`, `analysis_limit`, `read_only`, `vfs`, `immutable`, `pragma`,
`acquire_timeout`, `command_buffer_size`, `row_buffer_size`, and the pragma
setters). Add table-driven tests where one shape covers many options.

#### C37: Strengthen weak assertions

- `tests/error.rs:98-148`: foreign-key, NOT NULL, and CHECK tests only
  assert `message.contains("constraint")`; assert the primary and extended
  codes for each.
- `tests/sqlite_connection_control.rs:172-174`: `>= 0` on an `Option<i32>` is
  a tautology; assert non-zero frame counts after writes.
- `tests/snapshot.rs:26-27`: `image.len() >= empty_image.len()` proves
  little; deserialize and assert contents.
- `tests/functions.rs:71-87`: the innocuous-function index test only checks
  DDL success; query through the index.
- `tests/vacuum_into.rs:81-92`: destination-failure test only checks
  `Error::Sqlite(_)`; assert the SQLite message/code.
- `tests/executor.rs:72,226`: `last_insert_rowid() > 0`; assert the exact
  rowid.
- `tests/concurrent.rs:49-51,142-145,257-261,298-302,349-353`: `assert!(r
  .is_ok())` followed by `r.unwrap()`; drop the first line.
- `tests/vec.rs:151`: `assert_ne!(plain_type, "int8")`; assert the actual
  subtype.
- `crates/musq/src/types/time.rs:301-318` and `:322-356`: tests accept both
  `Ok` and `Err` and print instead of asserting, with stale "bug" comments.
  Fix the expected values and rename the tests.

#### C38: Consolidate test helpers and fixtures

- `tests/support/db.rs` is not declared by `tests/support/mod.rs`, forcing
  `#[path = "support/db.rs"] mod support_db;` in `sqlite.rs:4-5` and
  `error.rs:4-5`; declare `pub mod db;` once.
- About 20 test files build `Connection::connect_with(&Musq::new())` inline;
  reuse `support::connection()`.
- `snapshot.rs:8-17`, `transaction_behavior.rs:14-32`, and
  `vacuum_into.rs:118-127` rebuild near-identical schemas; share one
  fixture builder.
- `sqlite_connection_control.rs:250-258`'s
  `assert_configuration_contains` is reimplemented inline in
  `in_memory_settings.rs:28-31` and `snapshot.rs:39,70,97,145`; promote it to
  `support`.

#### C39: Fix test-level dead allowances and weak helpers

- `tests/concurrent.rs:230-232`: remove `_test_query` (also in C34) and make
  the test exercise a shared `Arguments` value.
- `tests/logger.rs:1`: narrow the crate-level `dead_code` allow (C19).
- `tests/pool_executor.rs:105-149`: `test_pool_in_generic_function` takes a
  concrete `&Pool` rather than a generic `E: QueryExecutor`; make it
  actually generic.
- `tests/pool_close.rs`: exercise `Pool::size`/`num_idle`/`is_closed`
  accessors directly, not only via `PoolStats`.

### G. Examples, Benches, Stress Test

#### C40: Fix the benchmark harness

`benches/benchmark.rs:42-55` creates a pool and seeds a row inside the timed
routine on every iteration, so writes and reads largely measure connection
setup, and `CONCURRENCY=20` with `max_connections(5)` (lines 13-16,28-33)
forces connection opens inside the measurement. `BatchSize::SmallInput`
grows the batch and holds every setup pool until timing finishes. Hoist the
pool and fixture out of the timed routine, vary dataset size as an input, and
use a fixed batch count. Remove the empty untracked `benches/sqlite/`
directory.

#### C41: Fix the stress test's rowid assumption and error reporting

`crates/stresstest/src/main.rs:253-254,306-331` picks random reads from
`1..=max_id` where `max_id` counts only this run's inserts, so a reused
`--database` reads the wrong range; the sanity check at 369-379 always warns
on a reused database. Capture inserted ids (for example with `RETURNING id`)
or document that the database must be empty. Include the error text in the
insert/read failure prints at 318-328, which currently discard it.

#### C42: Make the README snippets execute

`crates/musq/examples/readme_snippets.rs:258-260` only prints; the `sql`,
`values`, and `transactions` functions query `users`/`events` without
creating them, so they would fail if called. Wire `main` (or a test) to set
up the schema and run the functions, or delete the markers. This is the only
behavioral check that the README SQL matches the code.

## Decisions Needed

Three items need an owner decision and are excluded from the clear-win
count.

- D1: Add CI. There is no CI configuration; the `.github` directory was
  removed in `7070a81`. A workflow running the gate
  (`clippy --fix --tests --examples --benches`, `fmt --check`, `ncode test`,
  `--no-default-features`, `cargo doc --no-deps`, `cargo package --list`)
  would prevent most drift found here. Decide the runner and required
  jobs.
- D2: Clippy line threshold. `clippy.toml:2` sets
  `too-many-lines-threshold = 800`, which effectively disables the workspace
  `too_many_lines` lint. Either lower it to a meaningful value (for example
  the clippy default 100) or drop the lint.
- D3: Retire `plans/improve.md`. All 51 items are checked, and commit
  `29fdc29` removed completed plans, but this file remains tracked. Archive
  or delete it once this sweep is executed.

## Execution Plan

Every change in this document appears below exactly once. Stages are in
dependency order. Each stage ends with the repository gate:

1. `cargo clippy --fix --allow-dirty --tests --examples --benches` with no
   warnings after review.
2. `cargo fmt`.
3. `cargo test`.
4. `cargo test -p musq --no-default-features`.
5. `cargo doc --no-deps` with no warnings.
6. `git diff --check`.

### Stage 1: Correctness

- [x] C1. Reorder `WorkerSession` so `ClearDbOnDrop` runs before
  `ConnectionState` drops; add the pointer-cleared unit test.
- [x] C2. Add the fresh-lifetime helper in `core.rs`; use it in `json.rs`,
  `decode.rs`, and `row.rs`; add generic and user-lifetime trybuild pass
  cases.
- [x] C3. Replace `Ident::new` at `sql.rs:283` with a fallible parse and a
  format-literal-span error; add a fail trybuild case.
- [x] C4. Exclude `Token![==]` from the named-argument peek at `sql.rs:55`;
  add a pass trybuild case.
- [x] C5. Apply `rename_all` only when `rename` is absent; default
  `RowContainer` to `Verbatim`; correct `from_row.rs` docs; add unit and
  integration tests for explicit rename, `rename_all`, and `r#type`.
- [x] C6. Match `libsqlite3-sys`'s `!= "0"` off-value in `build.rs`, or
  document and narrow the other three checks.
- [x] Stage gate.

### Stage 2: Documentation And Cleanup

- [x] C7. Delete the duplicated backup paragraph at
  `connection/mod.rs:225-230`.
- [x] C8. Fix the `Pool::acquire` and `Pool::num_idle` docs.
- [x] C9. Replace the `query!()` references in `query.rs`.
- [x] C10. Rewrite the stale SQLx/editor/dead-type comments in
  `pool/connection.rs`, `musq.rs`, `connection/handle.rs`, and
  `tests/sqlite.rs`.
- [x] C11. Fix the `FromRow` doc typos and attribute list.
- [x] C12. Correct the README introspection bullets.
- [x] C13. Delete the unused `DbConfigIntOp` variants and allows.
- [x] C14. Delete the dead macro-crate items listed in C14, including
  `check_repr_enum_attrs`, the skipped darling fields, and the stale
  decode-test allow.
- [x] C15. Remove the redundant drops and the unreachable `push_upsert`
  branch.
- [x] C16. Remove the stale Cargo `exclude` and both `wip/` directories.
- [x] C17. Delete `crates/xtask` and the `.cargo/config.toml` alias.
- [x] C18. Apply the `statement_cache` and `function` hardening fixes.
- [x] C19. Apply the visibility and inert-allow cleanups.
- [x] Stage gate.

### Stage 3: Dependencies, Packaging, And API

- [x] C20. Replace `tempdir` with `tempfile` in all six test files.
- [x] C21. Drop the `futures-util` `sink` and `io` features; verify with
  `--all-features`.
- [x] C22. Add the workspace `musq` version, add `license-file` to both
  crates, and record the version-bump step for the next release.
- [x] C23. Track `api/` in git and regenerate it after C5 lands; verify with
  `ncode api --check`.
- [x] Stage gate, plus `cargo package -p musq --list` includes `LICENSE`
  and `cargo package -p musq-macros --list` succeeds.

### Stage 4: Deslop

- [x] C24. Replace the 16 tuple `FromRow` invocations with one macro; run the
  full test suite to confirm tuple arities still decode.
- [x] C25. Add integer and vector impl macros in `types/`.
- [x] C26. Collapse the `QueryExecutor` `&T`/`&mut T` impls.
- [x] C27. Add the macro-crate derive helper and fresh-lifetime helper;
  normalize attribute/doc ordering.
- [x] C28. Consolidate the `sql.rs` list arms, span mapping, and empty-list
  recursion.
- [x] C29. Consolidate `row.rs` column-name/lifetime helpers, decode
  `try_from` once, and qualify generated paths.
- [x] C30. Convert `Transaction::begin`/`begin_with`,
  `Connection::begin`/`begin_with`, and `Connection::prepare` to `async fn`.
- [x] C31. Remove the `PoolConnection` delegating methods and the discarded
  `return_to_pool` bool.
- [x] C32. Deduplicate the `Query`/`Map` bind methods.
- [x] C33. Delete the print-only inline macro tests and consolidate
  `tests/encode.rs` with `tests/decode.rs`.
- [x] Stage gate, plus regenerate `api/` and confirm no public-path changes
  beyond the intended ones.

### Stage 5: Test Suite

- [x] C34. Remove the exact duplicates and compile-only tests listed in C34.
- [x] C35. De-flake and shrink the timing-dependent tests; gate the remaining
  stress loops behind a flag; check task results instead of discarding them.
- [x] C36. Add the missing public-API coverage, prioritizing
  `query_*_with`, `Arguments`, `Pool::try_*`/`close_event`,
  `Connection::transaction`, `QueryBuilder::push_*`, `Value` accessors, and
  the `Musq` options.
- [x] C37. Strengthen the weak assertions listed in C37, including the
  `types/time.rs` tests.
- [x] C38. Declare `support/db.rs`, share connection/schema helpers, and
  promote `assert_configuration_contains`.
- [x] C39. Fix the remaining test-level allowances and the
  non-generic generic helper.
- [x] Stage gate; record the new test count and any tests moved behind a
  stress flag.

### Stage 6: Tooling And Final Decisions

- [x] C40. Rework the benchmark setup and remove `benches/sqlite/`.
- [x] C41. Fix the stress-test rowid assumption and error reporting.
- [x] C42. Make the README snippets execute.
- [x] D1. Decide and, if approved, add the CI workflow.
- [x] D2. Decide the `too-many-lines-threshold` policy.
- [x] D3. Archive or delete `plans/improve.md`.
- [x] Stage gate, plus a full release-style dry run
  (`cargo package` for both crates) if C22 or D1 landed.
