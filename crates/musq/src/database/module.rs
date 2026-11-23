//! Composable schema modules, `_musq_modules` registry, prefix ownership
//! verification, migrations, and read-only module discovery.

use std::{collections::HashSet, future::Future};

use super::surface::QuerySurface;
use crate::{Connection, Error, ModuleError, Pool, Result, Transaction, query, query_as, query_scalar};

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
    fn install(
        tx: &mut Transaction<&mut Connection>,
    ) -> impl Future<Output = Result<()>> + Send;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SchemaObject {
    /// SQLite object type (`table`, `index`, `view`, or `trigger`).
    obj_type: String,
    /// SQLite object name.
    name: String,
    /// Owning table name (`sqlite_schema.tbl_name`).
    tbl_name: String,
}

/// Raw row fetched from `_musq_modules`.
type RawModuleRow = (String, String, i64, String, String);

/// Validate `M::ID` and `M::TABLE_PREFIX` before starting a mount transaction.
pub(crate) fn validate_module_identity(module_id: &str, prefix: &str) -> Result<()> {
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
    let lower = prefix.to_ascii_lowercase();
    if lower.starts_with("_musq_") {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "prefix `_musq_` is reserved for host metadata".to_owned(),
        }));
    }
    if lower.starts_with("sqlite_") {
        return Err(Error::Module(ModuleError::InvalidPrefix {
            module_id: module_id.to_owned(),
            prefix: prefix.to_owned(),
            reason: "prefix `sqlite_` is reserved by SQLite".to_owned(),
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

/// Execute module mount (fresh install, idempotent check, or upgrade) inside an
/// active `BEGIN IMMEDIATE` transaction.
pub(crate) async fn mount_in_tx<M: Module>(
    tx: &mut Transaction<&mut Connection>,
    meta_json: &str,
) -> Result<ModuleRecord> {
    let installed = load_modules_in_tx(tx).await?;
    check_dependencies::<M>(&installed)?;
    check_prefix_conflict::<M>(&installed)?;

    if let Some(existing) = installed.into_iter().find(|rec| rec.module_id == M::ID) {
        if !existing.table_prefix.eq_ignore_ascii_case(M::TABLE_PREFIX) {
            return Err(Error::Module(ModuleError::PrefixMismatch {
                module_id: M::ID.to_owned(),
                installed_prefix: existing.table_prefix,
                expected_prefix: M::TABLE_PREFIX.to_owned(),
            }));
        }
        if existing.schema_version == M::SCHEMA_VERSION {
            return Ok(existing);
        }
        if existing.schema_version > M::SCHEMA_VERSION {
            return Err(Error::Module(ModuleError::Downgrade {
                module_id: M::ID.to_owned(),
                installed_version: existing.schema_version,
                target_version: M::SCHEMA_VERSION,
            }));
        }
        return upgrade_module_in_tx::<M>(tx, existing.schema_version, meta_json).await;
    }

    install_module_in_tx::<M>(tx, meta_json).await
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

    verify_new_schema_objects(M::ID, M::TABLE_PREFIX, &before, &after)?;
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

    verify_new_schema_objects(M::ID, M::TABLE_PREFIX, &before, &after)?;
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
        let present =
            dep_id != M::ID && installed.iter().any(|record| record.module_id == dep_id);
        if !present {
            return Err(Error::Module(ModuleError::DependencyMissing {
                module_id: M::ID.to_owned(),
                dependency_id: dep_id.to_owned(),
            }));
        }
    }
    Ok(())
}

/// Verify that no other module already owns `M::TABLE_PREFIX`.
fn check_prefix_conflict<M: Module>(installed: &[ModuleRecord]) -> Result<()> {
    for record in installed {
        if record.module_id != M::ID && record.table_prefix.eq_ignore_ascii_case(M::TABLE_PREFIX) {
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

/// Snapshot all rows of `sqlite_schema` inside `tx`.
async fn snapshot_schema(tx: &Transaction<&mut Connection>) -> Result<Vec<SchemaObject>> {
    let rows: Vec<(String, String, String)> =
        query_as("SELECT type, name, tbl_name FROM sqlite_schema")
            .fetch_all(&**tx)
            .await?;
    Ok(rows
        .into_iter()
        .map(|(obj_type, name, tbl_name)| SchemaObject {
            obj_type,
            name,
            tbl_name,
        })
        .collect())
}

/// Verify that every object present in `after` but not in `before` respects
/// `prefix` and target-table ownership.
fn verify_new_schema_objects(
    module_id: &str,
    prefix: &str,
    before: &[SchemaObject],
    after: &[SchemaObject],
) -> Result<()> {
    let before_set: HashSet<&SchemaObject> = before.iter().collect();
    let view_prefix = format!("v_{prefix}");

    for obj in after {
        if before_set.contains(obj) {
            continue;
        }
        if obj.name.eq_ignore_ascii_case("sqlite_sequence") {
            continue;
        }
        if starts_with_ascii_ci(&obj.name, "sqlite_autoindex_") {
            if !starts_with_ascii_ci(&obj.tbl_name, prefix) {
                return Err(schema_violation(
                    module_id,
                    obj,
                    format!(
                        "autoindex targets table `{}` outside prefix `{prefix}`",
                        obj.tbl_name
                    ),
                ));
            }
            continue;
        }

        match obj.obj_type.as_str() {
            "table" => {
                if !starts_with_ascii_ci(&obj.name, prefix) {
                    return Err(schema_violation(
                        module_id,
                        obj,
                        format!("table name must start with `{prefix}`"),
                    ));
                }
            }
            "index" | "trigger" => {
                if !starts_with_ascii_ci(&obj.name, prefix) {
                    return Err(schema_violation(
                        module_id,
                        obj,
                        format!("{} name must start with `{prefix}`", obj.obj_type),
                    ));
                }
                if !starts_with_ascii_ci(&obj.tbl_name, prefix) {
                    return Err(schema_violation(
                        module_id,
                        obj,
                        format!(
                            "{} targets foreign table `{}` outside prefix `{prefix}`",
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
                        format!("view name must start with `{prefix}` or `{view_prefix}`"),
                    ));
                }
            }
            _ => {
                if !starts_with_ascii_ci(&obj.name, prefix) {
                    return Err(schema_violation(
                        module_id,
                        obj,
                        format!("object name must start with `{prefix}`"),
                    ));
                }
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
pub(crate) async fn list_modules_from_pool(pool: &Pool) -> Result<Vec<ModuleRecord>> {
    if !modules_table_exists(pool).await? {
        return Ok(Vec::new());
    }
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
pub(crate) async fn load_module_from_pool(
    pool: &Pool,
    module_id: &str,
) -> Result<Option<ModuleRecord>> {
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
pub(crate) async fn verify_module_from_pool<M: Module>(pool: &Pool) -> Result<ModuleRecord> {
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
