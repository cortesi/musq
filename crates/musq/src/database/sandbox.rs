//! Composable read-only SQL sandbox and multi-file `fleet_query`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use futures_util::TryStreamExt;
#[cfg(feature = "json")]
use serde_json::{Number as JsonNumber, Value as JsonValue};

use super::{Database, surface::QuerySurface};
use crate::{
    ActionCode, AuthResult, Connection, Error, Musq, Result, SandboxError, SqliteLimit, Text,
    Value, error::PrimaryErrCode, query,
};

/// Default maximum rows returned by [`ReadOnlySandbox::query`] and
/// [`fleet_query`].
pub const DEFAULT_SANDBOX_MAX_ROWS: usize = 1_000;
/// Default maximum estimated output bytes returned by
/// [`ReadOnlySandbox::query`] and [`fleet_query`].
pub const DEFAULT_SANDBOX_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Default per-statement timeout for sandboxed queries.
pub const DEFAULT_SANDBOX_TIMEOUT: Duration = Duration::from_secs(10);
/// Default `SQLITE_LIMIT_LENGTH` cap in bytes (16 MiB).
pub const DEFAULT_SANDBOX_LIMIT_LENGTH: u32 = 16 * 1024 * 1024;
/// Default `SQLITE_LIMIT_SQL_LENGTH` cap in bytes (64 KiB).
pub const DEFAULT_SANDBOX_LIMIT_SQL_LENGTH: u32 = 64 * 1024;
/// Default `SQLITE_LIMIT_EXPR_DEPTH` cap.
pub const DEFAULT_SANDBOX_LIMIT_EXPR_DEPTH: u32 = 64;
/// Default `SQLITE_LIMIT_COMPOUND_SELECT` cap.
pub const DEFAULT_SANDBOX_LIMIT_COMPOUND_SELECT: u32 = 16;

/// Built-in allowlist of safe read-only SQL scalar and aggregate functions.
const BUILTIN_ALLOWED_FUNCTIONS: &[&str] = &[
    "abs",
    "avg",
    "coalesce",
    "count",
    "glob",
    "group_concat",
    "hex",
    "ifnull",
    "instr",
    "json_array_length",
    "json_extract",
    "json_type",
    "json_valid",
    "length",
    "like",
    "lower",
    "ltrim",
    "max",
    "min",
    "nullif",
    "replace",
    "round",
    "rtrim",
    "substr",
    "substring",
    "sum",
    "total",
    "trim",
    "typeof",
    "upper",
];

/// Bounds and runtime limits for [`ReadOnlySandbox::query`] and
/// [`fleet_query`].
#[derive(Debug, Clone)]
pub struct SandboxQueryOptions {
    /// Maximum number of rows returned.
    pub max_rows: usize,
    /// Maximum cumulative estimated output bytes.
    pub max_bytes: usize,
    /// Per-statement execution timeout enforced by `sqlite3_progress_handler`.
    pub timeout: Duration,
    /// `SQLITE_LIMIT_LENGTH` cap in bytes.
    pub limit_length: u32,
    /// `SQLITE_LIMIT_SQL_LENGTH` cap in bytes.
    pub limit_sql_length: u32,
    /// `SQLITE_LIMIT_EXPR_DEPTH` cap.
    pub limit_expr_depth: u32,
    /// `SQLITE_LIMIT_COMPOUND_SELECT` cap.
    pub limit_compound_select: u32,
    /// SQLite `PRAGMA cache_size` value (negative = KiB; default `-8192`).
    pub cache_size_kib: i32,
    /// Open connections with SQLite `immutable=1` in [`fleet_query`].
    pub immutable: bool,
}

