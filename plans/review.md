# Musq Correctness And API Review Plan

This review covers the current workspace, with emphasis on correctness, shutdown
stability, macro/API behavior, and low-hanging features. Existing `plans/modern.md`
was used as context only and left untouched.

Review commands already run:

- `cargo test` passed.
- `cargo clippy --tests --examples --benches` passed.
- `cargo fmt --check` passed.
- `cargo test -p musq --no-default-features` passed.
- `cargo doc --no-deps` built, but emitted rustdoc warnings that should be fixed.

Implementation validation completed:

- `cargo test -p musq-macros` passed.
- `cargo test` passed.
- `cargo test -p musq --no-default-features` passed.
- `cargo clippy --fix --allow-dirty --tests --examples --benches` passed.
- `cargo fmt` passed.
- `cargo doc --no-deps` passed with no warnings.
- `cargo xtask test` passed: 321 tests run, 321 passed.

1. Stage One: Pool Shutdown Correctness

Fix the highest-risk lifecycle issue first: mixed idle and checked-out pools can let
`Pool::close()` finish early because idle close uses a guard path intended for live
checked-out connections. Keep the pool accounting invariant explicit while changing
this code: `size` counts all open connections, `num_idle` mirrors the idle queue,
and checked-out live connections own a forgotten semaphore permit until they return
or close. Idle connections have already returned their permit.

1. [x] Add a regression test in `crates/musq/tests/pool_close.rs` with
   `max_connections(2)`: acquire two connections, drop one to idle, start
   `Pool::close()`, assert close does not complete while the other connection is
   still checked out, then drop it and assert close completes.
2. [x] Add the diagnostics needed for these tests before asserting accounting:
   either promote `Pool::size()` and `Pool::num_idle()` to public read-only
   diagnostics, add a `Pool::stats()` return type, or add a narrow test-only hook.
3. [x] Repair the idle-close path in `crates/musq/src/pool/inner.rs` and
   `crates/musq/src/pool/connection.rs` so closing an idle connection decrements
   both `size` and `num_idle` without adding an extra semaphore permit. Add a
   dedicated idle teardown path that closes the raw connection and forbid
   `Live::float()` on idle-queue connections.
4. [x] Audit all pool permit and size transitions with a done-when matrix for
   `(size, num_idle, semaphore permits)`: new connection, checked-out reattach,
   return to idle, close while checked out, close while idle, acquire cancellation,
   release racing with close, and drop without a Tokio runtime.
5. [x] Rework the close loop so it re-drains idle connections after its final
   permit acquisition and only returns once `size() == 0`, covering a connection
   that observes the pool as open just before `mark_closed()` and races back into
   the idle queue.
6. [x] Add tests for close races with multiple idle connections and one held
   connection, for `num_idle` staying consistent through close, and for
   `Pool::try_acquire()` returning `None` after close without disturbing size
   accounting.
7. [x] Clean up the stale `min_connections` comment in
   `crates/musq/src/pool/connection.rs` while touching the drop/return path.

2. Stage Two: Macro Compile Correctness

Fix proc-macro cases where the current tests inspect token strings but do not prove
the generated code compiles in real consumer code.

1. [x] Add trybuild pass fixtures for tuple-struct `#[derive(FromRow)]`, including
   plain tuple structs, generic tuple structs, and tuple structs with lifetimes.
   Compile these fixtures with `#![deny(warnings)]` so generated unused-variable
   warnings are caught.
2. [x] Remove the generated `R: musq::Row` generic from tuple `FromRow` derives in
   `crates/musq-macros/src/row.rs`. `musq::Row` is a concrete struct, not a trait,
   so this generated bound is invalid for real compilation. Also use or rename the
   generated `prefix` parameter in tuple `from_row()` so consumer crates with
   denied warnings do not fail after the compile bug is fixed.
3. [x] Update `crates/musq-macros/src/tests/from_row.rs` to assert the corrected
   expansion shape instead of preserving the invalid `R: musq::Row` bound, and
   assert the tuple pass fixtures compile.
