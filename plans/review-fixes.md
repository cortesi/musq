# Review Fixes Checklist

Checklist to address the correctness/efficiency findings from plans/clean.md, including tests and
fixes per item.

1. Stage One: Rows Affected for Read-only Statements

Test and fix `rows_affected` reporting for read-only statements in
`crates/musq/src/sqlite/connection/execute.rs`.

1. [x] Add a regression test that shows `rows_affected` is stale for a SELECT after a prior DML.
2. [x] Implement a read-only check (e.g., `sqlite3_stmt_readonly`) to report 0 changes for reads.
3. [x] Update or add logging assertions if needed; run tests for the new case.

2. Stage Two: Nested Transaction Rollback Savepoint Semantics

Ensure nested rollback releases savepoints and keeps `transaction_depth` consistent.

1. [x] Add a test that begins a nested transaction, rolls back inner, then commits outer.
2. [x] Implement rollback-to-savepoint + release savepoint in worker transaction handling.

3. Stage Three: TEXT Decoding and UTF-8 Handling

Use SQLite's UTF-8 conversion and handle invalid UTF-8 explicitly.

1. [x] Add a test for UTF-16/invalid UTF-8 TEXT decoding behavior.
2. [x] Replace `column_blob` + `from_utf8_lossy` with `column_text` and strict UTF-8 checks.

4. Stage Four: NULL Decoding for Non-Option Types

Make NULL decoding return an error for non-Option decoders.

1. [x] Add tests that decoding NULL into `i32`, `String`, etc. yields `DecodeError`.
2. [x] Update `Value::{int,int64,double,blob,text}` NULL handling to error (except Option path).

5. Stage Five: Named Parameter Collisions in Query Join

Detect or disambiguate named parameter collisions when joining queries.

1. [x] Add a test that joins two queries using the same named parameter and fails today.
2. [x] Implement collision detection or name rewriting in `QueryBuilder::push_query`.

6. Stage Six: Numeric $NNN Parameter Validation

Reject `$0` (and leading-zero forms) to avoid underflow in binding.

1. [x] Add a test for `$0`/`$00` parameter parsing in `Arguments::bind`.
2. [x] Add validation to treat `$0` as protocol error like `?0`.

7. Stage Seven: PoolConnection Drop Without Runtime

Avoid panicking when dropping a `PoolConnection` outside a Tokio runtime.

1. [x] Add a test that drops a `PoolConnection` without a runtime and asserts no panic.
2. [x] Use `tokio::runtime::Handle::try_current()` and a fallback close/return path.

8. Stage Eight: Avoid Cloning Arguments on Execute

Reduce unnecessary argument cloning during execution.

1. [ ] Add a micro-benchmark or test to demonstrate cloning of large args.
2. [ ] Rework `Execute::arguments` for `Query` to move/borrow args without cloning.

9. Stage Nine: Row Materialization Costs

Investigate deferred decoding or shared buffers for large result sets.

1. [ ] Add a benchmark or targeted test to show allocation overhead.
2. [ ] Prototype a lazy/zero-copy row or use shared buffers for TEXT/BLOB.
