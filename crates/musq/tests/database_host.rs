//! Integration tests for `Database`, `DatabaseOptions`, lazy FIFO writer,
//! `Connection::register_function`, and `export_archive`.

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        future::Ready,
        path::{Path, PathBuf},
    };

    use musq::{
        Connection, Database, DatabaseOptions, Error, FunctionFlags, Musq, Transaction, Value,
        query, query_scalar,
    };
    use tempfile::tempdir;

    /// Compute a SQLite sidecar path (`-wal` or `-shm`).
    fn sidecar(path: &Path, suffix: &str) -> PathBuf {
        let mut os = OsString::from(path.as_os_str());
        os.push(suffix);
        PathBuf::from(os)
    }

    /// Panics synchronously before returning its future.
    fn sync_panic(_tx: &mut Transaction<&mut Connection>) -> Ready<Result<(), Error>> {
        panic!("sync boom")
    }

    #[tokio::test]
    async fn wal_bootstrap_and_header_stamping() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("nested").join("host.db");

        let opts = DatabaseOptions::new()
            .application_id(0x5657_5452)
            .user_version(7);
        let db = Database::open(&db_path, opts.clone()).await?;

        assert!(!db.is_read_only());
        assert!(!db.is_immutable());
        assert!(db.path().is_absolute());

        let page_size: i64 = query_scalar("PRAGMA page_size")
            .fetch_one(db.pool())
            .await?;
        let auto_vacuum: i64 = query_scalar("PRAGMA auto_vacuum")
            .fetch_one(db.pool())
            .await?;
        let journal_mode: String = query_scalar("PRAGMA journal_mode")
            .fetch_one(db.pool())
            .await?;
        let foreign_keys: i64 = query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await?;
        let app_id: i32 = query_scalar("PRAGMA application_id")
            .fetch_one(db.pool())
            .await?;
        let user_ver: i32 = query_scalar("PRAGMA user_version")
            .fetch_one(db.pool())
            .await?;

        assert_eq!(page_size, 4096);
        assert_eq!(auto_vacuum, 2); // INCREMENTAL
        assert!(journal_mode.eq_ignore_ascii_case("wal"));
        assert_eq!(foreign_keys, 1);
        assert_eq!(app_id, 0x5657_5452);
        assert_eq!(user_ver, 7);

        // `_musq_modules` registry table exists.
        let reg_exists: i64 = query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = '_musq_modules'",
        )
        .fetch_one(db.pool())
        .await?;
        assert_eq!(reg_exists, 1);

        db.close().await?;

        // Reopening with matching header succeeds.
        let reopened = Database::open(&db_path, opts).await?;
        reopened.close().await?;

        // Reopening with mismatched application_id fails with
        // IncompatibleFormat.
        let err = Database::open(&db_path, DatabaseOptions::new().application_id(0x1111_2222))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::IncompatibleFormat { .. }));

        // Reopening with mismatched user_version fails with IncompatibleFormat.
        let err = Database::open(&db_path, DatabaseOptions::new().user_version(99))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::IncompatibleFormat { .. }));

        // Read-only default open ignores host application_id/user_version.
        let ro = Database::open_read_only(&db_path, false).await?;
        assert!(ro.is_read_only());
        assert!(!ro.is_immutable());
        let write_err = ro.write(async |_tx| Ok::<_, Error>(())).await.unwrap_err();
        assert!(matches!(write_err, Error::ReadOnly));
        ro.close().await?;

        Ok(())
    }

    #[tokio::test]
    async fn concurrent_writers_serialize_without_busy_errors() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("concurrent.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;

        db.write(async |tx| {
            query("CREATE TABLE items (id INTEGER PRIMARY KEY AUTOINCREMENT, worker INTEGER NOT NULL, seq INTEGER NOT NULL)")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        let mut tasks = Vec::new();
        for worker in 0_i64..12 {
            let handle = db.clone();
            tasks.push(tokio::spawn(async move {
                for seq in 0_i64..20 {
                    handle
                        .write(async move |tx| {
                            query("INSERT INTO items (worker, seq) VALUES (?, ?)")
                                .bind(worker)
                                .bind(seq)
                                .execute(&**tx)
                                .await?;
                            Ok::<_, Error>(())
                        })
                        .await?;
                }
                Ok::<_, Error>(())
            }));
        }

        for task in tasks {
            task.await??;
        }

        let count: i64 = query_scalar("SELECT COUNT(*) FROM items")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(count, 12 * 20);
        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn atomic_cross_table_commit_rollback_and_panic_recovery() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("atomic.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;

        db.write(async |tx| {
            query("CREATE TABLE a_tbl (id INTEGER PRIMARY KEY, val TEXT NOT NULL)")
                .execute(&**tx)
                .await?;
            query("CREATE TABLE b_tbl (id INTEGER PRIMARY KEY, val TEXT NOT NULL)")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        // Atomic commit across two tables.
        let sum: i64 = db
            .write(async |tx| {
                query("INSERT INTO a_tbl (id, val) VALUES (1, 'ok')")
                    .execute(&**tx)
                    .await?;
                query("INSERT INTO b_tbl (id, val) VALUES (1, 'ok')")
                    .execute(&**tx)
                    .await?;
                Ok::<_, Error>(42)
            })
            .await?;
        assert_eq!(sum, 42);

        // Error rolls back both tables.
        let err = db
            .write(async |tx| {
                query("INSERT INTO a_tbl (id, val) VALUES (2, 'rolled_back')")
                    .execute(&**tx)
                    .await?;
                Err::<(), Error>(Error::Query("abort cross-module write".into()))
            })
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Query(_)));

        let count_a: i64 = query_scalar("SELECT COUNT(*) FROM a_tbl")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(count_a, 1);

        // Panic inside callback rolls back cleanly and keeps writer usable.
        let db_for_panic = db.clone();
        let panic_res = tokio::spawn(async move {
            let res: Result<(), Error> = db_for_panic
                .write(async |tx| {
                    query("INSERT INTO a_tbl (id, val) VALUES (3, 'panicked')")
                        .execute(&**tx)
                        .await?;
                    panic!("simulated callback panic")
                })
                .await;
            res
        })
        .await;
        assert!(panic_res.unwrap_err().is_panic());

        // Subsequent write still works and row 3 was rolled back.
        db.write(async |tx| {
            query("INSERT INTO b_tbl (id, val) VALUES (2, 'after_panic')")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        let count_a: i64 = query_scalar("SELECT COUNT(*) FROM a_tbl")
            .fetch_one(db.pool())
            .await?;
        let count_b: i64 = query_scalar("SELECT COUNT(*) FROM b_tbl")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(count_a, 1);
        assert_eq!(count_b, 2);

        db.close().await?;

        // Writes after close return PoolClosed.
        let after_close = db.write(async |_tx| Ok::<_, Error>(())).await.unwrap_err();
        assert!(matches!(after_close, Error::PoolClosed));
        Ok(())
    }

    #[tokio::test]
    async fn export_archive_mid_wal_has_no_sidecars_and_opens_immutable() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("live.db");
        let archive_path = dir.path().join("exports").join("archive.db");

        let db = Database::open(&db_path, DatabaseOptions::new()).await?;
        db.write(async |tx| {
            query("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
                .execute(&**tx)
                .await?;
            for idx in 1_i64..=50 {
                query("INSERT INTO notes (id, body) VALUES (?, ?)")
                    .bind(idx)
                    .bind(format!("note-{idx}"))
                    .execute(&**tx)
                    .await?;
            }
            Ok::<_, Error>(())
        })
        .await?;

        // Hold an active reader connection while exporting mid-WAL.
        let _reader = db.pool().acquire().await?;
        db.export_archive(&archive_path).await?;

        assert!(archive_path.exists());
        assert!(!sidecar(&archive_path, "-wal").exists());
        assert!(!sidecar(&archive_path, "-shm").exists());

        let archived = Database::open_read_only(&archive_path, true).await?;
        assert!(archived.is_immutable());
        let mode: String = query_scalar("PRAGMA journal_mode")
            .fetch_one(archived.pool())
            .await?;
        assert!(mode.eq_ignore_ascii_case("delete"));
        let count: i64 = query_scalar("SELECT COUNT(*) FROM notes")
            .fetch_one(archived.pool())
            .await?;
        assert_eq!(count, 50);

        archived.close().await?;
        drop(_reader);
        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn connection_register_function_works_on_live_connection() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let conn = pool.acquire().await?;
        conn.register_function(
            "triple_int",
            1,
            FunctionFlags {
                deterministic: true,
                direct_only: true,
                innocuous: false,
            },
            |args| {
                let n = args[0].int64().unwrap_or(0);
                Ok(Value::Integer {
                    value: n * 3,
                    type_info: None,
                })
            },
        )
        .await?;

        let result: i64 = query_scalar("SELECT triple_int(14)")
            .fetch_one(&*conn)
            .await?;
        assert_eq!(result, 42);
        drop(conn);
        let () = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn export_archive_rejects_source_path_and_preserves_live_db() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("live.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;

        db.write(async |tx| {
            query("CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT NOT NULL)")
                .execute(&**tx)
                .await?;
            query("INSERT INTO items (id, label) VALUES (1, 'safe')")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        // Both canonical path, original path, and WAL sidecar must be rejected
        // before unlinking anything.
        for alias in [
            db.path().to_path_buf(),
            db_path.clone(),
            sidecar(&db_path, "-wal"),
        ] {
            let err = db.export_archive(&alias).await.unwrap_err();
            assert!(
                matches!(err, Error::Configuration(_)),
                "expected Error::Configuration for alias {}, got {err:?}",
                alias.display()
            );
        }

        let label: String = query_scalar("SELECT label FROM items WHERE id = 1")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(label, "safe");

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn synchronous_panic_in_write_rolls_back_and_keeps_writer_alive() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("sync-panic.db");
        let db = Database::open(&db_path, DatabaseOptions::new()).await?;

        db.write(async |tx| {
            query("CREATE TABLE items (id INTEGER PRIMARY KEY, label TEXT NOT NULL)")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        // Panic synchronously inside the callback BEFORE returning a future.
        let db_clone = db.clone();
        let handle = tokio::spawn(async move { db_clone.write(sync_panic).await });
        assert!(handle.await.unwrap_err().is_panic());

        // Subsequent writes still succeed on the live writer thread.
        db.write(async |tx| {
            query("INSERT INTO items (id, label) VALUES (1, 'alive')")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        let label: String = query_scalar("SELECT label FROM items WHERE id = 1")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(label, "alive");

        db.close().await?;
        Ok(())
    }
}