impl Default for SandboxQueryOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxQueryOptions {
    /// Construct default [`SandboxQueryOptions`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_rows: DEFAULT_SANDBOX_MAX_ROWS,
            max_bytes: DEFAULT_SANDBOX_MAX_BYTES,
            timeout: DEFAULT_SANDBOX_TIMEOUT,
            limit_length: DEFAULT_SANDBOX_LIMIT_LENGTH,
            limit_sql_length: DEFAULT_SANDBOX_LIMIT_SQL_LENGTH,
            limit_expr_depth: DEFAULT_SANDBOX_LIMIT_EXPR_DEPTH,
            limit_compound_select: DEFAULT_SANDBOX_LIMIT_COMPOUND_SELECT,
            cache_size_kib: -8192,
            immutable: false,
        }
    }

    /// Set the maximum row cap.
    #[must_use]
    pub const fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = max_rows;
        self
    }

    /// Set the maximum output byte cap.
    #[must_use]
    pub const fn max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// Set the per-statement execution timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set `SQLITE_LIMIT_LENGTH`.
    #[must_use]
    pub const fn limit_length(mut self, limit_length: u32) -> Self {
        self.limit_length = limit_length;
        self
    }

    /// Set `SQLITE_LIMIT_SQL_LENGTH`.
    #[must_use]
    pub const fn limit_sql_length(mut self, limit_sql_length: u32) -> Self {
        self.limit_sql_length = limit_sql_length;
        self
    }

    /// Set `SQLITE_LIMIT_EXPR_DEPTH`.
    #[must_use]
    pub const fn limit_expr_depth(mut self, limit_expr_depth: u32) -> Self {
        self.limit_expr_depth = limit_expr_depth;
        self
    }

    /// Set `SQLITE_LIMIT_COMPOUND_SELECT`.
    #[must_use]
    pub const fn limit_compound_select(mut self, limit_compound_select: u32) -> Self {
        self.limit_compound_select = limit_compound_select;
        self
    }

    /// Set whether [`fleet_query`] opens files with `immutable=1`.
    #[must_use]
    pub const fn immutable(mut self, immutable: bool) -> Self {
        self.immutable = immutable;
        self
    }
}

/// Tabular result returned by [`ReadOnlySandbox::query`] and [`fleet_query`].
#[derive(Debug, Clone)]
pub struct SandboxQueryResult {
    /// Column names in result order.
    pub columns: Vec<String>,
    /// Rows of SQLite values in column order.
    pub rows: Vec<Vec<Value>>,
    /// `true` when the query hit `max_rows` or `max_bytes`.
    pub truncated: bool,
}

impl SandboxQueryResult {
    /// Convert every row into JSON values (blobs rendered as lowercase hex).
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    #[must_use]
    pub fn rows_as_json(&self) -> Vec<Vec<JsonValue>> {
        self.rows
            .iter()
            .map(|row| row.iter().map(value_to_json).collect())
            .collect()
    }
}

/// Hardened read-only SQL sandbox over a composed [`QuerySurface`].
///
/// Each [`Self::query`] call opens a fresh unpooled read-only SQLite
/// connection with `SQLITE_DBCONFIG_DEFENSIVE`, `trusted_schema = OFF`,
/// `PRAGMA query_only = ON`, `SqliteLimit` caps, `statement_timeout`,
/// `TEMP` views, pre-authorizer `prepare_hooks`, and a strict SQLite
/// authorizer blocking `_musq_modules`, `sqlite_*` catalog tables, unexposed
/// base tables, writes, PRAGMAs, and `ATTACH`.
#[derive(Debug, Clone)]
pub struct ReadOnlySandbox {
    /// Target database file path.
    path: PathBuf,
    /// Open SQLite connection with `immutable=1`.
    immutable: bool,
    /// Composed query surface for this sandbox.
    surface: QuerySurface,
}

impl ReadOnlySandbox {
    /// Create a [`ReadOnlySandbox`] for `db`, unioning the [`QuerySurface`] of
    /// every mounted or registered module on `db`.
    #[must_use]
    pub fn new(db: &Database) -> Self {
        Self {
            path: db.path().to_path_buf(),
            immutable: db.is_immutable(),
            surface: db.query_surface(),
        }
    }

    /// Create a [`ReadOnlySandbox`] for `path` using an explicit
    /// [`QuerySurface`].
    #[must_use]
    pub fn from_surface(path: impl Into<PathBuf>, immutable: bool, surface: QuerySurface) -> Self {
        Self {
            path: path.into(),
            immutable,
            surface,
        }
    }

    /// Return the target database path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return whether connections open with `immutable=1`.
    #[must_use]
    pub const fn is_immutable(&self) -> bool {
        self.immutable
    }

    /// Return the active [`QuerySurface`].
    #[must_use]
    pub const fn surface(&self) -> &QuerySurface {
        &self.surface
    }

