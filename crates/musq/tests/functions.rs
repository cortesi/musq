//! User-defined scalar functions and collations.

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use musq::{Connection, FunctionFlags, Musq, Value, query, query_scalar};

    fn int(value: i64) -> Value {
        Value::Integer {
            value,
            type_info: None,
        }
    }

    #[tokio::test]
    async fn deterministic_function_in_where() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().function(
            "twice",
            1,
            FunctionFlags {
                deterministic: true,
                ..FunctionFlags::default()
            },
            |args| Ok(int(args[0].int64().map_err(musq::Error::Decode)? * 2)),
        ))
        .await?;
        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;
        query("INSERT INTO t (x) VALUES (1), (2), (3)")
            .execute(&conn)
            .await?;
        let values: Vec<i64> = query_scalar("SELECT x FROM t WHERE twice(x) = 4")
            .fetch_all(&conn)
            .await?;
        assert_eq!(values, [2]);
        Ok(())
    }

    #[tokio::test]
    async fn function_error_is_returned() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().function(
            "fail",
            0,
            FunctionFlags::default(),
            |_| Err(musq::Error::Query("nope".into())),
        ))
        .await?;
        let err = match query("SELECT fail()").fetch_one(&conn).await {
            Ok(_) => panic!("function should return an error"),
            Err(error) => error,
        };
        assert!(err.to_string().contains("nope"), "{err}");
        Ok(())
    }

    #[tokio::test]
    async fn function_panic_is_caught() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().function(
            "boom",
            0,
            FunctionFlags::default(),
            |_| panic!("boom"),
        ))
        .await?;
        let err = match query("SELECT boom()").fetch_one(&conn).await {
            Ok(_) => panic!("function should return an error"),
            Err(error) => error,
        };
        assert!(err.to_string().contains("musq: function panicked"), "{err}");
        Ok(())
    }

    #[tokio::test]
    async fn innocuous_function_may_be_used_in_an_index() -> anyhow::Result<()> {
        let flags = FunctionFlags {
            deterministic: true,
            direct_only: false,
            innocuous: true,
        };
        let conn = Connection::connect_with(&Musq::new().function("twice", 1, flags, |args| {
            Ok(int(args[0].int64().map_err(musq::Error::Decode)? * 2))
        }))
        .await?;
        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;
        query("CREATE INDEX i ON t (twice(x))")
            .execute(&conn)
            .await?;
        query("INSERT INTO t (x) VALUES (21)")
            .execute(&conn)
            .await?;

        let values: Vec<i64> = query_scalar("SELECT x FROM t WHERE twice(x) = 42")
            .fetch_all(&conn)
            .await?;
        assert_eq!(values, [21]);
        Ok(())
    }

    #[tokio::test]
    async fn non_innocuous_function_is_rejected_in_an_index() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().function(
            "twice",
            1,
            FunctionFlags {
                deterministic: true,
                direct_only: true,
                innocuous: false,
            },
            |args| Ok(int(args[0].int64().map_err(musq::Error::Decode)? * 2)),
        ))
        .await?;
        query("CREATE TABLE t (x INTEGER)").execute(&conn).await?;
        let err = query("CREATE INDEX i ON t (twice(x))")
            .execute(&conn)
            .await
            .unwrap_err();
        assert!(err.as_sqlite().is_some(), "{err}");
        Ok(())
    }

    #[tokio::test]
    async fn collation_orders_rows() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().collation("rev", |a, b| b.cmp(a))).await?;
        query("CREATE TABLE t (name TEXT)").execute(&conn).await?;
        query("INSERT INTO t (name) VALUES ('a'), ('b'), ('c')")
            .execute(&conn)
            .await?;
        let names: Vec<String> = query_scalar("SELECT name FROM t ORDER BY name COLLATE rev")
            .fetch_all(&conn)
            .await?;
        assert_eq!(names, ["c", "b", "a"]);
        Ok(())
    }

    #[tokio::test]
    async fn function_is_registered_on_every_pool_connection() -> anyhow::Result<()> {
        let pool = Musq::new()
            .max_connections(2)
            .function("twice", 1, FunctionFlags::default(), |args| {
                Ok(int(args[0].int64().map_err(musq::Error::Decode)? * 2))
            })
            .open_in_memory()
            .await?;
        let a = pool.acquire().await?;
        let b = pool.acquire().await?;
        let left: i64 = query_scalar("SELECT twice(3)").fetch_one(&a).await?;
        let right: i64 = query_scalar("SELECT twice(4)").fetch_one(&b).await?;
        assert_eq!(left, 6);
        assert_eq!(right, 8);
        Ok(())
    }

    #[tokio::test]
    async fn function_round_trips_text_and_blob_values() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new().function(
            "echo",
            1,
            FunctionFlags::default(),
            |args| Ok(args[0].clone()),
        ))
        .await?;

        let text: String = query_scalar("SELECT echo(?)")
            .bind("héllo 🌍")
            .fetch_one(&conn)
            .await?;
        assert_eq!(text, "héllo 🌍");

        let empty_text: String = query_scalar("SELECT echo(?)")
            .bind("")
            .fetch_one(&conn)
            .await?;
        assert_eq!(empty_text, "");

        let blob: Vec<u8> = query_scalar("SELECT echo(?)")
            .bind(&[0_u8, 1, 255][..])
            .fetch_one(&conn)
            .await?;
        assert_eq!(blob, [0_u8, 1, 255]);

        let empty_blob: Vec<u8> = query_scalar("SELECT echo(?)")
            .bind(&[][..])
            .fetch_one(&conn)
            .await?;
        assert!(empty_blob.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn invalid_function_registration_returns_error_without_double_free() -> anyhow::Result<()>
    {
        struct DropCounter(Arc<AtomicUsize>);
        impl Drop for DropCounter {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let guard = DropCounter(Arc::clone(&drops));
        let options = Musq::new().function("bad_arity", -2, FunctionFlags::default(), move |_| {
            let _ = &guard;
            Ok(Value::Null { type_info: None })
        });

        let err = Connection::connect_with(&options)
            .await
            .expect_err("SQLite should reject n_args = -2");
        assert!(err.as_sqlite().is_some(), "{err}");
        assert_eq!(drops.load(Ordering::SeqCst), 0);

        drop(options);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
