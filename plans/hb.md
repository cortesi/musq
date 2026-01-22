# HB db crate: making `musq::Values` fully expressive

Goal: remove the main friction points in using `musq::Values` end-to-end from the hb `db` crate,
especially for JSONB, timestamps, and other non-literal SQL expressions, while keeping the safety
and ergonomics of the `sql!` + `{set:...}/{insert:...}/{where:...}` placeholders.

This plan is written from the perspective of:
- `hb` consuming `musq` as a path dependency at `hb/crates/db -> musq/crates/musq`
- `hb/crates/db` currently storing JSONB (SQLite `jsonb(...)`) and ISO 8601 timestamps as TEXT

## Decisions and constraints (integrated)

Decisions:
- Timestamps stay stored as TEXT in the database in an RFC3339-in-UTC format. We will likely migrate
  hb/db *record* timestamp fields (e.g. `created_at`) to `time::OffsetDateTime` while keeping
  hbtypes snapshots as `String` for now, unless hbtypes is ready to move too.
- Breaking changes in `musq` are permitted for this work. We'll use that to remove impedance
  mismatches rather than layering more ad-hoc helpers on top.
- `now()` should be DB-side (SQLite expression), not Rust-side computed binds, so all timestamps in
  a transaction are consistent and we don't couple correctness to host formatting.
- We need both:
  - curated, safe expression helpers (`now_rfc3339_utc()`, `jsonb_text(...)`, etc.) that are *not*
    tainted
  - an explicit raw escape hatch for expressions that *does* taint the query
- "JSONB" here refers to SQLite's internal binary JSON representation.
  Values should be written and read as JSON *text*; SQLite converts internally via `jsonb(...)` and
  `json(...)`. Our support should therefore focus on making `jsonb(?)` ergonomic and safe.
- hb is willing to move `hbtypes` and the hb codebase alongside musq changes; backwards
  compatibility is not required.
- It is acceptable for hb to add `musq::Encode` / `musq::Decode` for hb ID newtypes, including in
  `hbtypes` if hb is comfortable taking that dependency (or by putting impls in hb/db instead).

Constraints (blob compatibility):
- Do not change the semantics of plain `BLOB` binding/reading in musq. Existing users storing opaque
  bytes (images, etc.) must continue to work unchanged.
- Any optional JSON helpers must be opt-in and must not cause automatic interpretation of `BLOB`
  payloads. Opaque `BLOB` bytes must remain opaque unless a caller explicitly opts into JSON
  functions or encodings.
- JSONB helpers must be explicit (e.g. `expr::jsonb_text(...)`), so callers storing arbitrary blobs
  are never accidentally routed through SQLite JSON functions.

## Key findings (hb `db` today)

1. JSON payloads are stored as SQLite JSONB BLOBs:
   - schema uses `BLOB ... CHECK (json_valid(data, 6))` and friends
   - writes are mostly `jsonb({data})` where `{data}` is a JSON string
   - reads are mostly `json(data) AS data` and then `String` -> `serde_json::from_str(...)`
   - note: SQLite JSONB is an internal binary encoding; hb treats values as JSON text at the API
     boundary
   - hb docs already codify this: "Use `jsonb()` on write, `json(data)` on read, `->>` for
     extraction." (`hb/docs/storage.md`)

2. Timestamps are stored as TEXT and written via a repeated expression:
   - `STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')`
   - hb docs describe these as ISO 8601 timestamps (`hb/docs/storage.md`)
   - fields are typed as `String` in record structs and snapshots today

3. `musq::Values` is only used for "plain binds":
   - `WHERE {where:filters}` and `SET {set:changes}` work for basic values
   - JSONB and timestamp updates require leaving the `Values` path and hand-writing expressions

4. `WHERE {where:values}` cannot currently express `IS NULL` correctly:
   - it always emits `col = ?` even when the bound value is NULL, which will not match NULL rows
   - hb works around this with explicit `... WHERE parent_id IS NULL ...` patterns

## hb scope inventory (integrated)

Scope confirmation:
- `hb/crates/db` is the only hb crate using musq directly today (confirmed via repo-wide search).
- `hb/crates/db` already depends on musq via a path dependency, so musq changes are immediately
  consumable from hb without publishing or compatibility shims.

High-value migration targets (hb/db):
- JSONB writes via `jsonb(...)` appear in:
  - `hb/crates/db/src/nodes.rs`
  - `hb/crates/db/src/edges.rs`
  - `hb/crates/db/src/links.rs`
  - `hb/crates/db/src/operations.rs`
  - `hb/crates/db/src/migrations.rs`
- "now" timestamps via `STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')` appear in:
  - `hb/crates/db/src/nodes.rs`
  - `hb/crates/db/src/edges.rs`
  - `hb/crates/db/src/links.rs`
  - `hb/crates/db/src/operations.rs`
  - `hb/crates/db/src/batch.rs`
  - `hb/crates/db/src/migrations.rs`