    /// Execute one bounded read-only SQL statement inside the sandbox.
    pub async fn query(
        &self,
        sql: &str,
        params: &BTreeMap<String, Value>,
        options: &SandboxQueryOptions,
    ) -> Result<SandboxQueryResult> {
        validate_single_statement(sql)?;

        let musq_opts = Musq::new()
            .filename(&self.path)
            .read_only(true)
            .immutable(self.immutable)
            .defensive(true)
            .trusted_schema(false)
            .statement_timeout(options.timeout)
            .pragma("temp_store", "FILE")
            .pragma("cache_size", &options.cache_size_kib.to_string())
            .pragma("mmap_size", "0");

        let mut conn = Connection::connect_with(&musq_opts).await?;
        let outcome = execute_on_sandbox_conn(&mut conn, &self.surface, sql, params, options).await;
        conn.close().await.ok();
        outcome
    }
}

/// Execute one bounded read-only SQL query across multiple database files and
/// merge rows with a leading `source_file` column while enforcing global row
/// and byte caps.
pub async fn fleet_query<P: AsRef<Path>>(
    paths: &[P],
    surface: &QuerySurface,
    sql: &str,
    params: &BTreeMap<String, Value>,
    options: &SandboxQueryOptions,
) -> Result<SandboxQueryResult> {
    validate_single_statement(sql)?;
    if paths.is_empty() {
        return Ok(SandboxQueryResult {
            columns: Vec::new(),
            rows: Vec::new(),
            truncated: false,
        });
    }

    let mut merged_columns: Option<Vec<String>> = None;
    let mut merged_rows: Vec<Vec<Value>> = Vec::new();
    let mut approx_bytes = 0_usize;
    let mut truncated = false;

    for path_ref in paths {
        if merged_rows.len() >= options.max_rows {
            truncated = true;
            break;
        }
        let remaining_rows = options.max_rows.saturating_sub(merged_rows.len());
        let remaining_bytes = options.max_bytes.saturating_sub(approx_bytes);
        if !merged_rows.is_empty() && remaining_bytes == 0 {
            truncated = true;
            break;
        }

        let per_file_opts = SandboxQueryOptions {
            max_rows: remaining_rows,
            max_bytes: remaining_bytes.max(1024),
            ..options.clone()
        };
        let path = path_ref.as_ref();
        let file_label = path.to_string_lossy().into_owned();
        let sandbox = ReadOnlySandbox::from_surface(path, options.immutable, surface.clone());
        let partial = sandbox.query(sql, params, &per_file_opts).await?;

        if merged_columns.is_none() && !partial.columns.is_empty() {
            let mut cols = Vec::with_capacity(partial.columns.len() + 1);
            cols.push("source_file".to_owned());
            cols.extend(partial.columns);
            merged_columns = Some(cols);
        }

        for row in partial.rows {
            if merged_rows.len() >= options.max_rows {
                truncated = true;
                break;
            }
            let label_val = Value::Text {
                value: Text::new(file_label.clone()),
                type_info: None,
            };
            let mut tagged = Vec::with_capacity(row.len() + 1);
            let mut row_bytes = 2_usize.saturating_add(estimate_value_bytes(&label_val));
            tagged.push(label_val);
            for cell in row {
                row_bytes = row_bytes.saturating_add(estimate_value_bytes(&cell));
                tagged.push(cell);
            }
            if !merged_rows.is_empty() && approx_bytes.saturating_add(row_bytes) > options.max_bytes
            {
                truncated = true;
                break;
            }
            approx_bytes = approx_bytes.saturating_add(row_bytes);
            merged_rows.push(tagged);
        }

        if partial.truncated {
            truncated = true;
        }
    }

    Ok(SandboxQueryResult {
        columns: merged_columns.unwrap_or_default(),
        rows: merged_rows,
        truncated,
    })
}

/// Validate that `sql` contains exactly one non-empty SQL statement (allowing
/// whitespace, line/block comments, and at most one trailing semicolon).
pub fn validate_single_statement(sql: &str) -> Result<()> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut pos = 0;
    let mut saw_token = false;
    let mut saw_semicolon = false;

    while pos < len {
        let ch = bytes[pos];
        if ch.is_ascii_whitespace() {
            pos += 1;
            continue;
        }
        if let Some(next_pos) = skip_comment(bytes, pos)? {
            pos = next_pos;
            continue;
        }
        if saw_semicolon {
            return Err(Error::Sandbox(SandboxError::Rejected(
                "multi-statement SQL is not allowed".to_owned(),
            )));
        }
        if ch == b';' {
            if !saw_token {
                return Err(Error::Sandbox(SandboxError::Rejected(
                    "empty SQL statement".to_owned(),
                )));
            }
            saw_semicolon = true;
            pos += 1;
            continue;
        }
        saw_token = true;
        pos = advance_sql_token(bytes, pos);
    }

    if !saw_token {
        return Err(Error::Sandbox(SandboxError::Rejected(
            "empty SQL statement".to_owned(),
        )));
    }
    Ok(())
}

