//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::{env, sync::Arc, time::Duration};

    use futures::{StreamExt, TryStreamExt};
    use musq::{Connection, Error, Musq, Row, query, query_as, query_scalar};
    use musq_test::{connection, tdb};
    use rand::{Rng, SeedableRng};
    use rand_xoshiro::Xoshiro256PlusPlus;
    use tokio::{sync::Barrier, task::spawn, time::sleep};

    #[tokio::test]
    async fn it_connects() -> anyhow::Result<()> {
        connection().await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_fetches_and_inflates_row() -> anyhow::Result<()> {
        let conn = connection().await?;

        // process rows, one-at-a-time
        // this reuses the memory of the row

        {
            let expected = [15, 39, 51];
            let mut i = 0;
            let mut s = query("SELECT 15 UNION SELECT 51 UNION SELECT 39").fetch(&conn);

            while let Some(row) = s.try_next().await? {
                let v1 = row.get_value_idx::<i32>(0).unwrap();
                assert_eq!(expected[i], v1);
                i += 1;
            }
        }

        // same query, but fetch all rows at once
        // this triggers the internal inflation

        let rows = query("SELECT 15 UNION SELECT 51 UNION SELECT 39")
            .fetch_all(&conn)
            .await?;

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].get_value_idx::<i32>(0).unwrap(), 15);
        assert_eq!(rows[1].get_value_idx::<i32>(0).unwrap(), 39);
        assert_eq!(rows[2].get_value_idx::<i32>(0).unwrap(), 51);

        let row1 = query("SELECT 15 UNION SELECT 51 UNION SELECT 39")
            .fetch_one(&conn)
            .await?;

        assert_eq!(row1.get_value_idx::<i32>(0).unwrap(), 15);

        let row2 = query("SELECT 15 UNION SELECT 51 UNION SELECT 39")
            .fetch_one(&conn)
            .await?;

        assert_eq!(row1.get_value_idx::<i32>(0).unwrap(), 15);
        assert_eq!(row2.get_value_idx::<i32>(0).unwrap(), 15);

        let row1 = query("SELECT 15 UNION SELECT 51 UNION SELECT 39")
            .fetch_one(&conn)
            .await?;

        assert_eq!(row1.get_value_idx::<i32>(0).unwrap(), 15);

        let row2 = query("SELECT 15 UNION SELECT 51 UNION SELECT 39")
            .fetch_one(&conn)
            .await?;

        assert_eq!(row1.get_value_idx::<i32>(0).unwrap(), 15);
        assert_eq!(row2.get_value_idx::<i32>(0).unwrap(), 15);

        Ok(())
    }

    #[tokio::test]
    async fn it_maths() -> anyhow::Result<()> {
        let conn = connection().await?;

        let value = query("select 1 + ?1")
            .bind(5_i32)
            .try_map(|row: Row| row.get_value_idx::<i32>(0))
            .fetch_one(&conn)
            .await?;

        assert_eq!(6i32, value);

        Ok(())
    }

    #[tokio::test]
    async fn test_bind_multiple_statements_multiple_values() -> anyhow::Result<()> {
        let conn = connection().await?;

        let values: Vec<i32> = musq::query_scalar::<i32>("select ?; select ?")
            .bind(5_i32)
            .bind(15_i32)
            .fetch_all(&conn)
            .await?;

        assert_eq!(values.len(), 2);
        assert_eq!(values[0], 5);
        assert_eq!(values[1], 15);

        Ok(())
    }

    #[tokio::test]
    async fn test_bind_multiple_statements_same_value() -> anyhow::Result<()> {
        let conn = connection().await?;

        let values: Vec<i32> = musq::query_scalar::<i32>("select ?1; select ?1")
            .bind(25_i32)
            .fetch_all(&conn)
            .await?;

        assert_eq!(values.len(), 2);
        assert_eq!(values[0], 25);
        assert_eq!(values[1], 25);

        Ok(())
    }

    #[tokio::test]
    async fn it_can_describe_with_pragma() -> anyhow::Result<()> {
        let conn = tdb().await?;
        let defaults = query("pragma table_info (tweet)")
            .try_map(|row: Row| {
                let val: Option<String> = row.get_value("dflt_value")?;
                Ok(val)
            })
            .fetch_all(&conn)
            .await?;
        assert_eq!(defaults[0], None);
        assert_eq!(defaults[2], Some("TRUE".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn it_binds_positional_parameters_issue_467() -> anyhow::Result<()> {
        let conn = connection().await?;

        let row: (i32, i32, i32, i32) = musq::query_as("select ?1, ?1, ?3, ?2")
            .bind(5_i32)
            .bind(500_i32)
            .bind(1020_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(row.0, 5);
        assert_eq!(row.1, 5);
        assert_eq!(row.2, 1020);
        assert_eq!(row.3, 500);

        Ok(())
    }

    #[tokio::test]
    async fn it_fetches_in_loop() -> anyhow::Result<()> {
        // this is trying to check for any data races
        // there were a few that triggered *sometimes* while building out StatementWorker
        for _ in 0..1000_usize {
            let conn = connection().await?;
            let v: Vec<(i32,)> = query_as("SELECT 1").fetch_all(&conn).await?;

            assert_eq!(v[0].0, 1);
        }

        Ok(())
    }

    #[tokio::test]
    async fn it_executes_with_pool() -> anyhow::Result<()> {
        let pool = Musq::new().max_connections(2).open_in_memory().await?;

        let rows = query("SELECT 1; SElECT 2").fetch_all(&pool).await?;

        assert_eq!(rows.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn it_opens_in_memory() -> anyhow::Result<()> {
        // If the filename is ":memory:", then a private, temporary in-memory database
        // is created for the connection.
        let conn = Connection::connect_with(&Musq::new()).await?;
        conn.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn it_fails_to_parse() -> anyhow::Result<()> {
        let conn = connection().await?;
        let res = query("SEELCT 1").execute(&conn).await;

        assert!(res.is_err());

        let err = res.unwrap_err().into_sqlite_error().unwrap();

        assert_eq!(err.message, "near \"SEELCT\": syntax error");

        Ok(())
    }

    #[tokio::test]
    async fn it_handles_empty_queries() -> anyhow::Result<()> {
        let conn = connection().await?;
        let done = query("").execute(&conn).await?;

        assert_eq!(done.rows_affected(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn it_binds_parameters() -> anyhow::Result<()> {
        let conn = connection().await?;

        let v: i32 = query_scalar("SELECT ?")
            .bind(10_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v, 10);

        let v: (i32, i32) = query_as("SELECT ?1, ?")
            .bind(10_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v.0, 10);
        assert_eq!(v.1, 10);

        Ok(())
    }

    #[tokio::test]
    async fn it_binds_dollar_parameters() -> anyhow::Result<()> {
        let conn = connection().await?;

        let v: (i32, i32) = query_as("SELECT $1, $2")
            .bind(10_i32)
            .bind(11_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v.0, 10);
        assert_eq!(v.1, 11);

        Ok(())
    }

    #[tokio::test]
    async fn it_binds_named_parameters() -> anyhow::Result<()> {
        let conn = connection().await?;

        let v: (i32, i32) = query_as("SELECT :a, @b")
            .bind_named(":a", 10_i32)
            .bind_named("@b", 11_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v.0, 10);
        assert_eq!(v.1, 11);

        Ok(())
    }

    #[tokio::test]
    async fn it_binds_duplicate_named_parameters() -> anyhow::Result<()> {
        let conn = connection().await?;

        let v: (i32, i32) = query_as("SELECT :x, :x")
            .bind_named("x", 7_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v.0, 7);
        assert_eq!(v.1, 7);

        Ok(())
    }

    #[tokio::test]
    async fn it_uses_named_parameters_in_sql() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TEMP TABLE np (id INTEGER PRIMARY KEY, val TEXT);")
            .execute(&conn)
            .await?;

        query("INSERT INTO np (id, val) VALUES (:id, :val)")
            .bind_named("id", 1_i32)
            .bind_named("val", "alpha")
            .execute(&conn)
            .await?;

        let (val,): (String,) = query_as("SELECT val FROM np WHERE id = :id")
            .bind_named("id", 1_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(val, "alpha");

        Ok(())
    }

    #[tokio::test]
    async fn it_mixes_named_and_positional_parameters() -> anyhow::Result<()> {
        let conn = connection().await?;

        let (sum,): (i32,) = query_as("SELECT :a + ?2 + ?3")
            .bind_named("a", 2_i32) // :a
            .bind(3_i32) // ?2
            .bind(4_i32) // ?3
            .fetch_one(&conn)
            .await?;

        assert_eq!(sum, 9);

        Ok(())
    }

    #[tokio::test]
    async fn it_supports_named_only_binding() -> anyhow::Result<()> {
        let conn = connection().await?;

        let (a, b): (i32, i32) = query_as("SELECT :first, :second")
            .bind_named("second", 42_i32)
            .bind_named("first", 7_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(a, 7);
        assert_eq!(b, 42);

        Ok(())
    }

    #[tokio::test]
    async fn it_combines_named_and_positional_binds() -> anyhow::Result<()> {
        let conn = connection().await?;

        let (sum,): (i32,) = query_as("SELECT :v + ?2 + :v")
            .bind_named("v", 5_i32)
            .bind(3_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(sum, 13);

        Ok(())
    }

    #[tokio::test]
    async fn it_executes_queries() -> anyhow::Result<()> {
        let conn = connection().await?;

        let _ = query(
            r#"
    CREATE TEMPORARY TABLE users (id INTEGER PRIMARY KEY)
                "#,
        )
        .execute(&conn)
        .await?;

        for index in 1..=10_i32 {
            let done = query("INSERT INTO users (id) VALUES (?)")
                .bind(index * 2)
                .execute(&conn)
                .await?;

            assert_eq!(done.rows_affected(), 1);
        }

        let sum: i32 = query("SELECT id FROM users")
            .fetch(&conn)
            .map(|res| res.map(|row| row.get_value::<i32>("id").unwrap()))
            .try_fold(0_i32, |acc, x: i32| async move { Ok(acc + x) })
            .await?;

        assert_eq!(sum, 110);

        Ok(())
    }

    #[tokio::test]
    async fn it_reports_zero_rows_affected_for_select() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TEMPORARY TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;

        let done = query("INSERT INTO t (id) VALUES (1)")
            .execute(&conn)
            .await?;
        assert_eq!(done.rows_affected(), 1);

        let done = query("SELECT id FROM t").execute(&conn).await?;
        assert_eq!(done.rows_affected(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn it_can_execute_multiple_statements() -> anyhow::Result<()> {
        let conn = connection().await?;

        let done = query(
            r#"
    CREATE TEMPORARY TABLE users (id INTEGER PRIMARY KEY, other INTEGER);
    INSERT INTO users DEFAULT VALUES;
                "#,
        )
        .execute(&conn)
        .await?;

        assert_eq!(done.rows_affected(), 1);

        for index in 2..5_i32 {
            let (id, other): (i32, i32) = query_as(
                r#"
    INSERT INTO users (other) VALUES (?);
    SELECT id, other FROM users WHERE id = last_insert_rowid();
                "#,
            )
            .bind(index)
            .fetch_one(&conn)
            .await?;

            assert_eq!(id, index);
            assert_eq!(other, index);
        }

        Ok(())
    }

    #[tokio::test]
    async fn it_interleaves_reads_and_writes() -> anyhow::Result<()> {
        let conn = connection().await?;

        let mut cursor = query(
            "
    CREATE TABLE IF NOT EXISTS _musq_test (
        id INT PRIMARY KEY,
        text TEXT NOT NULL
    );

    SELECT 'Hello World' as _1;

    INSERT INTO _musq_test (text) VALUES ('this is a test');

    SELECT id, text FROM _musq_test;
        ",
        )
        .fetch(&conn);

        let row = cursor.try_next().await?.unwrap();

        assert!("Hello World" == row.get_value::<&str>("_1")?);

        let row = cursor.try_next().await?.unwrap();

        let id: Option<i64> = row.get_value("id")?;
        let text: &str = row.get_value("text")?;

        assert_eq!(None, id);
        assert_eq!("this is a test", text);

        Ok(())
    }

    #[tokio::test]
    async fn it_caches_statements() -> anyhow::Result<()> {
        let conn = connection().await?;

        let row = query("SELECT 100 AS val").fetch_one(&conn).await?;
        let val: i32 = row.get_value("val").unwrap();
        assert_eq!(val, 100);

        // `Query` is persistent by default.
        let conn = connection().await?;
        for i in 0..2 {
            let row = query("SELECT ? AS val").bind(i).fetch_one(&conn).await?;

            let val: i32 = row.get_value("val").unwrap();

            assert_eq!(i, val);
        }

        // Cache can be cleared, but this is an internal detail so we simply
        // ensure queries continue to execute.

        // `Query` is not persistent if `.persistent(false)` is used
        // explicitly.
        let conn = connection().await?;
        for i in 0..2 {
            let row = query("SELECT ? AS val").bind(i).fetch_one(&conn).await?;

            let val: i32 = row.get_value("val").unwrap();

            assert_eq!(i, val);
        }

        Ok(())
    }

    #[tokio::test]
    async fn it_respects_statement_cache_capacity() -> anyhow::Result<()> {
        let options = Musq::new().statement_cache_capacity(1);
        let pool = options.open_in_memory().await?;
        let conn = pool.acquire().await?;

        // first query populates cache
        let row = query("SELECT 1 AS val").fetch_one(&conn).await?;
        let val: i32 = row.get_value("val").unwrap();
        assert_eq!(val, 1);

        // second query should also succeed even when the cache evicts the first
        let row = query("SELECT 2 AS val").fetch_one(&conn).await?;
        let val: i32 = row.get_value("val").unwrap();
        assert_eq!(val, 2);

        Ok(())
    }

    #[tokio::test]
    async fn it_can_prepare_then_execute() -> anyhow::Result<()> {
        let mut conn = tdb().await?;
        let tx = conn.begin().await?;

        let _ = query("INSERT INTO tweet ( id, text ) VALUES ( 2, 'Hello, World' )")
            .execute(&tx)
            .await?;

        let tweet_id: i32 = 2;

        let statement = tx.prepare("SELECT * FROM tweet WHERE id = ?1").await?;

        let row = statement.query().bind(tweet_id).fetch_one(&tx).await?;
        let tweet_text: &str = row.get_value("text")?;

        assert_eq!(tweet_text, "Hello, World");

        Ok(())
    }

    #[tokio::test]
    async fn it_handles_numeric_affinity() -> anyhow::Result<()> {
        let conn = tdb().await?;

        query("INSERT INTO products (product_no, name, price) VALUES (1, 'Prod 1', 9.99)")
            .execute(&conn)
            .await?;

        query("INSERT INTO products (product_no, name, price) VALUES (?, ?, ?)")
            .bind(2_i32)
            .bind("Prod 2")
            .bind(19.95_f64)
            .execute(&conn)
            .await?;

        let stmt = conn
            .prepare("SELECT price FROM products WHERE product_no = ?")
            .await?;

        let row = stmt.query().bind(1_i32).fetch_one(&conn).await?;
        let price: f64 = row.get_value_idx(0)?;
        assert_eq!(price, 9.99_f64);

        let row = stmt.query().bind(2_i32).fetch_one(&conn).await?;
        let price: f64 = row.get_value_idx(0)?;
        assert_eq!(price, 19.95_f64);

        Ok(())
    }

    #[tokio::test]
    async fn it_resets_prepared_statement_after_fetch_one() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TEMPORARY TABLE foobar (id INTEGER)")
            .execute(&conn)
            .await?;
        query("INSERT INTO foobar VALUES (42)")
            .execute(&conn)
            .await?;

        let r = query("SELECT id FROM foobar").fetch_one(&conn).await?;
        let x: i32 = r.get_value("id")?;
        assert_eq!(x, 42);

        query("DROP TABLE foobar").execute(&conn).await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_resets_prepared_statement_after_fetch_many() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("CREATE TEMPORARY TABLE foobar (id INTEGER)")
            .execute(&conn)
            .await?;
        query("INSERT INTO foobar VALUES (42)")
            .execute(&conn)
            .await?;
        query("INSERT INTO foobar VALUES (43)")
            .execute(&conn)
            .await?;

        let mut rows = query("SELECT id FROM foobar").fetch(&conn);
        let row = rows.try_next().await?.unwrap();
        let x: i32 = row.get_value("id")?;
        assert_eq!(x, 42);
        drop(rows);

        query("DROP TABLE foobar").execute(&conn).await?;

        Ok(())
    }

    #[tokio::test]
    async fn it_can_transact() {
        let pool = Musq::new().open_in_memory().await.unwrap();
        query("CREATE TABLE foo (value INTEGER)")
            .execute(&pool)
            .await
            .unwrap();

        macro_rules! add {
            ($tx: expr, $v:expr) => {
                query("INSERT INTO foo (value) VALUES (?)")
                    .bind($v)
                    .execute(&*$tx)
                    .await
                    .unwrap();
            };
        }

        macro_rules! check {
            ($tx: expr, $v:expr) => {
                query_as::<(i64,)>("SELECT count(*) FROM foo WHERE value = ?")
                    .bind($v)
                    .fetch_one(&*$tx)
                    .await
                    .unwrap()
                    .0
            };
        }

        let mut conn = pool.acquire().await.unwrap();
        {
            let mut tx0 = conn.begin().await.unwrap();
            assert_eq!(check!(tx0, 0), 0);
            add!(tx0, 0);
            assert_eq!(check!(tx0, 0), 1);
            {
                let tx1 = tx0.begin().await.unwrap();
                assert_eq!(check!(tx1, 0), 1);
                add!(tx1, 1);
                assert_eq!(check!(tx1, 1), 1);
            }
            assert_eq!(check!(tx0, 1), 0);
            assert_eq!(check!(tx0, 0), 1);
        }

        let mut ntx = conn.begin().await.unwrap();
        assert_eq!(check!(ntx, 0), 0);
        ntx.rollback().await.unwrap();
        drop(ntx);

        {
            let mut tx0 = conn.begin().await.unwrap();
            add!(tx0, 0);
            {
                let mut tx1 = tx0.begin().await.unwrap();
                add!(tx1, 1);
                tx1.commit().await.unwrap();
            }
            assert_eq!(check!(tx0, 1), 1);
        }
    }

    #[tokio::test]
    async fn it_releases_savepoint_on_nested_rollback() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let mut conn = pool.acquire().await?;

        query("CREATE TABLE t (value INTEGER)")
            .execute(&conn)
            .await?;

        let mut tx0 = conn.begin().await?;
        query("INSERT INTO t (value) VALUES (0)")
            .execute(&tx0)
            .await?;

        {
            let mut tx1 = tx0.begin().await?;
            query("INSERT INTO t (value) VALUES (1)")
                .execute(&tx1)
                .await?;
            tx1.rollback().await?;
        }

        {
            let mut tx2 = tx0.begin().await?;
            query("INSERT INTO t (value) VALUES (2)")
                .execute(&tx2)
                .await?;
            tx2.commit().await?;
        }

        tx0.commit().await?;
        drop(tx0);

        let values: Vec<(i64,)> = query_as("SELECT value FROM t ORDER BY value")
            .fetch_all(&conn)
            .await?;

        assert_eq!(values, vec![(0,), (2,)]);

        Ok(())
    }

    // https://github.com/launchbadge/sqlx/issues/1300
    #[tokio::test]
    async fn concurrent_resets_dont_segfault() {
        let pool = Musq::new().open_in_memory().await.unwrap();

        query("CREATE TABLE stuff (name INTEGER, value INTEGER)")
            .execute(&pool)
            .await
            .unwrap();

        spawn(async move {
            for i in 0..1000 {
                query("INSERT INTO stuff (name, value) VALUES (?, ?)")
                    .bind(i)
                    .bind(0)
                    .execute(&pool)
                    .await
                    .unwrap();
            }
        });

        sleep(Duration::from_millis(1)).await;
    }

    // https://github.com/launchbadge/sqlx/issues/1419
    // note: this passes before and after the fix; you need to run it with `--nocapture`
    // to see the panic from the worker thread, which doesn't happen after the fix
    #[tokio::test]
    async fn row_dropped_after_connection_doesnt_panic() {
        let conn = Connection::connect_with(&Musq::new()).await.unwrap();

        let books = query("SELECT 'hello' AS title")
            .fetch_all(&conn)
            .await
            .unwrap();

        for book in &books {
            // force the row to be inflated
            let _title: String = book.get_value("title").unwrap();
        }

        // hold `books` past the lifetime of `conn`
        drop(conn);
        sleep(Duration::from_secs(1)).await;
        drop(books);
    }

    #[tokio::test]
    async fn issue_1467() -> anyhow::Result<()> {
        // Regression test for https://github.com/launchbadge/sqlx/issues/1467
        //
        // The original report required many iterations and was more reliably reproduced in
        // release mode. Keep this test fast for `cargo test` and allow overriding the
        // iteration count for stress runs.
        let iterations = env::var("MUSQ_ISSUE_1467_ITERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000_usize);

        let mut conn = Connection::connect_with(&Musq::new()).await?;

        query(
            r#"
        CREATE TABLE kv (k PRIMARY KEY, v);
        CREATE INDEX idx_kv ON kv (v);
        "#,
        )
        .execute(&conn)
        .await?;

        // Deterministic seed so this is stable under CI.
        let seed: [u8; 32] = [0x42; 32];

        // reproducible RNG for testing
        let mut rng = Xoshiro256PlusPlus::from_seed(seed);

        for _ in 0..iterations {
            let key = rng.random_range(0..1_000);
            let value = rng.random_range(0..1_000);
            let mut tx = conn.begin().await?;

            let exists = query("SELECT 1 FROM kv WHERE k = ? LIMIT 1")
                .bind(key)
                .fetch_optional(&tx)
                .await?;
            if exists.is_some() {
                query("UPDATE kv SET v = ? WHERE k = ?")
                    .bind(value)
                    .bind(key)
                    .execute(&tx)
                    .await?;
            } else {
                query("INSERT INTO kv(k, v) VALUES (?, ?)")
                    .bind(key)
                    .bind(value)
                    .execute(&tx)
                    .await?;
            }
            tx.commit().await?;
        }
        Ok(())
    }

    #[tokio::test]
    async fn concurrent_read_and_write() -> anyhow::Result<()> {
        use tempdir::TempDir;

        let dir = TempDir::new("musq-concurrent-read-write")?;
        let path = dir.path().join("test.db");

        let pool = Musq::new()
            .create_if_missing(true)
            .max_connections(2)
            .open(&path)
            .await?;

        // Use WAL to avoid reader/writer deadlocks in rollback-journal mode.
        query("PRAGMA journal_mode = WAL").execute(&pool).await?;

        query("CREATE TABLE kv (k PRIMARY KEY, v)")
            .execute(&pool)
            .await?;

        let n = 100;
        let barrier = Arc::new(Barrier::new(2));

        let conn = pool.acquire().await?;
        let read = spawn({
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                for i in 0u32..n {
                    let _v: Option<u32> = query_scalar("SELECT v FROM kv WHERE k = ?")
                        .bind(i)
                        .fetch_optional(&conn)
                        .await?;
                }
                Ok::<_, anyhow::Error>(())
            }
        });

        let write = spawn({
            let barrier = Arc::clone(&barrier);
            async move {
                barrier.wait().await;
                for i in 0u32..n {
                    query("INSERT INTO kv (k, v) VALUES (?, ?)")
                        .bind(i)
                        .bind(i * i)
                        .execute(&pool)
                        .await?;
                }
                Ok::<_, anyhow::Error>(())
            }
        });

        read.await??;
        write.await??;

        Ok(())
    }

    #[tokio::test]
    async fn it_binds_strings() -> anyhow::Result<()> {
        let conn = connection().await?;

        let row: (String, String, String) = musq::query_as("select ?1, ?2, ?3")
            .bind("1")
            .bind("2".to_string())
            .bind(Arc::new("3".to_string()))
            .fetch_one(&conn)
            .await?;

        assert_eq!(row.0, "1");
        assert_eq!(row.1, "2");
        assert_eq!(row.2, "3");

        Ok(())
    }

    #[tokio::test]
    async fn it_decodes_utf16_text_as_utf8() -> anyhow::Result<()> {
        let conn = connection().await?;

        query("PRAGMA encoding = 'UTF-16le'").execute(&conn).await?;

        query("CREATE TABLE t (val TEXT)").execute(&conn).await?;
        query("INSERT INTO t (val) VALUES ('café')")
            .execute(&conn)
            .await?;

        let val: String = query_scalar("SELECT val FROM t").fetch_one(&conn).await?;

        assert_eq!(val, "café");

        Ok(())
    }

    #[tokio::test]
    async fn it_errors_on_invalid_utf8_text() -> anyhow::Result<()> {
        let conn = connection().await?;

        let res: musq::Result<String> = query_scalar("SELECT CAST(X'80' AS TEXT)")
            .fetch_one(&conn)
            .await;

        let err = res.unwrap_err();
        assert!(matches!(err, Error::Decode(_)), "{err:?}");
        assert!(err.to_string().contains("invalid UTF-8"), "{err}");

        Ok(())
    }

    #[tokio::test]
    async fn it_fails_on_missing_bind() -> anyhow::Result<()> {
        let conn = connection().await?;

        let res = musq::query("select ?1, ?2, ?4")
            .bind(10_i32)
            .bind(11_i32)
            .fetch_one(&conn)
            .await;

        assert!(res.is_err());

        let err = res.err().unwrap();

        match err {
            Error::Protocol(msg) => {
                assert!(msg.contains("index is 4"));
            }
            _ => panic!("expected protocol error, got {err:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn connection_drops_without_close() -> anyhow::Result<()> {
        use musq_test::connection;

        let conn = connection().await?;
        drop(conn);

        // ensure a new connection can be established after dropping
        let conn2 = connection().await?;
        conn2.close().await?;

        Ok(())
    }
}
