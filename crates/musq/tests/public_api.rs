//! Coverage for smaller public API surfaces.

mod support;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use musq::{
        Arguments, Conditions, DeserializeMode, JournalMode, Musq, QueryBuilder, SqliteDataType,
        Text, UpdateOp, Value, Values, WalCheckpointMode,
        error::{ExtendedErrCode, PrimaryErrCode},
        expr, query, query_as_with, query_scalar, query_scalar_with, query_with,
    };
    use tokio::{sync::mpsc, time::timeout};

    use crate::support::connection;

    #[tokio::test]
    async fn with_constructors_accept_arguments() -> anyhow::Result<()> {
        let conn = connection().await?;

        let mut args = Arguments::default();
        args.add(&7_i32)?;
        args.add_named("name", &"Ada")?;
        let row = query_with("SELECT ?1 AS id, :name AS name", args)
            .fetch_one(&conn)
            .await?;
        assert_eq!(row.get_value_idx::<i32>(0)?, 7);
        assert_eq!(row.get_value_idx::<String>(1)?, "Ada");

        let mut args = Arguments::default();
        args.add(&9_i32)?;
        args.add_named("name", &"Bob")?;
        let (id, name): (i32, String) = query_as_with("SELECT ?1 AS id, :name AS name", args)
            .fetch_one(&conn)
            .await?;
        assert_eq!((id, name), (9, "Bob".to_string()));

        let mut args = Arguments::default();
        args.add(&2_i64)?;
        args.add(&3_i64)?;
        let total: i64 = query_scalar_with("SELECT ?1 + ?2", args)
            .fetch_one(&conn)
            .await?;
        assert_eq!(total, 5);
        Ok(())
    }

    #[tokio::test]
    async fn query_map_variants() -> anyhow::Result<()> {
        let conn = connection().await?;

        let doubled: i32 = query("SELECT 21")
            .map(|row| row.get_value_idx::<i32>(0).unwrap() * 2)
            .fetch_one(&conn)
            .await?;
        assert_eq!(doubled, 42);

        let incremented: i32 = query("SELECT 20")
            .try_map(|row| row.get_value_idx::<i32>(0))
            .map(|value| value + 1)
            .fetch_one(&conn)
            .await?;
        assert_eq!(incremented, 21);
        Ok(())
    }

    #[tokio::test]
    async fn query_builder_composition() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE b (id INTEGER, name TEXT)")
            .execute(&conn)
            .await?;

        let values = Values::new().val("id", 1_i32)?.val("name", "one")?;
        let mut builder = QueryBuilder::new();
        builder.push_sql("INSERT INTO b ");
        builder.push_insert(&values)?;
        builder.build().execute(&conn).await?;

        let mut builder = QueryBuilder::new();
        builder.push_sql("UPDATE b SET ");
        builder.push_set(&Values::new().val("name", "two")?)?;
        builder.push_sql(" WHERE id = ");
        builder.push_bind(&1_i32)?;
        builder.build().execute(&conn).await?;

        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT name FROM b WHERE ");
        builder.push_where(&Values::new().val("name", "two")?)?;
        let name: String = builder
            .build()
            .try_map(|row| row.get_value_idx(0))
            .fetch_one(&conn)
            .await?;
        assert_eq!(name, "two");

        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 1 AS a; ");
        builder.push_query(query("SELECT 2 AS b"));
        let rows = builder.build().fetch_all(&conn).await?;
        assert_eq!(rows.len(), 2);

        let mut builder = QueryBuilder::new();
        assert!(builder.try_push_query(query("SELECT ?1")).is_err());
        Ok(())
    }

    #[tokio::test]
    async fn pool_try_and_close_surfaces() -> anyhow::Result<()> {
        let acquire_pool = Musq::new().max_connections(1).open_in_memory().await?;
        let held = acquire_pool
            .try_acquire()
            .expect("idle connection available");
        assert!(acquire_pool.try_acquire().is_none());
        drop(held);

        let begin_pool = Musq::new().max_connections(1).open_in_memory().await?;
        let tx = begin_pool
            .try_begin()
            .await?
            .expect("idle connection available");
        assert!(begin_pool.try_begin().await?.is_none());
        tx.rollback().await?;

        let conn = begin_pool.acquire().await?;
        conn.close().await?;
        let replacement = begin_pool.acquire().await?;
        replacement.close().await?;

        let close_event = begin_pool.close_event();
        let closer = begin_pool.clone();
        tokio::spawn(async move {
            let _ = closer.close().await;
        });
        timeout(Duration::from_secs(1), close_event)
            .await
            .expect("close event fired");
        Ok(())
    }

    #[tokio::test]
    async fn connection_transaction_and_dropped_hook_events() -> anyhow::Result<()> {
        let mut conn = connection().await?;
        query("CREATE TABLE ct (id INTEGER)").execute(&conn).await?;

        let inserted = conn
            .transaction(async |tx| {
                query("INSERT INTO ct (id) VALUES (1)")
                    .execute(&*tx)
                    .await?;
                Ok::<_, musq::Error>(1_i64)
            })
            .await?;
        assert_eq!(inserted, 1);

        let count: i64 = query_scalar("SELECT COUNT(*) FROM ct")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 1);
        assert_eq!(conn.dropped_hook_events(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn update_hook_reports_update_and_delete() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE u (id INTEGER PRIMARY KEY, v INTEGER)")
            .execute(&conn)
            .await?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        conn.on_update(move |event| {
            tx.send(event).ok();
        })
        .await?;

        query("INSERT INTO u (id, v) VALUES (1, 10)")
            .execute(&conn)
            .await?;
        query("UPDATE u SET v = 11 WHERE id = 1")
            .execute(&conn)
            .await?;
        query("DELETE FROM u WHERE id = 1").execute(&conn).await?;

        let mut ops = Vec::new();
        while let Ok(Some(event)) = timeout(Duration::from_millis(200), rx.recv()).await {
            ops.push(event.op);
        }
        assert_eq!(ops, [UpdateOp::Insert, UpdateOp::Update, UpdateOp::Delete]);
        Ok(())
    }

    #[test]
    fn value_and_text_accessors() {
        let integer = Value::Integer {
            value: 5,
            type_info: None,
        };
        assert_eq!(integer.int().unwrap(), 5);
        assert_eq!(integer.int64().unwrap(), 5);
        assert_eq!(integer.double().unwrap(), 5.0);
        assert_eq!(integer.type_info(), SqliteDataType::Int);
        assert!(!integer.is_null());

        let double = Value::Double {
            value: 1.5,
            type_info: None,
        };
        assert_eq!(double.double().unwrap(), 1.5);

        let text = Value::Text {
            value: "hi".into(),
            type_info: None,
        };
        assert_eq!(text.text().unwrap(), "hi");
        assert_eq!(text.blob().unwrap(), b"hi");
        assert_eq!(text.type_info(), SqliteDataType::Text);
        assert!(!text.is_null());

        let blob = Value::Blob {
            value: vec![1_u8, 2, 3].into(),
            type_info: None,
        };
        assert_eq!(blob.blob().unwrap(), &[1_u8, 2, 3]);

        let null = Value::Null { type_info: None };
        assert!(null.is_null());
        assert!(null.int().is_err());

        assert_eq!(SqliteDataType::from_str("TEXT"), Some(SqliteDataType::Text));
        assert_eq!(SqliteDataType::Text.name(), "TEXT");
        assert!(SqliteDataType::Null.is_null());

        let text = Text::new("hi".to_string());
        assert_eq!(text.as_str(), "hi");
        assert_eq!(text.as_bytes(), b"hi");
        let text: Text = "borrowed".into();
        assert_eq!(text.as_str(), "borrowed");
        let text: Text = "owned".to_string().into();
        assert_eq!(text.as_str(), "owned");
    }

    #[tokio::test]
    async fn sqlite_error_code_accessors() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE pk (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;
        query("INSERT INTO pk (id) VALUES (1)")
            .execute(&conn)
            .await?;

        let error = query("INSERT INTO pk (id) VALUES (1)")
            .execute(&conn)
            .await
            .unwrap_err();
        let sqlite = error.as_sqlite().expect("SQLite error");
        assert_eq!(
            sqlite.codes(),
            (
                PrimaryErrCode::Constraint,
                Some(ExtendedErrCode::ConstraintPrimaryKey)
            )
        );
        assert!(sqlite.is_unique_violation());
        assert!(!sqlite.is_busy());
        Ok(())
    }

    #[tokio::test]
    async fn conditions_and_expression_fragments() -> anyhow::Result<()> {
        let conn = connection().await?;

        let mut conditions = Conditions::new();
        assert!(conditions.is_empty());
        conditions.push(query("a = 1"));
        let conditions = conditions.with(query("b = 2"));
        assert!(!conditions.is_empty());

        let raw = expr::raw("current_timestamp");
        assert!(raw.tainted());
        assert_eq!(raw.sql(), "current_timestamp");

        query("CREATE TABLE e (id INTEGER, stamp TEXT)")
            .execute(&conn)
            .await?;
        let values = Values::new().val("id", 1_i32)?.val("stamp", raw)?;
        let mut builder = QueryBuilder::new();
        builder.push_sql("INSERT INTO e ");
        builder.push_insert(&values)?;
        builder.build().execute(&conn).await?;

        let stamp: String = query_scalar("SELECT stamp FROM e").fetch_one(&conn).await?;
        assert!(!stamp.is_empty());
        Ok(())
    }

    #[cfg(feature = "json")]
    #[tokio::test]
    async fn jsonb_serde_expression() -> anyhow::Result<()> {
        let conn = connection().await?;
        let value = serde_json::json!({"a": 1});
        let expression = expr::jsonb_serde(&value)?;
        assert!(!expression.tainted());

        query("CREATE TABLE j (v TEXT)").execute(&conn).await?;
        let values = Values::new().val("v", expression)?;
        let mut builder = QueryBuilder::new();
        builder.push_sql("INSERT INTO j ");
        builder.push_insert(&values)?;
        builder.build().execute(&conn).await?;

        let extracted: i64 = query_scalar("SELECT json_extract(v, '$.a') FROM j")
            .fetch_one(&conn)
            .await?;
        assert_eq!(extracted, 1);
        Ok(())
    }

    #[tokio::test]
    async fn wal_checkpoint_modes() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("wal.db");
        let pool = Musq::new()
            .create_if_missing(true)
            .journal_mode(JournalMode::Wal)
            .open(&path)
            .await?;
        query("CREATE TABLE w (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await?;

        for mode in [
            WalCheckpointMode::Passive,
            WalCheckpointMode::Full,
            WalCheckpointMode::Restart,
            WalCheckpointMode::Truncate,
        ] {
            query("INSERT INTO w (id) VALUES (NULL)")
                .execute(&pool)
                .await?;
            let checkpoint = pool.wal_checkpoint(Some("main"), mode).await?;
            assert!(checkpoint.log_frames.is_some(), "mode {mode:?}");
            assert!(checkpoint.checkpointed_frames.is_some(), "mode {mode:?}");
        }

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn deserialize_read_only_succeeds() -> anyhow::Result<()> {
        let source = connection().await?;
        query("CREATE TABLE d (id INTEGER PRIMARY KEY, name TEXT)")
            .execute(&source)
            .await?;
        query("INSERT INTO d (id, name) VALUES (1, 'one')")
            .execute(&source)
            .await?;
        let image = source.serialize("main").await?;

        let dest = connection().await?;
        dest.deserialize("main", image, DeserializeMode::ReadOnly)
            .await?;
        let name: String = query_scalar("SELECT name FROM d WHERE id = 1")
            .fetch_one(&dest)
            .await?;
        assert_eq!(name, "one");
        Ok(())
    }

    #[tokio::test]
    async fn builder_options_apply() -> anyhow::Result<()> {
        let pool = Musq::new()
            .defensive(true)
            .optimize_on_close(true)
            .thread_name(|id| format!("musq-public-api-{id}"))
            .analysis_limit(1000)
            .acquire_timeout(Duration::from_secs(2))
            .command_buffer_size(0)
            .row_buffer_size(0)
            .foreign_keys(true)
            .pragma("cache_size", "-2000")
            .open_in_memory()
            .await?;

        let analysis_limit: i64 = query_scalar("PRAGMA analysis_limit")
            .fetch_one(&pool)
            .await?;
        assert_eq!(analysis_limit, 1000);

        let foreign_keys: i64 = query_scalar("PRAGMA foreign_keys").fetch_one(&pool).await?;
        assert_eq!(foreign_keys, 1);

        let cache_size: i64 = query_scalar("PRAGMA cache_size").fetch_one(&pool).await?;
        assert_eq!(cache_size, -2000);

        let expected: i64 = query_scalar("SELECT 21 + 21").fetch_one(&pool).await?;
        assert_eq!(expected, 42);

        let _ = pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn read_only_and_immutable_options() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("ro.db");
        let pool = Musq::new().create_if_missing(true).open(&path).await?;
        query("CREATE TABLE r (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await?;
        query("INSERT INTO r (id) VALUES (1)")
            .execute(&pool)
            .await?;
        let _ = pool.close().await;

        let read_only = Musq::new().read_only(true).open(&path).await?;
        let count: i64 = query_scalar("SELECT COUNT(*) FROM r")
            .fetch_one(&read_only)
            .await?;
        assert_eq!(count, 1);
        assert!(
            query("INSERT INTO r (id) VALUES (2)")
                .execute(&read_only)
                .await
                .is_err()
        );
        let _ = read_only.close().await;

        let immutable = Musq::new()
            .read_only(true)
            .immutable(true)
            .open(&path)
            .await?;
        let count: i64 = query_scalar("SELECT COUNT(*) FROM r")
            .fetch_one(&immutable)
            .await?;
        assert_eq!(count, 1);
        let _ = immutable.close().await;

        #[cfg(unix)]
        {
            let vfs = Musq::new().vfs("unix").open_in_memory().await?;
            let one: i64 = query_scalar("SELECT 1").fetch_one(&vfs).await?;
            assert_eq!(one, 1);
            let _ = vfs.close().await;
        }
        Ok(())
    }
}