/// Skip a SQL line (`--`) or block (`/* ... */`) comment starting at `pos`.
fn skip_comment(bytes: &[u8], pos: usize) -> Result<Option<usize>> {
    let len = bytes.len();
    if bytes[pos] == b'-' && pos + 1 < len && bytes[pos + 1] == b'-' {
        let mut cursor = pos + 2;
        while cursor < len && bytes[cursor] != b'\n' {
            cursor += 1;
        }
        return Ok(Some(cursor));
    }
    if bytes[pos] == b'/' && pos + 1 < len && bytes[pos + 1] == b'*' {
        let mut cursor = pos + 2;
        while cursor + 1 < len {
            if bytes[cursor] == b'*' && bytes[cursor + 1] == b'/' {
                return Ok(Some(cursor + 2));
            }
            cursor += 1;
        }
        return Err(Error::Sandbox(SandboxError::Rejected(
            "unterminated block comment in SQL".to_owned(),
        )));
    }
    Ok(None)
}

/// Advance past one quoted string/identifier or single SQL byte starting at
/// `pos`.
fn advance_sql_token(bytes: &[u8], pos: usize) -> usize {
    let len = bytes.len();
    let ch = bytes[pos];
    if matches!(ch, b'\'' | b'"' | b'`') {
        let mut cursor = pos + 1;
        while cursor < len {
            if bytes[cursor] == ch {
                if cursor + 1 < len && bytes[cursor + 1] == ch {
                    cursor += 2;
                } else {
                    return cursor + 1;
                }
            } else {
                cursor += 1;
            }
        }
        return len;
    }
    if ch == b'[' {
        let mut cursor = pos + 1;
        while cursor < len {
            if bytes[cursor] == b']' {
                return cursor + 1;
            }
            cursor += 1;
        }
        return len;
    }
    pos + 1
}

/// Authorizer allowlists derived from a [`QuerySurface`].
struct AuthorizerPolicy {
    /// Lowercased `TEMP` view names exposed to queries.
    allowed_views: Vec<String>,
    /// Lowercased base table names in `main` allowed for `count(*)` probes.
    count_only_tables: Vec<String>,
    /// Lowercased SQL function names allowed in expressions.
    allowed_functions: Vec<String>,
}

impl AuthorizerPolicy {
    /// Compile an [`AuthorizerPolicy`] from `surface`.
    fn from_surface(surface: &QuerySurface) -> Self {
        let allowed_views = surface
            .allowed_views
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        let count_only_tables = surface
            .count_only_tables
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect();
        let mut allowed_functions: Vec<String> = BUILTIN_ALLOWED_FUNCTIONS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        for extra in &surface.allowed_functions {
            let lower = extra.to_ascii_lowercase();
            if !allowed_functions.contains(&lower) {
                allowed_functions.push(lower);
            }
        }
        Self {
            allowed_views,
            count_only_tables,
            allowed_functions,
        }
    }

    /// Evaluate one SQLite authorizer callback invocation.
    fn authorize(
        &self,
        action: ActionCode,
        arg1: Option<&str>,
        arg2: Option<&str>,
        db_name: Option<&str>,
        trigger_or_view: Option<&str>,
    ) -> AuthResult {
        match action {
            ActionCode::Select | ActionCode::Recursive => AuthResult::Ok,
            ActionCode::Read => self.authorize_read(arg1, arg2, db_name, trigger_or_view),
            ActionCode::Function => {
                if arg2.is_some_and(|func| contains_ascii_ci(&self.allowed_functions, func)) {
                    AuthResult::Ok
                } else {
                    AuthResult::Deny
                }
            }
            _ => AuthResult::Deny,
        }
    }

