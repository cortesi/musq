//! Integration tests for `Module`, `_musq_modules`, prefix/table ownership
//! verification, migrations with foreign-key checks, and read-only discovery.

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use musq::{
        Connection, Database, DatabaseOptions, Error, Module, ModuleError, ModuleRecord, Musq,
        QuerySurface, Result, Transaction, TransactionBehavior, query, query_scalar,
    };
    use tempfile::tempdir;
    use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

    /// Base module A owning prefix `a_`.
    struct ModA;

    impl Module for ModA {
        const ID: &'static str = "test.mod_a";
        const TABLE_PREFIX: &'static str = "a_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TABLE a_items (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    slug TEXT NOT NULL UNIQUE
                 )",
            )
            .execute(&**tx)
            .await?;
            query("CREATE INDEX a_items_slug_idx ON a_items (slug)")
                .execute(&**tx)
                .await?;
            query("CREATE VIEW v_a_items AS SELECT id, slug FROM a_items")
                .execute(&**tx)
                .await?;
            Ok(())
        }

        fn meta_json(&self) -> String {
            r#"{"owner":"core"}"#.to_owned()
        }

        fn query_surface(&self) -> QuerySurface {
            QuerySurface::new().with_allowed_view("v_a_items")
        }
    }

    /// Dependent module B v1 owning `b_` with a downward FK into `a_items`.
    struct ModBV1;

    impl Module for ModBV1 {
        const ID: &'static str = "test.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 1;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TABLE b_notes (
                    id INTEGER PRIMARY KEY,
                    item_id INTEGER NOT NULL REFERENCES a_items(id),
                    body TEXT NOT NULL
                 )",
            )
            .execute(&**tx)
            .await?;
            query(
                "CREATE TRIGGER b_notes_check BEFORE INSERT ON b_notes
                 BEGIN
                     SELECT CASE WHEN length(NEW.body) = 0 THEN RAISE(ABORT, 'empty body') END;
                 END",
            )
            .execute(&**tx)
            .await?;
            Ok(())
        }
    }

    /// Upgraded module B v2 rebuilding `b_notes` with `defer_foreign_keys`.
    struct ModBV2;

    impl Module for ModBV2 {
        const ID: &'static str = "test.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 2;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TABLE b_notes (
                    id INTEGER PRIMARY KEY,
                    item_id INTEGER NOT NULL REFERENCES a_items(id),
                    body TEXT NOT NULL,
                    tag TEXT NOT NULL DEFAULT 'general'
                 )",
            )
            .execute(&**tx)
            .await?;
            Ok(())
        }

        async fn migrate(tx: &mut Transaction<&mut Connection>, from_version: u32) -> Result<()> {
            assert_eq!(from_version, 1);
            // Rebuild table while preserving FK references (uses
            // defer_foreign_keys).
            query(
                "CREATE TABLE b_notes_new (
                    id INTEGER PRIMARY KEY,
                    item_id INTEGER NOT NULL REFERENCES a_items(id),
                    body TEXT NOT NULL,
                    tag TEXT NOT NULL DEFAULT 'migrated'
                 )",
            )
            .execute(&**tx)
            .await?;
            query(
                "INSERT INTO b_notes_new (id, item_id, body) SELECT id, item_id, body FROM b_notes",
            )
            .execute(&**tx)
            .await?;
            query("DROP TABLE b_notes").execute(&**tx).await?;
            query("ALTER TABLE b_notes_new RENAME TO b_notes")
                .execute(&**tx)
                .await?;
            query("CREATE INDEX b_notes_tag_idx ON b_notes (tag)")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module B v3 with default unimplemented `migrate()`.
    struct ModBV3NoMigrate;

    impl Module for ModBV3NoMigrate {
        const ID: &'static str = "test.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 3;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module B v3 whose migration leaves a dangling foreign key.
    struct ModBV3BrokenFk;

    impl Module for ModBV3BrokenFk {
        const ID: &'static str = "test.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 3;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }

        async fn migrate(tx: &mut Transaction<&mut Connection>, _from_version: u32) -> Result<()> {
            // `defer_foreign_keys = ON` allows the insert inside tx, then
            // post-migration `PRAGMA foreign_key_check` must catch and roll
            // back.
            query("INSERT INTO b_notes (id, item_id, body) VALUES (999, 99999, 'orphan')")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module with conflicting table prefix `a_`.
    struct ModConflictPrefix;

    impl Module for ModConflictPrefix {
        const ID: &'static str = "test.conflict";
        const TABLE_PREFIX: &'static str = "a_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("CREATE TABLE a_other (id INTEGER PRIMARY KEY)")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module using reserved `_musq_` prefix.
    struct ModReservedPrefix;

    impl Module for ModReservedPrefix {
        const ID: &'static str = "test.reserved";
        const TABLE_PREFIX: &'static str = "_musq_custom_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module creating an unprefixed table.
    struct ModUnprefixedTable;

    impl Module for ModUnprefixedTable {
        const ID: &'static str = "test.bad_table";
        const TABLE_PREFIX: &'static str = "bt_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("CREATE TABLE rogue_table (id INTEGER PRIMARY KEY)")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module creating a prefixed index targeting another module's table.
    struct ModForeignIndex;

    impl Module for ModForeignIndex {
        const ID: &'static str = "test.foreign_idx";
        const TABLE_PREFIX: &'static str = "fi_";
        const SCHEMA_VERSION: u32 = 1;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("CREATE INDEX fi_steal_idx ON a_items (slug)")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module creating a prefixed trigger targeting another module's table.
    struct ModForeignTrigger;

    impl Module for ModForeignTrigger {
        const ID: &'static str = "test.foreign_trg";
        const TABLE_PREFIX: &'static str = "ft_";
        const SCHEMA_VERSION: u32 = 1;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TRIGGER ft_steal_trg AFTER INSERT ON a_items
                 BEGIN SELECT 1; END",
            )
            .execute(&**tx)
            .await?;
            Ok(())
        }
    }

    /// Module creating a view without `TABLE_PREFIX` or `v_<TABLE_PREFIX>`.
    struct ModBadView;

    impl Module for ModBadView {
        const ID: &'static str = "test.bad_view";
        const TABLE_PREFIX: &'static str = "bv_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("CREATE TABLE bv_rows (id INTEGER PRIMARY KEY)")
                .execute(&**tx)
                .await?;
            query("CREATE VIEW v_other_rows AS SELECT id FROM bv_rows")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn dag_dependencies_migrations_idempotency_downgrade_and_discovery() -> anyhow::Result<()>
    {
        let dir = tempdir()?;
        let db_path = dir.path().join("app.db");

        // Host uses its own arbitrary application_id and user_version.
        let db = Database::open(
            &db_path,
            DatabaseOptions::new()
                .application_id(0x4150_5031)
                .user_version(12),
        )
        .await?;

        // Mounting B before A fails with DependencyMissing.
        let dep_err = db.mount(ModBV1).await.unwrap_err();
        assert!(matches!(
            dep_err,
            Error::Module(ModuleError::DependencyMissing {
                ref module_id,
                ref dependency_id,
            }) if module_id == "test.mod_b" && dependency_id == "test.mod_a"
        ));

        // Mount A then B v1.
        let rec_a = db.mount(ModA).await?;
        assert_eq!(rec_a.module_id, "test.mod_a");
        assert_eq!(rec_a.table_prefix, "a_");
        assert_eq!(rec_a.schema_version, 1);
        assert_eq!(rec_a.meta_json, r#"{"owner":"core"}"#);
        assert!(rec_a.installed_at.ends_with('Z'));

        let rec_b1 = db.mount(ModBV1).await?;
        assert_eq!(rec_b1.schema_version, 1);

        // Idempotent remount at same version succeeds.
        let rec_a_again = db.mount(ModA).await?;
        assert_eq!(rec_a_again.installed_at, rec_a.installed_at);

        // Insert cross-module data with downward FK from b_notes -> a_items.
        db.write(async |tx| {
            query("INSERT INTO a_items (id, slug) VALUES (10, 'alpha')")
                .execute(&**tx)
                .await?;
            query("INSERT INTO b_notes (id, item_id, body) VALUES (1, 10, 'linked')")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        // Upgrade B from v1 -> v2.
        let rec_b2 = db.mount(ModBV2).await?;
        assert_eq!(rec_b2.schema_version, 2);
        let tag: String = query_scalar("SELECT tag FROM b_notes WHERE id = 1")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(tag, "migrated");

        // Downgrade B from v2 -> v1 is rejected.
        let downgrade_err = db.mount(ModBV1).await.unwrap_err();
        assert!(matches!(
            downgrade_err,
            Error::Module(ModuleError::Downgrade {
                installed_version: 2,
                target_version: 1,
                ..
            })
        ));

        // Default migrate() returns MigrationUnsupported.
        let unsup_err = db.mount(ModBV3NoMigrate).await.unwrap_err();
        assert!(matches!(
            unsup_err,
            Error::Module(ModuleError::MigrationUnsupported {
                from_version: 2,
                to_version: 3,
                ..
            })
        ));

        // Broken FK migration is caught by PRAGMA foreign_key_check and rolled
        // back.
        let fk_err = db.mount(ModBV3BrokenFk).await.unwrap_err();
        assert!(matches!(
            fk_err,
            Error::Module(ModuleError::ForeignKeyViolations { ref violations, .. })
                if !violations.is_empty()
        ));
        let note_count: i64 = query_scalar("SELECT COUNT(*) FROM b_notes")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(note_count, 1);
        assert_eq!(db.verify_module::<ModBV2>().await?.schema_version, 2);

        db.close().await?;

        // Read-only discovery succeeds regardless of host
        // application_id/user_version.
        let ro = Database::open_read_only(&db_path, false).await?;
        let modules = ro.mounted_modules().await?;
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].module_id, "test.mod_a");
        assert_eq!(modules[1].module_id, "test.mod_b");

        let verified_a = ro.verify_module::<ModA>().await?;
        assert_eq!(verified_a.table_prefix, "a_");
        let verified_b = ro.verify_module::<ModBV2>().await?;
        assert_eq!(verified_b.schema_version, 2);

        // Version mismatch on verify_module returns VersionMismatch.
        let ver_err = ro.verify_module::<ModBV1>().await.unwrap_err();
        assert!(matches!(
            ver_err,
            Error::Module(ModuleError::VersionMismatch {
                installed_version: 2,
                expected_version: 1,
                ..
            })
        ));

        // Uninstalled module returns NotInstalled.
        let missing_err = ro.verify_module::<ModBadView>().await.unwrap_err();
        assert!(matches!(
            missing_err,
            Error::Module(ModuleError::NotInstalled { .. })
        ));

        // Mounting on read-only DB returns ReadOnly.
        let ro_mount_err = ro.mount(ModA).await.unwrap_err();
        assert!(matches!(ro_mount_err, Error::ReadOnly));

        ro.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn prefix_and_schema_ownership_rules_are_enforced_and_rolled_back() -> anyhow::Result<()>
    {
        let dir = tempdir()?;
        let db_path = dir.path().join("ownership.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        db.mount(ModA).await?;

        // Conflicting table prefix rejected.
        let conflict = db.mount(ModConflictPrefix).await.unwrap_err();
        assert!(matches!(
            conflict,
            Error::Module(ModuleError::PrefixConflict {
                ref existing_module_id,
                ..
            }) if existing_module_id == "test.mod_a"
        ));

        // Reserved `_musq_` prefix rejected.
        let reserved = db.mount(ModReservedPrefix).await.unwrap_err();
        assert!(matches!(
            reserved,
            Error::Module(ModuleError::InvalidPrefix { .. })
        ));

        // Unprefixed table rejected and rolled back.
        let bad_table = db.mount(ModUnprefixedTable).await.unwrap_err();
        assert!(matches!(
            bad_table,
            Error::Module(ModuleError::SchemaViolation {
                ref object_type,
                ref object_name,
                ..
            }) if object_type == "table" && object_name == "rogue_table"
        ));
        let rogue_exists: i64 =
            query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'rogue_table'")
                .fetch_one(db.pool())
                .await?;
        assert_eq!(rogue_exists, 0);

        // Foreign index targeting another module's table rejected and rolled
        // back.
        let foreign_idx = db.mount(ModForeignIndex).await.unwrap_err();
        assert!(matches!(
            foreign_idx,
            Error::Module(ModuleError::SchemaViolation {
                ref object_type,
                ref object_name,
                target_table: Some(ref tbl),
                ..
            }) if object_type == "index" && object_name == "fi_steal_idx" && tbl == "a_items"
        ));
        let idx_exists: i64 =
            query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'fi_steal_idx'")
                .fetch_one(db.pool())
                .await?;
        assert_eq!(idx_exists, 0);

        // Foreign trigger targeting another module's table rejected and rolled
        // back.
        let foreign_trg = db.mount(ModForeignTrigger).await.unwrap_err();
        assert!(matches!(
            foreign_trg,
            Error::Module(ModuleError::SchemaViolation {
                ref object_type,
                ref object_name,
                target_table: Some(ref tbl),
                ..
            }) if object_type == "trigger" && object_name == "ft_steal_trg" && tbl == "a_items"
        ));
        let trg_exists: i64 =
            query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'ft_steal_trg'")
                .fetch_one(db.pool())
                .await?;
        assert_eq!(trg_exists, 0);

        // View without prefix or v_<prefix> rejected and rolled back.
        let bad_view = db.mount(ModBadView).await.unwrap_err();
        assert!(matches!(
            bad_view,
            Error::Module(ModuleError::SchemaViolation {
                ref object_type,
                ref object_name,
                ..
            }) if object_type == "view" && object_name == "v_other_rows"
        ));
        let bv_exists: i64 =
            query_scalar("SELECT COUNT(*) FROM sqlite_schema WHERE name = 'bv_rows'")
                .fetch_one(db.pool())
                .await?;
        assert_eq!(bv_exists, 0);

        // Only ModA remains registered.
        let mounted = db.mounted_modules().await?;
        assert_eq!(mounted.len(), 1);
        assert_eq!(mounted[0].module_id, "test.mod_a");

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn read_only_discovery_on_legacy_db_without_registry_table() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let legacy_path = dir.path().join("legacy.db");
        let pool = musq::Musq::new()
            .create_if_missing(true)
            .open(&legacy_path)
            .await?;
        query("CREATE TABLE plain (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await?;
        let () = pool.close().await;

        let ro = Database::open_read_only(&legacy_path, false).await?;
        assert!(ro.mounted_modules().await?.is_empty());
        assert!(ro.module_record(ModA::ID).await?.is_none());
        let err = ro.verify_module::<ModA>().await.unwrap_err();
        assert!(matches!(
            err,
            Error::Module(ModuleError::NotInstalled { .. })
        ));
        ro.close().await?;
        Ok(())
    }

    /// Holds the writer of `db` in one job until the returned sender fires.
    async fn hold_writer(db: &Database) -> (oneshot::Sender<()>, JoinHandle<Result<()>>) {
        let (held_tx, held_rx) = oneshot::channel::<()>();
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let writer = db.clone();
        let hold = tokio::spawn(async move {
            writer
                .write(async move |_tx| {
                    held_tx.send(()).ok();
                    release_rx.await.ok();
                    Ok::<_, Error>(())
                })
                .await
        });
        held_rx.await.expect("the writer runs the held job");
        (release_tx, hold)
    }

    /// Spawns a mount of `M` on `db`, and gives it two read-pool round trips
    /// to finish when it needs no writer job.
    async fn spawn_mount<M: Module + Send + 'static>(
        db: &Database,
        module: M,
    ) -> anyhow::Result<JoinHandle<Result<ModuleRecord>>> {
        let mounter = db.clone();
        let mount = tokio::spawn(async move { mounter.mount(module).await });
        db.mounted_modules().await?;
        db.mounted_modules().await?;
        Ok(mount)
    }

    #[tokio::test]
    async fn done_mount_queues_no_writer_job() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db = Database::open(dir.path().join("app.db"), DatabaseOptions::new()).await?;
        let first = db.mount(ModA).await?;

        // Hold the writer in one job: a mount that is done must not queue
        // behind it. The bound only turns a wait on the held writer into a
        // failure.
        let (release, hold) = hold_writer(&db).await;
        let again = timeout(Duration::from_secs(10), db.mount(ModA))
            .await
            .expect("a done mount waited for the writer")?;
        assert_eq!(again, first);

        release.send(()).ok();
        hold.await??;
        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn first_mount_waits_for_the_writer() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db = Database::open(dir.path().join("app.db"), DatabaseOptions::new()).await?;

        let (release, hold) = hold_writer(&db).await;
        let mount = spawn_mount(&db, ModA).await?;
        assert!(!mount.is_finished(), "an install waits for the writer");

        release.send(()).ok();
        hold.await??;
        assert_eq!(mount.await??.module_id, ModA::ID);
        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn refused_mount_waits_for_the_writer_and_returns_its_refusal() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db = Database::open(dir.path().join("app.db"), DatabaseOptions::new()).await?;
        db.mount(ModA).await?;
        db.mount(ModBV1).await?;
        db.mount(ModBV2).await?;

        let (release, hold) = hold_writer(&db).await;
        let mount = spawn_mount(&db, ModBV1).await?;
        assert!(!mount.is_finished(), "a refusal waits for the writer");

        release.send(()).ok();
        hold.await??;
        let err = mount.await?.unwrap_err();
        assert!(matches!(
            err,
            Error::Module(ModuleError::Downgrade {
                installed_version: 2,
                target_version: 1,
                ..
            })
        ));
        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn mount_waits_for_an_install_that_has_not_committed() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let path = dir.path().join("app.db");
        let db = Database::open(&path, DatabaseOptions::new()).await?;

        // A second connection installs `ModA` and holds its transaction open.
        let other = Musq::new().open(&path).await?;
        let tx = other.begin_with(TransactionBehavior::Immediate).await?;
        query("CREATE TABLE a_items (id INTEGER PRIMARY KEY, slug TEXT NOT NULL UNIQUE)")
            .execute(&tx)
            .await?;
        query(
            "INSERT INTO _musq_modules (module_id, table_prefix, schema_version, installed_at, meta_json)
             VALUES ('test.mod_a', 'a_', 1, '2026-01-01T00:00:00Z', '{\"by\":\"other\"}')",
        )
        .execute(&tx)
        .await?;
        assert!(
            db.mounted_modules().await?.is_empty(),
            "the read pool does not see an install that has not committed"
        );

        let mount = spawn_mount(&db, ModA).await?;
        assert!(!mount.is_finished(), "the mount waits for the install");
        tx.commit().await?;

        let record = mount.await??;
        assert_eq!(record.installed_at, "2026-01-01T00:00:00Z");
        assert_eq!(record.meta_json, r#"{"by":"other"}"#);
        let () = other.close().await;
        db.close().await?;
        Ok(())
    }

    /// Module attempting to `DROP` another module's table during install.
    struct ModDropForeignTable;

    impl Module for ModDropForeignTable {
        const ID: &'static str = "test.drop_foreign";
        const TABLE_PREFIX: &'static str = "df_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("DROP TABLE a_items").execute(&**tx).await?;
            Ok(())
        }
    }

    /// Module attempting to `ALTER` another module's table during install.
    struct ModAlterForeignTable;

    impl Module for ModAlterForeignTable {
        const ID: &'static str = "test.alter_foreign";
        const TABLE_PREFIX: &'static str = "af_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("ALTER TABLE a_items ADD COLUMN hacked TEXT")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module attempting to `DROP` `_musq_modules` during install.
    struct ModDropRegistry;

    impl Module for ModDropRegistry {
        const ID: &'static str = "test.drop_registry";
        const TABLE_PREFIX: &'static str = "dr_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("DROP TABLE _musq_modules").execute(&**tx).await?;
            Ok(())
        }
    }

    /// Clean v1 module used to test malicious `migrate()`.
    struct ModCleanV1;

    impl Module for ModCleanV1 {
        const ID: &'static str = "test.clean_mod";
        const TABLE_PREFIX: &'static str = "cm_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query("CREATE TABLE cm_rows (id INTEGER PRIMARY KEY)")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Malicious v2 migration attempting to `ALTER` `_musq_modules`.
    struct ModAlterRegistryV2;

    impl Module for ModAlterRegistryV2 {
        const ID: &'static str = "test.clean_mod";
        const TABLE_PREFIX: &'static str = "cm_";
        const SCHEMA_VERSION: u32 = 2;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }

        async fn migrate(tx: &mut Transaction<&mut Connection>, _from_version: u32) -> Result<()> {
            query("ALTER TABLE _musq_modules ADD COLUMN extra TEXT")
                .execute(&**tx)
                .await?;
            Ok(())
        }
    }

    /// Module with prefix `"_musq"` (shorter prefix overlapping `"_musq_"`).
    struct ModOverlapMusq;

    impl Module for ModOverlapMusq {
        const ID: &'static str = "test.overlap_musq";
        const TABLE_PREFIX: &'static str = "_musq";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module with prefix `"_"` (overlapping `"_musq_"`).
    struct ModOverlapUnderscore;

    impl Module for ModOverlapUnderscore {
        const ID: &'static str = "test.overlap_under";
        const TABLE_PREFIX: &'static str = "_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module with prefix `"sqlite"` (shorter prefix overlapping `"sqlite_"`).
    struct ModOverlapSqlite;

    impl Module for ModOverlapSqlite {
        const ID: &'static str = "test.overlap_sqlite";
        const TABLE_PREFIX: &'static str = "sqlite";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module with prefix `"a"` (shorter prefix overlapping installed `"a_"`).
    struct ModShorterOverlapA;

    impl Module for ModShorterOverlapA {
        const ID: &'static str = "test.short_a";
        const TABLE_PREFIX: &'static str = "a";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    /// Module with prefix `"a_sub_"` (longer prefix overlapping installed
    /// `"a_"`).
    struct ModLongerOverlapA;

    impl Module for ModLongerOverlapA {
        const ID: &'static str = "test.long_a";
        const TABLE_PREFIX: &'static str = "a_sub_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(_tx: &mut Transaction<&mut Connection>) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn rejects_drop_or_alter_of_foreign_tables_and_registry() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("tamper.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        db.mount(ModA).await?;
        db.mount(ModCleanV1).await?;

        let orig_a_sql: String =
            query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'a_items'")
                .fetch_one(db.pool())
                .await?;
        let orig_reg_sql: String = query_scalar(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = '_musq_modules'",
        )
        .fetch_one(db.pool())
        .await?;

        // Dropping another module's table is rejected and rolled back.
        let drop_err = db.mount(ModDropForeignTable).await.unwrap_err();
        assert!(matches!(
            drop_err,
            Error::Module(ModuleError::SchemaViolation {
                ref object_name,
                ..
            }) if object_name == "a_items"
        ));

        // Altering another module's table is rejected and rolled back.
        let alter_err = db.mount(ModAlterForeignTable).await.unwrap_err();
        assert!(matches!(
            alter_err,
            Error::Module(ModuleError::SchemaViolation {
                ref object_name,
                ..
            }) if object_name == "a_items"
        ));

        // Dropping `_musq_modules` during install is rejected and rolled back.
        let drop_reg_err = db.mount(ModDropRegistry).await.unwrap_err();
        assert!(matches!(
            drop_reg_err,
            Error::Module(ModuleError::SchemaViolation {
                ref object_name,
                ..
            }) if object_name == "_musq_modules"
        ));

        // Altering `_musq_modules` during migrate is rejected and rolled back.
        let alter_reg_err = db.mount(ModAlterRegistryV2).await.unwrap_err();
        assert!(matches!(
            alter_reg_err,
            Error::Module(ModuleError::SchemaViolation {
                ref object_name,
                ..
            }) if object_name == "_musq_modules"
        ));

        // Both schemas and `_musq_modules` remain untouched.
        let after_a_sql: String =
            query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'a_items'")
                .fetch_one(db.pool())
                .await?;
        let after_reg_sql: String = query_scalar(
            "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = '_musq_modules'",
        )
        .fetch_one(db.pool())
        .await?;
        assert_eq!(after_a_sql, orig_a_sql);
        assert_eq!(after_reg_sql, orig_reg_sql);
        assert_eq!(db.verify_module::<ModCleanV1>().await?.schema_version, 1);

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn rejects_overlapping_prefixes_bidirectionally() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("overlap.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        db.mount(ModA).await?;

        // Reserved namespace overlaps (`_musq`, `_`, `sqlite`).
        assert!(matches!(
            db.mount(ModOverlapMusq).await.unwrap_err(),
            Error::Module(ModuleError::InvalidPrefix { .. })
        ));
        assert!(matches!(
            db.mount(ModOverlapUnderscore).await.unwrap_err(),
            Error::Module(ModuleError::InvalidPrefix { .. })
        ));
        assert!(matches!(
            db.mount(ModOverlapSqlite).await.unwrap_err(),
            Error::Module(ModuleError::InvalidPrefix { .. })
        ));

        // Bidirectional overlap with already-installed `a_` (`a` and `a_sub_`).
        assert!(matches!(
            db.mount(ModShorterOverlapA).await.unwrap_err(),
            Error::Module(ModuleError::PrefixConflict {
                ref existing_module_id,
                ..
            }) if existing_module_id == "test.mod_a"
        ));
        assert!(matches!(
            db.mount(ModLongerOverlapA).await.unwrap_err(),
            Error::Module(ModuleError::PrefixConflict {
                ref existing_module_id,
                ..
            }) if existing_module_id == "test.mod_a"
        ));

        db.close().await?;
        Ok(())
    }

    /// Module v2 that tolerates reading v1 archives while still migrating
    /// writable databases from v1 to v2.
    struct ModReadCompatV2;

    impl Module for ModReadCompatV2 {
        const ID: &'static str = "test.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 2;
        const MIN_READ_VERSION: u32 = 1;
        const DEPENDS_ON: &'static [&'static str] = &["test.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            ModBV2::install(tx).await
        }

        async fn migrate(tx: &mut Transaction<&mut Connection>, from_version: u32) -> Result<()> {
            ModBV2::migrate(tx, from_version).await
        }
    }

    #[tokio::test]
    async fn verify_module_respects_min_read_version_while_mount_still_migrates()
    -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("compat.db");

        // Install v1 and close.
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        db.mount(ModA).await?;
        db.mount(ModBV1).await?;
        db.close().await?;

        // Read-only verify_module accepts v1 when MIN_READ_VERSION == 1 and
        // reports installed version 1, while default exact-match ModBV2 still
        // rejects v1.
        let ro = Database::open_read_only(&db_path, false).await?;
        let compat = ro.verify_module::<ModReadCompatV2>().await?;
        assert_eq!(compat.schema_version, 1);
        let strict_err = ro.verify_module::<ModBV2>().await.unwrap_err();
        assert!(matches!(
            strict_err,
            Error::Module(ModuleError::VersionMismatch {
                installed_version: 1,
                expected_version: 2,
                ..
            })
        ));
        ro.close().await?;

        // Out-of-range versions (< MIN_READ_VERSION or > SCHEMA_VERSION) are
        // still rejected.
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        for bad_version in [0_i64, 3_i64] {
            db.write(async move |tx| -> Result<()> {
                query("UPDATE _musq_modules SET schema_version = ? WHERE module_id = 'test.mod_b'")
                    .bind(bad_version)
                    .execute(&**tx)
                    .await?;
                Ok(())
            })
            .await?;
            let ro_bad = Database::open_read_only(&db_path, false).await?;
            let err = ro_bad.verify_module::<ModReadCompatV2>().await.unwrap_err();
            assert!(matches!(
                err,
                Error::Module(ModuleError::VersionMismatch {
                    installed_version,
                    expected_version: 2,
                    ..
                }) if i64::from(installed_version) == bad_version
            ));
            ro_bad.close().await?;
        }

        // Restore v1, then mount writable with ModReadCompatV2: writable mount
        // still upgrades v1 -> v2.
        db.write(async |tx| -> Result<()> {
            query("UPDATE _musq_modules SET schema_version = 1 WHERE module_id = 'test.mod_b'")
                .execute(&**tx)
                .await?;
            Ok(())
        })
        .await?;
        let upgraded = db.mount(ModReadCompatV2).await?;
        assert_eq!(upgraded.schema_version, 2);
        let verified_v2 = db.verify_module::<ModReadCompatV2>().await?;
        assert_eq!(verified_v2.schema_version, 2);
        db.close().await?;
        Ok(())
    }
}
