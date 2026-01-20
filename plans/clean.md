# Musq Code Review – Correctness & Efficiency Findings

Ordered by importance (highest first). Each item includes evidence and a suggested direction for fixes.

1. Incorrect `rows_affected`/logging for read-only statements
   - Evidence: `crates/musq/src/sqlite/connection/execute.rs:113-121` uses `statement.handle.changes()` for every statement, even SELECT/PRAGMA.
   - Impact: `QueryResult.rows_affected()` and query logs can report stale counts from a prior DML statement, and multi-statement `execute()` can double-count changes.
   - Fix: gate `sqlite3_changes` behind a read-only check (e.g., `sqlite3_stmt_readonly`) or compute changes only for non-row-returning statements; otherwise set changes to 0.

2. Nested transaction rollback leaves savepoints active (depth desync)
   - Evidence: `crates/musq/src/transaction.rs:141-147` builds `ROLLBACK TO SAVEPOINT` for nested rollback; worker decrements depth immediately (`crates/musq/src/sqlite/connection/worker.rs:240-245`).
   - Impact: SQLite keeps the savepoint after `ROLLBACK TO`, but `transaction_depth` is decremented. This can desynchronize state and cause incorrect behavior for later commits/rollbacks or reusing the same savepoint name.
   - Fix: after `ROLLBACK TO`, issue `RELEASE SAVEPOINT` for that savepoint (or use the standard two-step pattern) before decrementing depth.

3. TEXT decoding bypasses SQLite UTF-8 conversion and uses lossy decoding
   - Evidence: `crates/musq/src/row.rs:70-84` reads TEXT via `sqlite3_column_blob` + `from_utf8_lossy`.
   - Impact: For UTF-16 databases or non-UTF8 text, bytes are interpreted incorrectly and silently replaced, corrupting data. This also ignores SQLite’s guaranteed UTF‑8 conversion via `sqlite3_column_text`.
   - Fix: use `sqlite3_column_text` (UTF‑8 conversion) and handle invalid UTF‑8 as an error (or expose raw bytes via `BString`/`Vec<u8>` explicitly).

4. NULL values decode to default values for non-`Option` types
   - Evidence: `crates/musq/src/sqlite/value.rs:61-116` returns `0`, `0.0`, `""`, or `&[]` on `Value::Null`.
   - Impact: NULLs silently become default values for `i32`, `bool`, `String`, `Vec<u8>`, etc., masking data issues and causing incorrect application logic.
   - Fix: return `DecodeError::Conversion` on NULL for non-`Option<T>` decodes (keep `Option<T>` path as the NULL-safe route).

5. Named-parameter collisions when concatenating queries
   - Evidence: `crates/musq/src/query_builder.rs:205-217` rebases and inserts `other_args.named` into `self.arguments.named` without collision handling.
   - Impact: When two appended queries use the same named parameter, the later mapping overwrites the earlier one. Earlier placeholders can bind the wrong value, especially across compound statements.
   - Fix: detect name collisions and either error, or rewrite SQL to disambiguate names (e.g., `:name__1`, `:name__2`) while remapping indices.

6. `$0` / numeric `$NNN` parameters can underflow
   - Evidence: `crates/musq/src/sqlite/arguments.rs:105-110` accepts numeric `$NNN` without validating `n >= 1`; binding uses `self.values[n - 1]` at `:165`.
   - Impact: `$0` (or `$000`) yields index 0 and underflows to `usize::MAX`, causing panic or incorrect binding.
   - Fix: apply the same validation as `parse_question_param` (reject 0 and leading zeros) for the numeric `$NNN` path.

7. `PoolConnection` drop can panic outside a Tokio runtime
   - Evidence: `crates/musq/src/pool/connection.rs:142-148` unconditionally calls `tokio::spawn` in `Drop`.
   - Impact: Dropping a `PoolConnection` without an active Tokio runtime (or after runtime shutdown) will panic, potentially crashing the process.
   - Fix: use `tokio::runtime::Handle::try_current()` and fall back to a synchronous return/close path when no runtime is present.

8. Extra cloning of arguments on every execution (performance)
   - Evidence: `crates/musq/src/query.rs:392-394` returns `self.arguments.clone()` even though `Query` is consumed by `execute`/`fetch` paths.
   - Impact: Large blobs/strings are duplicated per execution, adding allocations and copies.
   - Fix: allow moving arguments out of `Query` (e.g., change `Execute::arguments` for `Query` to take ownership, or store `Arguments` behind `Arc`/`Cow`).

9. Eager row materialization forces allocations for all TEXT/BLOB values (performance)
   - Evidence: `crates/musq/src/row.rs:41-105` eagerly allocates `String`/`Vec<u8>` for every column in every row.
   - Impact: For large result sets, this increases memory churn and CPU even when callers only read a subset of columns or decode as references.
   - Fix: consider lazy decoding or using shared buffers (`Arc<[u8]>`, `Bytes`) to reduce copies; alternatively provide a streaming/zero-copy row type for advanced use cases.
