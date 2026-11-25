//! Composable schema modules, `_musq_modules` registry, prefix ownership
//! verification, migrations, and read-only module discovery.

use std::{collections::HashMap, future::Future};

use super::surface::QuerySurface;
use crate::{
    Connection, Error, ModuleError, Pool, Result, Transaction, query, query_as, query_scalar,
};

/// Registry entry recorded in `_musq_modules`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleRecord {
    /// Stable module identifier, such as `"verber.webtrace"`.
    pub module_id: String,
    /// Table prefix owned by this module, such as `"wt_"`.
    pub table_prefix: String,
    /// Installed schema version.
    pub schema_version: u32,
    /// UTC RFC 3339 timestamp when the module was installed or last migrated.
    pub installed_at: String,
    /// Module metadata JSON object.
    pub meta_json: String,
}

/// Composable schema module hosted on a [`crate::Database`].
///
/// Each module owns a unique [`Self::TABLE_PREFIX`] and declares its upstream
/// module dependencies in [`Self::DEPENDS_ON`]. On mount, the host snapshots
/// `sqlite_schema` before and after [`Self::install`] (and [`Self::migrate`])
/// to verify that every newly created physical table, index, and trigger starts
/// with [`Self::TABLE_PREFIX`], that indexes and triggers target only tables
/// with [`Self::TABLE_PREFIX`], and that views start with
/// [`Self::TABLE_PREFIX`] or `v_<TABLE_PREFIX>`.
pub trait Module {
    /// Stable module identifier (for example `"verber.webtrace"`).
    const ID: &'static str;
    /// Required prefix for every table, index, and trigger owned by this
    /// module (for example `"wt_"`).
    const TABLE_PREFIX: &'static str;
    /// Current schema version of this module.
    const SCHEMA_VERSION: u32;
    /// Module identifiers that must already be installed before this module
    /// mounts.
    const DEPENDS_ON: &'static [&'static str] = &[];

    /// Install this module's schema from scratch inside an active write
    /// transaction.
    fn install(tx: &mut Transaction<&mut Connection>) -> impl Future<Output = Result<()>> + Send;

    /// Migrate this module's schema from `from_version` to
    /// [`Self::SCHEMA_VERSION`] inside an active write transaction with
    /// `PRAGMA defer_foreign_keys = ON`.
    ///
    /// The default implementation returns
    /// [`ModuleError::MigrationUnsupported`].
    fn migrate(
        _tx: &mut Transaction<&mut Connection>,
        from_version: u32,
    ) -> impl Future<Output = Result<()>> + Send {
        async move {
            Err(Error::Module(ModuleError::MigrationUnsupported {
                module_id: Self::ID.to_owned(),
                from_version,
                to_version: Self::SCHEMA_VERSION,
            }))
        }
    }

    /// Optional metadata JSON stored in `_musq_modules.meta_json`.
    fn meta_json(&self) -> String {
        "{}".to_owned()
    }

    /// Optional read-only [`QuerySurface`] contributed by this module to
    /// [`crate::ReadOnlySandbox`].
    fn query_surface(&self) -> QuerySurface {
        QuerySurface::new()
    }
}

/// Row loaded from `sqlite_schema` for prefix and target-table ownership
/// checks.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SchemaObject {
    /// SQLite object type (`table`, `index`, `view`, or `trigger`).
    obj_type: String,
    /// SQLite object name.
    name: String,
    /// Owning table name (`sqlite_schema.tbl_name`).
    tbl_name: String,
    /// Original DDL SQL recorded by SQLite (if any).
    sql: Option<String>,
}

/// Raw row fetched from `_musq_modules`.
type RawModuleRow = (String, String, i64, String, String);

/// Validate `M::ID` and `M::TABLE_PREFIX` before starting a mount transaction.
pub fn validate_module_identity(module_id: &str, prefix: &str) -> Result<()> {
    if module_id.trim().is_empty() {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "module_id must not be empty".to_owned(),
        }));
    }
    if prefix.is_empty() {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "table_prefix must not be empty".to_owned(),
        }));
    }
    if prefixes_overlap(prefix, "_musq_") {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "prefix overlaps reserved `_musq_` host metadata namespace".to_owned(),
        }));
    }
    if prefixes_overlap(prefix, "sqlite_") {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "prefix overlaps reserved `sqlite_` namespace".to_owned(),
        }));
    }
    let bytes = prefix.as_bytes();
    let valid_first = bytes[0].is_ascii_alphabetic() || bytes[0] == b'_';
    let valid_rest = bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
    if !valid_first || !valid_rest {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "table_prefix must start with an ASCII letter or `_` and contain only ASCII letters, digits, or `_`".to_owned(),
        }));
    }
    Ok(())
}

