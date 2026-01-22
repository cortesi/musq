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
  - curated, safe expression helpers (`now_rfc3339_utc()`, `jsonb(...)`, `jsonb_serde(...)`, etc.)
    that are *not* tainted
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
- JSONB helpers must be explicit (e.g. `expr::jsonb(...)`), so callers storing arbitrary blobs
  are never accidentally routed through SQLite JSON functions.
  - Prefer `expr::jsonb(...)` / `expr::jsonb_serde(...)`; `expr::jsonb_text(...)` is an alias for
    the string form.

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

4. `WHERE {where:values}` needs correct `NULL` handling:
   - SQLite requires `col IS NULL` (not `col = NULL`) to match NULL rows
   - **Status:** implemented in musq (`Value::Null` -> `col IS NULL`) and hb migrated the one
     eligible `col IS NULL` call-site (`get_root_batches`)

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
  - `expr::jsonb(&str)` -> `jsonb(?)` (binds JSON text)
  - `expr::jsonb_serde(&T)` -> `jsonb(?)` (serializes `T: Serialize` to JSON text, then binds)
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
    "data": musq::expr::jsonb(&json),
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
  - keep `expr::jsonb(&str)` as the primary mechanism (SQLite converts to internal JSONB)
  - provide `expr::jsonb_serde(&T)` where `T: Serialize` (serializes to JSON text, then binds)
  - `expr::jsonb_text(&str)` is an alias for `expr::jsonb(&str)`
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
    "payload": musq::expr::jsonb_serde(payload)?,
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

Completed (implemented)
- [x] NULL-aware `{where:values}`: musq emits `col IS NULL` for `Value::Null`, hb migrated the one
      eligible `col IS NULL` call-site and added coverage.
- [x] Expression-capable `Values` + `musq::expr`: hb migrated all JSONB writes and DB-side `now()`
      timestamps to `{insert:...}` / `{set:...}` with `expr::{jsonb,jsonb_serde,now_rfc3339_utc}`.

1. Stage One: hb/db type tightening (timestamps) (in hb repo)

a) Musq changes
1. [ ] None (this stage consumes the musq work above).

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [ ] Migrate hb/db record timestamp types from `String` -> `time::OffsetDateTime`
       (`hb/crates/db/src/{nodes,edges,links,operations,batch}.rs`).
2. [ ] Ensure all hb/db JSONB reads remain explicit in SQL (`json(column) AS ...`) and that decode
       types are updated accordingly.
3. [ ] Run hb/db tests to validate behavior and ordering assumptions remain correct.

2. Stage Two: hbtypes alignment (recommended)

a) Musq changes
1. [ ] None required.

b) hb changes
Switch context: move to the hb repo and update hb crates to capitalize on the musq changes from
this stage.
1. [ ] Add `musq::Encode` / `musq::Decode` impls for hb ID newtypes (either in hbtypes or hb/db).
2. [ ] If hbtypes snapshots should become typed timestamps, migrate snapshots from `String` to
       `time::OffsetDateTime` as well (and update serde formats accordingly).