    /// Evaluate a `SQLITE_READ` callback, unconditionally denying
    /// `_musq_modules` and `sqlite_*` catalog tables.
    fn authorize_read(
        &self,
        arg1: Option<&str>,
        arg2: Option<&str>,
        db_name: Option<&str>,
        trigger_or_view: Option<&str>,
    ) -> AuthResult {
        if arg1.is_some_and(is_forbidden_catalog_table) {
            return AuthResult::Deny;
        }
        let reading_allowed_temp_view = db_name == Some("temp")
            && arg1.is_some_and(|name| contains_ascii_ci(&self.allowed_views, name));
        let via_allowed_view =
            trigger_or_view.is_some_and(|view| contains_ascii_ci(&self.allowed_views, view));
        let count_star_probe = db_name == Some("main")
            && arg2 == Some("")
            && trigger_or_view.is_none()
            && arg1.is_some_and(|table| contains_ascii_ci(&self.count_only_tables, table));

        if reading_allowed_temp_view || via_allowed_view || count_star_probe {
            AuthResult::Ok
        } else {
            AuthResult::Deny
        }
    }
}

/// Return `true` for `_musq_modules` or any `sqlite_*` catalog table.
fn is_forbidden_catalog_table(name: &str) -> bool {
    name.eq_ignore_ascii_case("_musq_modules")
        || name
            .as_bytes()
            .get(..7)
            .is_some_and(|head| head.eq_ignore_ascii_case(b"sqlite_"))
}

/// Return `true` if `needle` matches any entry in lowercased `haystack`.
fn contains_ascii_ci(haystack: &[String], needle: &str) -> bool {
    haystack
        .iter()
        .any(|item| item.eq_ignore_ascii_case(needle))
}

/// Prepare a fresh sandbox connection, install limits and the authorizer, and
/// stream rows within `options` bounds.
async fn execute_on_sandbox_conn(
    conn: &mut Connection,
    surface: &QuerySurface,
    sql: &str,
    params: &BTreeMap<String, Value>,
    options: &SandboxQueryOptions,
) -> Result<SandboxQueryResult> {
    for hook in &surface.prepare_hooks {
        hook(conn, sql).await?;
    }

    for ddl in &surface.temp_views {
        if !ddl.trim().is_empty() {
            query(ddl).execute(&*conn).await?;
        }
    }

    query("PRAGMA query_only = ON").execute(&*conn).await?;

    conn.set_limit(SqliteLimit::Length, options.limit_length)
        .await?;
    conn.set_limit(SqliteLimit::SqlLength, options.limit_sql_length)
        .await?;
    conn.set_limit(SqliteLimit::ExprDepth, options.limit_expr_depth)
        .await?;
    conn.set_limit(SqliteLimit::CompoundSelect, options.limit_compound_select)
        .await?;
    conn.set_limit(SqliteLimit::Attached, 0).await?;

    let policy = AuthorizerPolicy::from_surface(surface);
    conn.set_authorizer(move |action, arg1, arg2, db_name, trigger_or_view| {
        policy.authorize(action, arg1, arg2, db_name, trigger_or_view)
    })
    .await?;

    let mut stmt = query(sql);
    for (name, val) in params {
        stmt = stmt.try_bind_named(name, val.clone())?;
    }

    let mut stream = stmt.fetch(&*conn);
    let mut columns: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<Value>> = Vec::with_capacity(options.max_rows.min(1024));
    let mut approx_bytes = 0_usize;
    let mut truncated = false;

    while let Some(row) = stream.try_next().await.map_err(map_sandbox_error)? {
        if columns.is_empty() {
            columns = row
                .column_names()
                .into_iter()
                .map(ToOwned::to_owned)
                .collect();
        }
        if rows.len() >= options.max_rows {
            truncated = true;
            break;
        }
        let mut row_values = Vec::with_capacity(row.len());
        let mut row_bytes = 2_usize;
        for col_idx in 0..row.len() {
            let val: Value = row.get_value_idx(col_idx)?;
            row_bytes = row_bytes.saturating_add(estimate_value_bytes(&val));
            row_values.push(val);
        }
        if !rows.is_empty() && approx_bytes.saturating_add(row_bytes) > options.max_bytes {
            truncated = true;
            break;
        }
        approx_bytes = approx_bytes.saturating_add(row_bytes);
        rows.push(row_values);
    }

    Ok(SandboxQueryResult {
        columns,
        rows,
        truncated,
    })
}