/// Next step of a mount of `M`, decided from the installed module records.
enum MountStep {
    /// `M` is installed at [`Module::SCHEMA_VERSION`], so the mount is done.
    Done(ModuleRecord),
    /// `M` is not installed.
    Install,
    /// `M` is installed at this older schema version.
    Upgrade(u32),
}

/// Decide the mount step of `M` from `installed`, or return the error that
/// refuses the mount.
fn mount_step<M: Module>(installed: Vec<ModuleRecord>) -> Result<MountStep> {
    check_dependencies::<M>(&installed)?;
    check_prefix_conflict::<M>(&installed)?;

    let Some(existing) = installed.into_iter().find(|rec| rec.module_id == M::ID) else {
        return Ok(MountStep::Install);
    };
    if !existing.table_prefix.eq_ignore_ascii_case(M::TABLE_PREFIX) {
        return Err(Error::Module(ModuleError::PrefixMismatch {
            module_id: M::ID.to_owned(),
            installed_prefix: existing.table_prefix,
            expected_prefix: M::TABLE_PREFIX.to_owned(),
        }));
    }
    if existing.schema_version == M::SCHEMA_VERSION {
        return Ok(MountStep::Done(existing));
    }
    if existing.schema_version > M::SCHEMA_VERSION {
        return Err(Error::Module(ModuleError::Downgrade {
            module_id: M::ID.to_owned(),
            installed_version: existing.schema_version,
            target_version: M::SCHEMA_VERSION,
        }));
    }
    Ok(MountStep::Upgrade(existing.schema_version))
}

/// Return the record of `M` when the read pool of a writable host shows a done
/// mount of `M`: `M` at [`Module::SCHEMA_VERSION`] with its dependencies and
/// its prefix intact.
///
/// Return `None` when the mount must run in a write transaction: an install,
/// an upgrade, a refusal, or a failed read. That transaction decides again, and
/// returns each refusal and each error, so a mount needs only the writer.
pub async fn done_mount_from_pool<M: Module>(pool: &Pool) -> Option<ModuleRecord> {
    let installed = load_modules_from_pool(pool).await.ok()?;
    match mount_step::<M>(installed) {
        Ok(MountStep::Done(record)) => Some(record),
        Ok(MountStep::Install | MountStep::Upgrade(_)) | Err(_) => None,
    }
}

/// Execute module mount (fresh install, idempotent check, or upgrade) inside an
/// active `BEGIN IMMEDIATE` transaction.
pub async fn mount_in_tx<M: Module>(
    tx: &mut Transaction<&mut Connection>,
    meta_json: &str,
) -> Result<ModuleRecord> {
    let installed = load_modules_in_tx(tx).await?;
    match mount_step::<M>(installed)? {
        MountStep::Done(existing) => Ok(existing),
        MountStep::Install => install_module_in_tx::<M>(tx, meta_json).await,
        MountStep::Upgrade(from_version) => {
            upgrade_module_in_tx::<M>(tx, from_version, meta_json).await
        }
    }
}

/// Fresh-install `M` inside `tx`, verifying every newly created schema object
/// and checking foreign keys before recording `_musq_modules`.
async fn install_module_in_tx<M: Module>(
    tx: &mut Transaction<&mut Connection>,
    meta_json: &str,
) -> Result<ModuleRecord> {
    let before = snapshot_schema(tx).await?;
    M::install(tx).await?;
    let after = snapshot_schema(tx).await?;

    verify_schema_changes(M::ID, M::TABLE_PREFIX, &before, &after)?;
    check_foreign_keys(tx, M::ID).await?;

    query(
        "INSERT INTO _musq_modules (module_id, table_prefix, schema_version, installed_at, meta_json)
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?)",
    )
    .bind(M::ID)
    .bind(M::TABLE_PREFIX)
    .bind(i64::from(M::SCHEMA_VERSION))
    .bind(meta_json)
    .execute(&**tx)
    .await?;

    load_module_in_tx(tx, M::ID).await?.ok_or_else(|| {
        Error::Protocol(format!(
            "module `{}` missing from `_musq_modules` immediately after install",
            M::ID
        ))
    })
}