- NULL filtering candidates (`col IS NULL`) exist, but some `IS NULL` uses are for optional bind
  parameters (e.g. `({param} IS NULL OR ...)`) and should not be mechanically rewritten into
  `Values`.
- After a repo-wide search, the only code-level `col IS NULL` filter we found was the root batches
  query in `hb/crates/db/src/operations.rs`; the remaining `IS NULL` occurrences are either schema
  constraints/docs or optional-parameter guards (not eligible for `Values` migration).
  - Remaining code-level optional-parameter guards:
    - `hb/crates/db/src/operations.rs`: `({before_id} IS NULL OR id < {before_cutoff})`
    - `hb/crates/db/src/edges.rs`: `({exclude_id} IS NULL OR id != {exclude_id_check})` (2 sites)
  - These can be revisited if/when musq gains richer WHERE support (e.g. `<`, `!=`, and conditional
    inclusion) beyond the current equality-only `{where:values}`.

## Proposed feature set (musq changes + hb consequences)

### Feature 1: NULL-aware `{where:values}` (fix correctness)

Musq change:
- In `QueryBuilder::push_where`, detect `Value::Null` and emit `col IS NULL` (no bind), instead of
  `col = ?` (bind NULL).
- Add tests in musq for `WHERE` filtering on NULL columns using `values! { "col": Null }`.
- Update README/docs to explicitly state the NULL behavior.

hb consequence:
- hb can use `Values` for many `IS NULL` checks, reducing hand-written SQL.

Before (hb style, today):
```rust
musq::sql!(
    "SELECT id FROM batches WHERE parent_id IS NULL AND closed_at IS NULL;"
)?
```

After (hb style, with NULL-aware where):
```rust
let filters = musq::values! { "parent_id": musq::Null, "closed_at": musq::Null }?;
musq::sql!("SELECT id FROM batches WHERE {where:filters};")?
```

### Feature 2: Expression-capable values for `{insert:...}` and `{set:...}`

Musq change:
- Evolve `musq::Values` to hold either bound values or expressions (breaking changes allowed):
  - bound: emits `?` and adds an argument (current behavior)
  - expression: emits inline SQL (may include its own binds) and merges arguments
- Add `musq::expr` module with a safe expression type (not `Encode`) that is explicit at call-sites.
- Extend the `{insert:...}` and `{set:...}` pathways to accept expression values.
- Provide a small set of safe, non-tainted helpers:
  - `expr::now_rfc3339_utc()` -> `STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')`
  - `expr::jsonb_text(&str)` -> `jsonb(?)` (binds the JSON text)
  - (optionally) `expr::null()` -> `NULL` (mostly for symmetry)
- Provide an explicit raw escape hatch (tainted), e.g. `expr::raw("...")`.

hb consequence:
- Many hb queries collapse to "pure Values" composition; repeated timestamp/JSONB snippets go away.

Before (hb `nodes.rs:update_node_data_tx` today):
```rust
let changes = musq::values! { "type": new_type.as_str() }?;
musq::sql!(
    "UPDATE nodes
     SET {set:changes},
         data = jsonb({data}),
         updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'),
         render_cache = NULL
     WHERE {where:filters};",
    data = &json,
)?;
```

After (hb perspective, with expression-capable values):
```rust
let changes = musq::values! {
    "type": new_type.as_str(),
    "data": musq::expr::jsonb_text(&json),
    "updated_at": musq::expr::now_rfc3339_utc(),
    "render_cache": musq::Null,
}?; // then: UPDATE nodes SET {set:changes} WHERE {where:filters}
```

### Feature 3: Document and standardize datetime format (+ optional type tightening)

Musq change:
- Document the canonical formats musq uses:
  - `time::OffsetDateTime` encodes as RFC3339 text
  - `time::PrimitiveDateTime` encodes as `YYYY-MM-DD HH:MM:SS.subsec`
- Document a recommended DB-side now expression that matches RFC3339-in-UTC for SQLite:
  - `STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')`
- Add an explicit docs section that hb can link to for "timestamp columns should be TEXT RFC3339".

hb consequence (optional but recommended):
- Change record timestamp fields from `String` -> `time::OffsetDateTime` and let musq decode them.

Before (hb record field today):
```rust
pub created_at: String,
```

After (hb record field, if we tighten types):
```rust
pub created_at: time::OffsetDateTime,
```

### Feature 4: SQLite JSONB ergonomics (and optional JSON helpers) for Values

Musq change:
- Do not add any automatic interpretation of `BLOB` bytes.
- Provide explicit helpers that work with JSON *text*:
  - keep `expr::jsonb_text(&str)` as the primary mechanism (SQLite converts to internal JSONB)
  - optionally provide `expr::jsonb_serde(&T)` where `T: Serialize` under an opt-in `json` feature
- Keep reads explicit in SQL via `json(column) AS ...` (hb already does this).

hb consequence:
- Removes repeated `serde_json::to_string(...)` + `jsonb({param})` plumbing, especially for
  `operations_log.payload`.