/// Map SQLite sandbox enforcement errors to typed [`SandboxError`] variants.
fn map_sandbox_error(err: Error) -> Error {
    if let Some(sqlite) = err.as_sqlite() {
        if sqlite.primary == PrimaryErrCode::Auth || sqlite.message.contains("not authorized") {
            return Error::Sandbox(SandboxError::Rejected(sqlite.message.clone()));
        }
        if sqlite.primary == PrimaryErrCode::Interrupt {
            return Error::Sandbox(SandboxError::Timeout(sqlite.message.clone()));
        }
        if sqlite.primary == PrimaryErrCode::TooBig
            || sqlite.message.contains("too large")
            || sqlite.message.contains("too many terms")
        {
            return Error::Sandbox(SandboxError::LimitExceeded(sqlite.message.clone()));
        }
    }
    err
}

/// Estimate output byte contribution of one [`Value`] for output capping.
fn estimate_value_bytes(value: &Value) -> usize {
    match value {
        Value::Null { .. } => 4,
        Value::Integer { .. } | Value::Double { .. } => 12,
        Value::Text { value, .. } => value.as_bytes().len().saturating_add(2),
        Value::Blob { value, .. } => value.len().saturating_mul(2).saturating_add(2),
    }
}

/// Convert a [`Value`] to a [`serde_json::Value`] (blobs rendered as lowercase
/// hex).
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
#[must_use]
pub fn value_to_json(value: &Value) -> JsonValue {
    match value {
        Value::Null { .. } => JsonValue::Null,
        Value::Integer { value, .. } => JsonValue::Number(JsonNumber::from(*value)),
        Value::Double { value, .. } => {
            JsonNumber::from_f64(*value).map_or(JsonValue::Null, JsonValue::Number)
        }
        Value::Text { value, .. } => JsonValue::String(value.as_str().to_owned()),
        Value::Blob { value, .. } => JsonValue::String(encode_hex(value.as_ref())),
    }
}

/// Convert named JSON SQL parameters (`null`, `bool`, number, or string) into
/// [`Value`] bindings for [`ReadOnlySandbox::query`] or [`fleet_query`].
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub fn json_params_to_values(
    params: &BTreeMap<String, JsonValue>,
) -> Result<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    for (name, val) in params {
        let converted = match val {
            JsonValue::Null => Value::Null { type_info: None },
            JsonValue::Bool(flag) => Value::Integer {
                value: i64::from(*flag),
                type_info: None,
            },
            JsonValue::Number(num) => {
                if let Some(int_val) = num.as_i64() {
                    Value::Integer {
                        value: int_val,
                        type_info: None,
                    }
                } else if let Some(float_val) = num.as_f64() {
                    Value::Double {
                        value: float_val,
                        type_info: None,
                    }
                } else {
                    return Err(Error::Sandbox(SandboxError::InvalidParameter(format!(
                        "SQL parameter `{name}` has unsupported numeric value"
                    ))));
                }
            }
            JsonValue::String(text) => Value::Text {
                value: Text::new(text.clone()),
                type_info: None,
            },
            JsonValue::Array(_) | JsonValue::Object(_) => {
                return Err(Error::Sandbox(SandboxError::InvalidParameter(format!(
                    "SQL parameter `{name}` must be string, number, boolean, or null"
                ))));
            }
        };
        out.insert(name.clone(), converted);
    }
    Ok(out)
}

/// Encode bytes as lowercase hexadecimal.
#[cfg(feature = "json")]
fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[usize::from(byte >> 4)] as char);
        out.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::validate_single_statement;

    #[test]
    fn single_statement_validator_accepts_one_and_rejects_multi() {
        assert!(validate_single_statement("SELECT 1").is_ok());
        assert!(validate_single_statement("  SELECT ';' FROM v_a; -- trailing ").is_ok());
        assert!(validate_single_statement("/* ok */ SELECT [a;b] FROM v_a /* ; */ ;").is_ok());
        assert!(validate_single_statement("").is_err());
        assert!(validate_single_statement("   ;  ").is_err());
        assert!(validate_single_statement("SELECT 1; SELECT 2").is_err());
        assert!(validate_single_statement("SELECT 1; DROP TABLE a_items").is_err());
        assert!(validate_single_statement("/* unclosed").is_err());
    }
}