/// Upgrade an already-installed module inside `tx` with
/// `PRAGMA defer_foreign_keys = ON` and post-migration `foreign_key_check`.
async fn upgrade_module_in_tx<M: Module>(
    tx: &mut Transaction<&mut Connection>,
    from_version: u32,
    meta_json: &str,
) -> Result<ModuleRecord> {
    let before = snapshot_schema(tx).await?;
    query("PRAGMA defer_foreign_keys = ON")
        .execute(&**tx)
        .await?;
    M::migrate(tx, from_version).await?;
    let after = snapshot_schema(tx).await?;

    verify_schema_changes(M::ID, M::TABLE_PREFIX, &before, &after)?;
    check_foreign_keys(tx, M::ID).await?;

    query(
        "UPDATE _musq_modules
         SET schema_version = ?,
             installed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             meta_json = ?
         WHERE module_id = ?",
    )
    .bind(i64::from(M::SCHEMA_VERSION))
    .bind(meta_json)
    .bind(M::ID)
    .execute(&**tx)
    .await?;

    load_module_in_tx(tx, M::ID).await?.ok_or_else(|| {
        Error::Protocol(format!(
            "module `{}` missing from `_musq_modules` immediately after migration",
            M::ID
        ))
    })
}

/// Verify that every dependency in `M::DEPENDS_ON` is already present in
/// `installed`.
fn check_dependencies<M: Module>(installed: &[ModuleRecord]) -> Result<()> {
    for &dep_id in M::DEPENDS_ON {
        let present = dep_id != M::ID && installed.iter().any(|record| record.module_id == dep_id);
        if !present {
            return Err(Error::Module(ModuleError::DependencyMissing {
                module_id: M::ID.to_owned(),
                dependency_id: dep_id.to_owned(),
            }));
        }
    }
    Ok(())
}

/// Verify that no other module already owns an overlapping `TABLE_PREFIX`.
fn check_prefix_conflict<M: Module>(installed: &[ModuleRecord]) -> Result<()> {
    for record in installed {
        if record.module_id != M::ID && prefixes_overlap(&record.table_prefix, M::TABLE_PREFIX) {
            return Err(Error::Module(ModuleError::PrefixConflict {
                module_id: M::ID.to_owned(),
                prefix: M::TABLE_PREFIX.to_owned(),
                existing_module_id: record.module_id.clone(),
            }));
        }
    }
    Ok(())
}

/// Run `PRAGMA foreign_key_check` inside `tx` and return a typed error if any
/// violations exist.
async fn check_foreign_keys(tx: &Transaction<&mut Connection>, module_id: &str) -> Result<()> {
    let violations = tx.foreign_key_check().await?;
    if !violations.is_empty() {
        return Err(Error::Module(ModuleError::ForeignKeyViolations {
            module_id: module_id.to_owned(),
            violations,
        }));
    }
    Ok(())
}

/// Snapshot all rows of `sqlite_schema` (including `sql`) inside `tx` in
/// deterministic order (tables first so cascading index/trigger removals report
/// the parent table first).
async fn snapshot_schema(tx: &Transaction<&mut Connection>) -> Result<Vec<SchemaObject>> {
    let rows: Vec<(String, String, String, Option<String>)> = query_as(
        "SELECT type, name, tbl_name, sql
         FROM sqlite_schema
         ORDER BY CASE type
             WHEN 'table' THEN 0
             WHEN 'view' THEN 1
             WHEN 'index' THEN 2
             WHEN 'trigger' THEN 3
             ELSE 4
         END, name ASC",
    )
    .fetch_all(&**tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(obj_type, name, tbl_name, sql)| SchemaObject {
            obj_type,
            name,
            tbl_name,
            sql,
        })
        .collect())
}