4. [x] Add trybuild pass fixtures for named `FromRow` derives with
   `#[musq(rename_all = ...)]`, `#[musq(default)]`, `#[musq(skip)]`, and
   `#[musq(deserialize_with = ...)]` in the same file so named-struct attribute
   interaction is covered by compile tests, not only expansion-string tests.
5. [x] Define tuple-field attribute policy explicitly. Either implement the
   meaningful positional subset, such as `skip`, `default`, and
   `deserialize_with`, or reject unsupported tuple-field attributes in
   `check_row_attrs()` with trybuild fail fixtures and reviewed `.stderr`
   snapshots. Do not add tuple attribute pass fixtures that compile while the
   attributes are ignored.

3. Stage Three: Query Composition And Bind Safety

Make composed queries correct for all supported SQLite bind syntaxes, or reject
unsupported composition explicitly before execution.

Decision: numeric placeholders are rejected in appended fragments for now. Standalone
queries still support SQLite numeric placeholders, while composed fragments support
anonymous `?` and named parameters.

1. [x] Add failing tests in `crates/musq/tests/query_join.rs` for joining two
   fragments that both contain numeric parameters like `?1`, and for reversed
   numeric references such as `SELECT ?2, ?1`. Include a fragment that mixes
   numeric placeholders and anonymous `?` placeholders.
2. [x] Add failing tests in `crates/musq/tests/dynamic_values.rs` for `Values`
   expressions built from `Query` fragments containing numeric parameters.
3. [x] Choose the Stage Three contract before implementation: either reject
   numeric placeholders in appended fragments before execution, or fully support
   them. If supporting them, parse `?NNN` and numeric `$NNN` outside
   strings/comments with 1-based indexing, preserve repeated and reversed numeric
   references, and keep later anonymous `?` placeholders independent of the
   named-parameter `anon_pos` heuristic by rebasing them as needed.
4. [x] Move any fallible composition API into this stage. Add
   `Query::try_join()` and `QueryBuilder::try_push_query()` if composition can
   fail, and keep `join()` as a convenience wrapper only if its panic/error
   behavior is documented.
5. [x] Keep the existing named-parameter renaming tests, then add mixed named,
   anonymous, and numeric-parameter cases across multi-statement queries. For
   supported cases, assert both final SQL and successful execution with expected
   row values; for rejected cases, assert the error.
6. [x] Normalize or reject names in `QueryBuilder::push_bind_named()`. Today the
   argument map trims prefixes but SQL rendering always prepends `:`, so callers
   passing `:name` produce `::name`. Normalize the name once before both argument
   insertion and SQL rendering, then add a focused `query_builder.rs` test.
7. [x] Add public docs that spell out the supported placeholder forms for
   `Query::join()`, `Expr::from(Query)`, and `QueryBuilder::push_query()`.

4. Stage Four: Proc-Macro Crate Path Stability

Make derives and SQL macros work when downstream users rename the `musq` dependency
or re-export it through a facade crate.

Decision: dependency renaming is supported through automatic crate-path resolution and
a renamed-dependency fixture. A manual `#[musq(crate = ...)]` facade override was
considered but not added because no current caller requires it.

1. [x] Add proc-macro path resolution in `musq-macros`, using
   `proc-macro-crate` or an equivalent resolver through a small shared helper
   instead of hard-coded `musq::...` paths in `row.rs`, `encode.rs`, `decode.rs`,
   `json.rs`, and `sql.rs`. Add the new dependency explicitly and route emitted
   paths through the helper, preferably as absolute paths so a local `mod musq`
   cannot shadow the crate.
2. [x] Add tests for generated derives and `sql!`/`sql_as!` using the resolved
   crate path. If direct trybuild coverage cannot hide the normal `musq` crate
   name, add a tiny fixture crate under test data with `musq` renamed.
3. [x] Consider a `#[musq(crate = path::to::musq)]` override for derives if facade
   support is important. Keep the default automatic path as the simple case.
4. [x] Update macro docs to state that dependency renaming is supported once the
   new path resolver is in place.

5. Stage Five: Configuration And Optional Public API Polish

