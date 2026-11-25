//! Integration tests for `Database`, `DatabaseOptions`, lazy FIFO writer,
//! `Connection::register_function`, and `export_archive`.

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        fs,
        future::Ready,
        path::{Path, PathBuf},
    };

    use musq::{
        Connection, Database, DatabaseOptions, DatabaseRegistry, Error, FunctionFlags, Musq,
        Transaction, Value, query, query_scalar,
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

    /// Reads the `synchronous` level of the writer inside a write.
    async fn writer_synchronous(db: &Database, durable: bool) -> anyhow::Result<i64> {
        let read = async |tx: &mut Transaction<&mut Connection>| {
            query_scalar::<i64>("PRAGMA synchronous")
                .fetch_one(&**tx)
                .await
        };
        Ok(if durable {
            db.write_durable(read).await?
        } else {
            db.write(read).await?
        })
    }

    #[tokio::test]
    async fn durable_write_commits_at_full_and_gives_the_writer_its_level_back()
    -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db = Database::open(dir.path().join("durable.db"), DatabaseOptions::new()).await?;
        db.write(async |tx| {
            query("CREATE TABLE items (id INTEGER PRIMARY KEY)")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;

        // `NORMAL` is 1, and `FULL` is 2.
        assert_eq!(writer_synchronous(&db, false).await?, 1);
        assert_eq!(writer_synchronous(&db, true).await?, 2);
        assert_eq!(writer_synchronous(&db, false).await?, 1);

        db.write_durable(async |tx| {
            query("INSERT INTO items (id) VALUES (1)")
                .execute(&**tx)
                .await?;
            Ok::<_, Error>(())
        })
        .await?;
        let failed = db
            .write_durable(async |tx| {
                query("INSERT INTO items (id) VALUES (2)")
                    .execute(&**tx)
                    .await?;
                Err::<(), _>(Error::Protocol("refused".to_owned()))
            })
            .await;
        assert!(failed.is_err());
        assert_eq!(writer_synchronous(&db, false).await?, 1);
        let ids: Vec<i64> = query_scalar("SELECT id FROM items ORDER BY id")
            .fetch_all(db.pool())
            .await?;
        assert_eq!(ids, vec![1]);

        let ro = Database::open_read_only(dir.path().join("durable.db"), false).await?;
        let refused = ro
            .write_durable(async |_tx| Ok::<_, Error>(()))
            .await
            .unwrap_err();
        assert!(matches!(refused, Error::ReadOnly));
        ro.close().await?;
        db.close().await?;
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

    #[tokio::test]
    async fn stamp_header_if_fresh_stamps_new_and_accepts_existing_headers() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("co-mounted.db");

        // Fresh database receives the first opener's header stamp.
        let first_opts = DatabaseOptions::new()
            .application_id(0x5650_5256)
            .user_version(3)
            .stamp_header_if_fresh();
        let db = Database::open(&db_path, first_opts).await?;

        let app_id: i32 = query_scalar("PRAGMA application_id")
            .fetch_one(db.pool())
            .await?;
        let user_ver: i32 = query_scalar("PRAGMA user_version")
            .fetch_one(db.pool())
            .await?;
        assert_eq!(app_id, 0x5650_5256);
        assert_eq!(user_ver, 3);
        db.close().await?;

        // A second opener with different application_id and user_version
        // succeeds when `stamp_header_if_fresh()` is enabled, and preserves
        // the original header.
        let second_opts = DatabaseOptions::new()
            .application_id(0x5657_5452)
            .user_version(9)
            .stamp_header_if_fresh();
        let reopened = Database::open(&db_path, second_opts).await?;
        let app_id_after: i32 = query_scalar("PRAGMA application_id")
            .fetch_one(reopened.pool())
            .await?;
        let user_ver_after: i32 = query_scalar("PRAGMA user_version")
            .fetch_one(reopened.pool())
            .await?;
        assert_eq!(app_id_after, 0x5650_5256);
        assert_eq!(user_ver_after, 3);
        reopened.close().await?;

        // Default strict mode still rejects mismatched headers.
        let err = Database::open(&db_path, DatabaseOptions::new().application_id(0x5657_5452))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::IncompatibleFormat { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn database_registry_reuses_live_handle_and_reopens_after_replace() -> anyhow::Result<()>
    {
        let dir = tempdir()?;
        let db_path = dir.path().join("shared.db");
        let registry = DatabaseRegistry::new();
        assert!(registry.is_empty().await);

        let first = registry
            .open_writable(&db_path, DatabaseOptions::new())
            .await?;
        assert_eq!(registry.len().await, 1);

        first
            .write(async |tx| {
                query("CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL)")
                    .execute(&**tx)
                    .await?;
                query("INSERT INTO notes (id, body) VALUES (1, 'v1')")
                    .execute(&**tx)
                    .await?;
                Ok::<_, Error>(())
            })
            .await?;

        // Repeated open on the same canonical path returns a clone of the exact
        // same underlying `Database` (sharing pool and FIFO writer).
        let second = registry
            .open_writable(
                &db_path,
                DatabaseOptions::new()
                    .application_id(0x1234_5678)
                    .stamp_header_if_fresh(),
            )
            .await?;
        assert_eq!(registry.len().await, 1);
        let body: String = query_scalar("SELECT body FROM notes WHERE id = 1")
            .fetch_one(second.pool())
            .await?;
        assert_eq!(body, "v1");

        // Closing `first` closes `second`'s pool because both wrap the same
        // `DatabaseInner`.
        first.close().await?;
        assert!(second.pool().is_closed());

        // Subsequent open detects the closed pool and opens a fresh handle.
        let reopened = registry
            .open_writable(&db_path, DatabaseOptions::new())
            .await?;
        assert!(!reopened.pool().is_closed());
        let body_again: String = query_scalar("SELECT body FROM notes WHERE id = 1")
            .fetch_one(reopened.pool())
            .await?;
        assert_eq!(body_again, "v1");

        // Deleting and recreating the file at the same path evicts the stale
        // handle (new inode) and opens the replacement file cleanly.
        reopened.close().await?;
        fs::remove_file(&db_path)?;
        drop(fs::remove_file(sidecar(&db_path, "-wal")));
        drop(fs::remove_file(sidecar(&db_path, "-shm")));

        let fresh = registry
            .open_writable(&db_path, DatabaseOptions::new())
            .await?;
        let notes_table: i64 = query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'notes'",
        )
        .fetch_one(fresh.pool())
        .await?;
        assert_eq!(notes_table, 0);

        // Even without closing the prior handle first, deleting + recreating
        // the file at the same path causes the next open to replace it.
        fresh
            .write(async |tx| {
                query("CREATE TABLE v2_only (id INTEGER PRIMARY KEY)")
                    .execute(&**tx)
                    .await?;
                Ok::<_, Error>(())
            })
            .await?;
        fs::remove_file(&db_path)?;
        drop(fs::remove_file(sidecar(&db_path, "-wal")));
        drop(fs::remove_file(sidecar(&db_path, "-shm")));

        let recreated = registry
            .open_writable(&db_path, DatabaseOptions::new())
            .await?;
        let v2_table: i64 = query_scalar(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type = 'table' AND name = 'v2_only'",
        )
        .fetch_one(recreated.pool())
        .await?;
        assert_eq!(v2_table, 0);

        let removed = registry.remove(&db_path).await;
        assert!(removed.is_some());
        assert!(registry.is_empty().await);

        fresh.close().await?;
        recreated.close().await?;
        Ok(())
    }
}