/// Verify that every added, modified, or removed `sqlite_schema` object
/// respects `prefix` and target-table ownership.
fn verify_schema_changes(
    module_id: &str,
    prefix: &str,
    before: &[SchemaObject],
    after: &[SchemaObject],
) -> Result<()> {
    let before_map: HashMap<(String, String), &SchemaObject> = before
        .iter()
        .map(|obj| (schema_object_key(obj), obj))
        .collect();
    let after_map: HashMap<(String, String), &SchemaObject> = after
        .iter()
        .map(|obj| (schema_object_key(obj), obj))
        .collect();

    for obj in after {
        let key = schema_object_key(obj);
        match before_map.get(&key) {
            None => check_object_ownership(module_id, prefix, obj, "created")?,
            Some(prev) if *prev != obj => {
                check_object_ownership(module_id, prefix, prev, "modified")?;
                check_object_ownership(module_id, prefix, obj, "modified")?;
            }
            Some(_) => {}
        }
    }

    for prev in before {
        let key = schema_object_key(prev);
        if !after_map.contains_key(&key) {
            check_object_ownership(module_id, prefix, prev, "removed")?;
        }
    }

    Ok(())
}

/// Canonical case-insensitive lookup key `(type, name)` for `sqlite_schema`.
fn schema_object_key(obj: &SchemaObject) -> (String, String) {
    (
        obj.obj_type.to_ascii_lowercase(),
        obj.name.to_ascii_lowercase(),
    )
}

/// Verify that `obj` belongs to `prefix` (or is an allowed SQLite-internal
/// object).
fn check_object_ownership(
    module_id: &str,
    prefix: &str,
    obj: &SchemaObject,
    change: &str,
) -> Result<()> {
    if obj.name.eq_ignore_ascii_case("sqlite_sequence") {
        return Ok(());
    }
    if starts_with_ascii_ci(&obj.name, "sqlite_autoindex_") {
        if !starts_with_ascii_ci(&obj.tbl_name, prefix) {
            return Err(schema_violation(
                module_id,
                obj,
                format!(
                    "{change} autoindex targeting foreign table `{}` outside prefix `{prefix}`",
                    obj.tbl_name
                ),
            ));
        }
        return Ok(());
    }

    let view_prefix = format!("v_{prefix}");
    match obj.obj_type.as_str() {
        "table" => {
            if !starts_with_ascii_ci(&obj.name, prefix) {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!("{change} table outside prefix `{prefix}`"),
                ));
            }
        }
        "index" | "trigger" => {
            if !starts_with_ascii_ci(&obj.name, prefix) {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!("{change} {} outside prefix `{prefix}`", obj.obj_type),
                ));
            }
            if !starts_with_ascii_ci(&obj.tbl_name, prefix) {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!(
                        "{change} {} targeting foreign table `{}` outside prefix `{prefix}`",
                        obj.obj_type, obj.tbl_name
                    ),
                ));
            }
        }
        "view" => {
            if !starts_with_ascii_ci(&obj.name, prefix)
                && !starts_with_ascii_ci(&obj.name, &view_prefix)
            {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!("{change} view outside `{prefix}` or `{view_prefix}`"),
                ));
            }
        }
        _ => {
            if !starts_with_ascii_ci(&obj.name, prefix) {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!("{change} object outside prefix `{prefix}`"),
                ));
            }
        }
    }

    Ok(())
}

/// Construct a [`ModuleError::SchemaViolation`] for `obj`.
fn schema_violation(module_id: &str, obj: &SchemaObject, reason: String) -> Error {
    Error::Module(ModuleError::SchemaViolation {
        module_id: module_id.to_owned(),
        object_type: obj.obj_type.clone(),
        object_name: obj.name.clone(),
        target_table: Some(obj.tbl_name.clone()),
        reason,
    })
}

/// Return `true` when either prefix is an ASCII case-insensitive prefix of the
/// other.
fn prefixes_overlap(left: &str, right: &str) -> bool {
    starts_with_ascii_ci(left, right) || starts_with_ascii_ci(right, left)
}

/// Return `true` when `value` starts with `prefix`, comparing ASCII bytes
/// case-insensitively.
fn starts_with_ascii_ci(value: &str, prefix: &str) -> bool {
    value
        .as_bytes()
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix.as_bytes()))
}