Before (hb `operations.rs:log_operation` today):
```rust
let json = serde_json::to_string(payload)?;
musq::sql!(
    "INSERT INTO operations_log (payload, created_at)
     VALUES (jsonb({payload}), STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'));",
    payload = &json,
)?;
```

After (hb perspective, with `expr::jsonb_serde` and `expr::now_rfc3339_utc`):
```rust
let vals = musq::values! {
    "payload": musq::expr::jsonb_serde(payload),
    "created_at": musq::expr::now_rfc3339_utc(),
}?;
musq::sql!("INSERT INTO operations_log {insert:vals};")?;
```

### Feature 5 (optional): Reduce hb boilerplate with `Encode`/`Decode` for ID newtypes

Musq change:
- None (preferred). This is best done in hb crates because of orphan rules.

hb consequence:
- If `hbtypes` implements `musq::Encode` for `NodeId`/`EdgeId`/etc, hb/db can stop calling `.get()`
  everywhere.
- If hb also implements `musq::Decode`, hb/db can stop manual `NodeId::new(id)` conversions.

## Execution checklist

1. Stage One: Musq correctness + documentation (low risk, independent)

a) Musq changes
1. [x] Implement NULL-aware `QueryBuilder::push_where` semantics for `Value::Null`
       (`musq/crates/musq/src/query_builder.rs`).
2. [x] Add a musq integration test proving `WHERE {where:values}` can match NULL columns
       (new test file or extend `musq/crates/musq/tests/dynamic_values.rs`).
3. [x] Document the NULL behavior in `musq/crates/musq/src/values.rs` and the README `Values`
       section.
4. [x] Add a regression test proving opaque `BLOB` storage is unaffected (random bytes round-trip).

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [x] Replace eligible `col IS NULL` patterns with `Values` + `musq::Null` and
       `WHERE {where:filters}` (e.g. `hb/crates/db/src/operations.rs` root batches query).
2. [x] Add/adjust hb tests to cover NULL filtering via `Values` (especially in query helpers where
       `IS NULL` previously appeared).
3. [x] Run hb test suite to validate behavior is unchanged.

2. Stage Two: Musq expression-capable Values

a) Musq changes
1. [ ] Update `musq::Values` to support bind-or-expression values
       (`musq/crates/musq/src/values.rs`), without changing plain `BLOB` binding semantics.
2. [ ] Update `QueryBuilder::{push_insert,push_set,push_where}` to render expressions and merge
       their arguments (`musq/crates/musq/src/query_builder.rs`).
3. [ ] Add `musq::expr` module and export it (`musq/crates/musq/src/lib.rs`), including:
   - `expr::now_rfc3339_utc()`
   - `expr::jsonb_text(&str)` (binds JSON text; SQLite produces JSONB internally)
   - `expr::raw(...)` (taints the query)
4. [ ] Add tests:
   - `SET {set:...}` mixes binds and expressions correctly
   - `{insert:...}` mixes binds and expressions correctly
   - expressions with their own binds merge arguments in the right order
5. [ ] Extend docs/README with a short "computed values" section and examples for JSONB + now().

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [ ] Replace all `STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')` occurrences with
       `musq::expr::now_rfc3339_utc()` via `{set:...}` / `{insert:...}`
       (see `hb/crates/db/src/{nodes,edges,links,operations,batch,migrations}.rs`).
2. [ ] Replace `jsonb({data})` patterns with `musq::expr::jsonb_text(...)` (or `jsonb_serde`) via
       `{set:...}` / `{insert:...}`
       (see `hb/crates/db/src/{nodes,edges,links,operations,migrations}.rs`).
3. [ ] Remove remaining hand-written SQL fragments that exist solely to express computed values
       (timestamps/JSONB), unless they are intentionally more readable.
4. [ ] Run hb/db tests and fix any ordering/taint assertions that change due to query rewrites.

3. Stage Three: hb/db type tightening (timestamps) (in hb repo)

a) Musq changes
1. [ ] None (this stage consumes the musq work from stages 1–2).

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [ ] Migrate hb/db record timestamp types from `String` -> `time::OffsetDateTime`
       (`hb/crates/db/src/{nodes,edges,links,operations,batch}.rs`).
2. [ ] Ensure all hb/db JSONB reads remain explicit in SQL (`json(column) AS ...`) and that decode
       types are updated accordingly.
3. [ ] Run hb/db tests to validate behavior and ordering assumptions remain correct.

4. Stage Four: hbtypes alignment (recommended)

a) Musq changes
1. [ ] None required, unless hb wants a `json` feature for `expr::jsonb_serde`.

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [ ] Add `musq::Encode` / `musq::Decode` impls for hb ID newtypes (either in hbtypes or hb/db).
2. [ ] If hbtypes snapshots should become typed timestamps, migrate snapshots from `String` to
       `time::OffsetDateTime` as well (and update serde formats accordingly).