Address small API traps first, then only take the polish items that have a clear
caller or documentation payoff. Stages One through Four should not be blocked on
these optional ergonomics.

Decision: zero-sized command and row buffers are documented as supported rendezvous
channels. `Values::try_from_iter()` was considered and not added because the existing
`Values::new()` plus fallible `insert()` loop already covers dynamic construction
without a concrete caller showing additional need.

1. [x] Validate `Musq::max_connections(0)` during pool creation and return an
   immediate configuration error instead of timing out while trying to build the
   initial connection.
2. [x] Decide whether `command_buffer_size(0)` and `row_buffer_size(0)` are
   supported rendezvous modes. If yes, document them; if no, validate and reject.
3. [x] If Stage One did not already expose pool diagnostics, decide here between
   public `Pool::size()`/`Pool::num_idle()` and a single `Pool::stats()` return
   type. Do not expose `num_idle` until close-time accounting is fixed and tested.
4. [x] Optionally add `Row::len()`, `Row::column_names()`, and
   `Row::contains_column()` so callers do not need to probe by attempting a
   decode.
5. [x] Align `Prepared` documentation with reality: either expose column and
   parameter metadata, or remove wording that says prepared statements expose
   expected columns and parameter types.
6. [x] Consider `Values::try_from_iter()` only if a concrete caller shows the
   current `Values::new()` plus fallible `insert()` loop is meaningfully awkward.
   The existing API already supports dynamic key/value construction in a short
   loop.

6. Stage Six: Docs, Benchmarks, And Developer Tools

Clean up low-hanging stability and maintenance issues outside the main runtime.

1. [x] Fix all rustdoc warnings reported by `cargo doc --no-deps` until the command
   is clean. Current starting points include broken links in
   `sqlite/connection/mod.rs`, `pool/mod.rs`, `pool/connection.rs`,
   `transaction.rs`, `query.rs`, `error.rs`, `expr.rs`, and `musq.rs`.
2. [x] Fix or mark as text the invalid ignored code block in
   `crates/musq/src/from_row.rs` where prose appears inside a Rust code block.
3. [x] Update the manual `FromRow` documentation to use the current
   `fn from_row(prefix: &str, row: &Row)` signature.
4. [x] Fix `benches/benchmark.rs::setup()` so it returns the seeded pool. It
   currently inserts into `p` and then sends a fresh `pool().await`, so the read
   benchmark can silently benchmark failed `fetch_one()` results.
5. [x] Make benchmark futures fail loudly by asserting every `join_all()` result
   is `Ok`, rather than discarding query errors.
6. [x] Remove or repair stale nested benchmark code under `benches/sqlite/` if it
   is no longer part of the current Musq workspace.
7. [x] Harden `stresstest` argument handling: reject `--bins 0`, handle
   `records < bins`, and avoid `chunks(0)` in `TimingData::process()`.

7. Stage Seven: Validation And Closeout

Use the repository gates after each implementation stage, with extra coverage for
the feature and documentation surfaces touched by the review.

1. [x] Run the focused tests for the stage being changed first, such as
   `cargo test -p musq --test pool_close` or
   `cargo test -p musq --test query_join`.
2. [x] Run `cargo test -p musq-macros` after Stage Two or Stage Four changes.
3. [x] When adding trybuild fail fixtures, review and commit the generated
   `.stderr` snapshots intentionally before treating the next `cargo test`
   failure as a regression.
4. [x] Run `cargo test`.
5. [x] Run `cargo test -p musq --no-default-features`.
6. [x] Run `cargo xtask test` when nextest parity is useful for broader behavior
   changes; still run `cargo test` before any commit as required by `AGENTS.md`.
7. [x] Run `cargo clippy --fix --allow-dirty --tests --examples --benches`, then
   inspect and keep only intended edits.
8. [x] Run `cargo fmt`.
9. [x] Run `cargo doc --no-deps` and confirm rustdoc warnings are either fixed or
   intentionally documented.
10. [x] Run `git diff --check` and inspect `git status --short` before any commit.