/// Load all installed modules from `_musq_modules` using `pool`.
///
/// Returns an empty list if `_musq_modules` does not exist yet (for example on
/// a read-only legacy database).
pub async fn list_modules_from_pool(pool: &Pool) -> Result<Vec<ModuleRecord>> {
    if !modules_table_exists(pool).await? {
        return Ok(Vec::new());
    }
    load_modules_from_pool(pool).await
}

/// Load all rows of `_musq_modules` using `pool`. The table must exist, as it
/// does on each writable host.
async fn load_modules_from_pool(pool: &Pool) -> Result<Vec<ModuleRecord>> {
    let rows: Vec<RawModuleRow> = query_as(
        "SELECT module_id, table_prefix, schema_version, installed_at, meta_json
         FROM _musq_modules
         ORDER BY module_id ASC",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(decode_module_row).collect()
}

/// Load one module record by `module_id` from `pool`, returning `None` if
/// `_musq_modules` or the row does not exist.
pub async fn load_module_from_pool(pool: &Pool, module_id: &str) -> Result<Option<ModuleRecord>> {
    if !modules_table_exists(pool).await? {
        return Ok(None);
    }
    let row: Option<RawModuleRow> = query_as(
        "SELECT module_id, table_prefix, schema_version, installed_at, meta_json
         FROM _musq_modules
         WHERE module_id = ?",
    )
    .bind(module_id)
    .fetch_optional(pool)
    .await?;
    row.map(decode_module_row).transpose()
}

/// Verify that module `M` is installed in `pool` with matching prefix and
/// schema version.
pub async fn verify_module_from_pool<M: Module>(pool: &Pool) -> Result<ModuleRecord> {
    let Some(record) = load_module_from_pool(pool, M::ID).await? else {
        return Err(Error::Module(ModuleError::NotInstalled {
            module_id: M::ID.to_owned(),
        }));
    };
    if !record.table_prefix.eq_ignore_ascii_case(M::TABLE_PREFIX) {
        return Err(Error::Module(ModuleError::PrefixMismatch {
            module_id: M::ID.to_owned(),
            installed_prefix: record.table_prefix,
            expected_prefix: M::TABLE_PREFIX.to_owned(),
        }));
    }
    if record.schema_version != M::SCHEMA_VERSION {
        return Err(Error::Module(ModuleError::VersionMismatch {
            module_id: M::ID.to_owned(),
            installed_version: record.schema_version,
            expected_version: M::SCHEMA_VERSION,
        }));
    }
    Ok(record)
}

/// Return `true` when `_musq_modules` exists in `sqlite_schema`.
async fn modules_table_exists(pool: &Pool) -> Result<bool> {
    let count: i64 = query_scalar(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = '_musq_modules'",
    )
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// Load all rows from `_musq_modules` inside an active write transaction.
async fn load_modules_in_tx(tx: &Transaction<&mut Connection>) -> Result<Vec<ModuleRecord>> {
    let rows: Vec<RawModuleRow> = query_as(
        "SELECT module_id, table_prefix, schema_version, installed_at, meta_json
         FROM _musq_modules
         ORDER BY module_id ASC",
    )
    .fetch_all(&**tx)
    .await?;
    rows.into_iter().map(decode_module_row).collect()
}

/// Load one row from `_musq_modules` inside an active write transaction.
async fn load_module_in_tx(
    tx: &Transaction<&mut Connection>,
    module_id: &str,
) -> Result<Option<ModuleRecord>> {
    let row: Option<RawModuleRow> = query_as(
        "SELECT module_id, table_prefix, schema_version, installed_at, meta_json
         FROM _musq_modules
         WHERE module_id = ?",
    )
    .bind(module_id)
    .fetch_optional(&**tx)
    .await?;
    row.map(decode_module_row).transpose()
}

/// Decode one `_musq_modules` row into a [`ModuleRecord`].
fn decode_module_row(row: RawModuleRow) -> Result<ModuleRecord> {
    let (module_id, table_prefix, schema_version_raw, installed_at, meta_json) = row;
    let schema_version = u32::try_from(schema_version_raw).map_err(|_| {
        Error::Protocol(format!(
            "module `{module_id}` has invalid negative or out-of-range schema_version {schema_version_raw}"
        ))
    })?;
    Ok(ModuleRecord {
        module_id,
        table_prefix,
        schema_version,
        installed_at,
        meta_json,
    })
}
