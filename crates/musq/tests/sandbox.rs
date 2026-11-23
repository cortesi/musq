//! Integration tests for `ReadOnlySandbox`, composable `QuerySurface` unions,
//! pre-authorizer `prepare_hooks`, authorizer enforcement, limits, timeouts,
//! row/byte caps, and `fleet_query`.

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashMap},
        path::Path,
        sync::Arc,
        time::Duration,
    };

    use musq::{
        Connection, Database, DatabaseOptions, Error, FunctionFlags, Module, QuerySurface,
        ReadOnlySandbox, Result, SandboxError, SandboxQueryOptions, Text, Transaction, Value,
        fleet_query, query, query_as,
    };
    use tempfile::tempdir;

    /// Module A exposing `v_a_items` over `a_items` and a pre-auth scalar
    /// function `a_secret_lookup(id)` that reads raw `a_secrets` before the
    /// authorizer locks down raw tables.
    struct ModA;

    impl Module for ModA {
        const ID: &'static str = "sandbox.mod_a";
        const TABLE_PREFIX: &'static str = "a_";
        const SCHEMA_VERSION: u32 = 1;

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TABLE a_items (
                    id INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    internal_secret TEXT NOT NULL
                 )",
            )
            .execute(&**tx)
            .await?;
            query(
                "CREATE TABLE a_secrets (
                    item_id INTEGER PRIMARY KEY REFERENCES a_items(id),
                    payload TEXT NOT NULL
                 )",
            )
            .execute(&**tx)
            .await?;
            Ok(())
        }

        fn query_surface(&self) -> QuerySurface {
            QuerySurface::new()
                .with_temp_view("CREATE TEMP VIEW v_a_items AS SELECT id, name FROM main.a_items;")
                .with_allowed_view("v_a_items")
                .with_count_only_table("a_items")
                .with_allowed_function("a_secret_lookup")
                .with_prepare_hook(|conn, sql| {
                    let needed = sql.to_ascii_lowercase().contains("a_secret_lookup");
                    Box::pin(async move {
                        let map: Arc<HashMap<i64, String>> = if needed {
                            let rows: Vec<(i64, String)> =
                                query_as("SELECT item_id, payload FROM a_secrets")
                                    .fetch_all(&*conn)
                                    .await?;
                            Arc::new(rows.into_iter().collect())
                        } else {
                            Arc::new(HashMap::new())
                        };
                        conn.register_function(
                            "a_secret_lookup",
                            1,
                            FunctionFlags {
                                deterministic: true,
                                direct_only: true,
                                innocuous: false,
                            },
                            move |args| {
                                let id = args[0].int64().unwrap_or(-1);
                                Ok(match map.get(&id) {
                                    Some(text) => Value::Text {
                                        value: Text::new(text.clone()),
                                        type_info: None,
                                    },
                                    None => Value::Null { type_info: None },
                                })
                            },
                        )
                        .await
                    })
                })
        }
    }

    /// Module B depending on Module A, exposing `v_b_tags` and a stateless
    /// custom scalar function `b_shout(text)`.
    struct ModB;

    impl Module for ModB {
        const ID: &'static str = "sandbox.mod_b";
        const TABLE_PREFIX: &'static str = "b_";
        const SCHEMA_VERSION: u32 = 1;
        const DEPENDS_ON: &'static [&'static str] = &["sandbox.mod_a"];

        async fn install(tx: &mut Transaction<&mut Connection>) -> Result<()> {
            query(
                "CREATE TABLE b_tags (
                    id INTEGER PRIMARY KEY,
                    item_id INTEGER NOT NULL REFERENCES a_items(id),
                    tag TEXT NOT NULL
                 )",
            )
            .execute(&**tx)
            .await?;
            Ok(())
        }

        fn query_surface(&self) -> QuerySurface {
            QuerySurface::new()
                .with_temp_view(
                    "CREATE TEMP VIEW v_b_tags AS SELECT id, item_id, tag FROM main.b_tags;",
                )
                .with_allowed_view("v_b_tags")
                .with_count_only_table("b_tags")
                .with_scalar_function(
                    "b_shout",
                    1,
                    FunctionFlags {
                        deterministic: true,
                        direct_only: true,
                        innocuous: false,
                    },
                    |args| {
                        let text = args[0].text().unwrap_or("");
                        Ok(Value::Text {
                            value: Text::new(format!("{}!", text.to_ascii_uppercase())),
                            type_info: None,
                        })
                    },
                )
        }
    }

    /// Seed one database with ModA + ModB and sample rows.
    async fn seed_db(path: &Path, prefix: &str) -> anyhow::Result<Database> {
        let db = Database::open(path, DatabaseOptions::new()).await?;
        db.mount(ModA).await?;
        db.mount(ModB).await?;
        let prefix = prefix.to_owned();
        db.write(async move |tx| {
            for idx in 1_i64..=5 {
                query("INSERT INTO a_items (id, name, internal_secret) VALUES (?, ?, ?)")
                    .bind(idx)
                    .bind(format!("{prefix}-item-{idx}"))
                    .bind(format!("hidden-{idx}"))
                    .execute(&**tx)
                    .await?;
                query("INSERT INTO a_secrets (item_id, payload) VALUES (?, ?)")
                    .bind(idx)
                    .bind(format!("decoded-{prefix}-{idx}"))
                    .execute(&**tx)
                    .await?;
                query("INSERT INTO b_tags (id, item_id, tag) VALUES (?, ?, ?)")
                    .bind(idx)
                    .bind(idx)
                    .bind(format!("tag{idx}"))
                    .execute(&**tx)
                    .await?;
            }
            Ok::<_, Error>(())
        })
        .await?;
        Ok(db)
    }

    #[tokio::test]
    async fn co_mounted_views_custom_functions_and_params() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("sandbox.db");
        let db = seed_db(&db_path, "db1").await?;

        let sandbox = ReadOnlySandbox::new(&db);
        let mut params = BTreeMap::new();
        params.insert(
            "min_id".to_owned(),
            Value::Integer {
                value: 3,
                type_info: None,
            },
        );

        let res = sandbox
            .query(
                "SELECT a.id, a.name, b_shout(b.tag) AS loud_tag, a_secret_lookup(a.id) AS secret
                 FROM v_a_items a
                 JOIN v_b_tags b ON b.item_id = a.id
                 WHERE a.id >= :min_id
                 ORDER BY a.id ASC",
                &params,
                &SandboxQueryOptions::new(),
            )
            .await?;

        assert!(!res.truncated);
        assert_eq!(res.columns, ["id", "name", "loud_tag", "secret"]);
        assert_eq!(res.rows.len(), 3);
        assert_eq!(res.rows[0][0].int64()?, 3);
        assert_eq!(res.rows[0][1].text()?, "db1-item-3");
        assert_eq!(res.rows[0][2].text()?, "TAG3!");
        assert_eq!(res.rows[0][3].text()?, "decoded-db1-3");

        // `count(*)` flattener works on count_only_tables.
        let count_res = sandbox
            .query(
                "SELECT count(*) AS n FROM v_a_items",
                &BTreeMap::new(),
                &SandboxQueryOptions::new(),
            )
            .await?;
        assert_eq!(count_res.rows[0][0].int64()?, 5);

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn blocks_catalog_raw_tables_writes_pragmas_attach_and_multi_statement()
    -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("blocked.db");
        let db = seed_db(&db_path, "blk").await?;
        let sandbox = ReadOnlySandbox::new(&db);
        let empty = BTreeMap::new();
        let opts = SandboxQueryOptions::new();

        let forbidden_queries = [
            "SELECT * FROM _musq_modules",
            "SELECT * FROM sqlite_schema",
            "SELECT * FROM sqlite_master",
            "SELECT * FROM a_items",
            "SELECT * FROM a_secrets",
            "SELECT internal_secret FROM a_items",
            "INSERT INTO a_items (id, name, internal_secret) VALUES (99, 'x', 'y')",
            "UPDATE a_items SET name = 'pwned'",
            "DELETE FROM a_items",
            "PRAGMA table_info(a_items)",
            "PRAGMA user_version",
            "ATTACH DATABASE ':memory:' AS extra",
            "SELECT sqlite_version()",
            "SELECT * FROM v_a_items; SELECT * FROM v_b_tags",
            "   ;  ",
            "/* unterminated",
        ];

        for sql in forbidden_queries {
            let err = sandbox.query(sql, &empty, &opts).await.unwrap_err();
            assert!(
                matches!(err, Error::Sandbox(SandboxError::Rejected(_))),
                "expected SandboxError::Rejected for `{sql}`, got: {err:?}"
            );
        }

        // Even if a surface attempts to allowlist `_musq_modules` or
        // `sqlite_schema`, the authorizer hard-denies them.
        let malicious_surface = QuerySurface::new()
            .with_allowed_view("_musq_modules")
            .with_allowed_view("sqlite_schema")
            .with_count_only_table("_musq_modules")
            .with_count_only_table("sqlite_schema");
        let hard_sandbox = ReadOnlySandbox::from_surface(db.path(), false, malicious_surface);
        for sql in [
            "SELECT count(*) FROM _musq_modules",
            "SELECT count(*) FROM sqlite_schema",
        ] {
            let err = hard_sandbox.query(sql, &empty, &opts).await.unwrap_err();
            assert!(
                matches!(err, Error::Sandbox(SandboxError::Rejected(_))),
                "expected hard denial for `{sql}`, got: {err:?}"
            );
        }

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn enforces_row_cap_byte_cap_limits_and_timeout() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("caps.db");
        let db = seed_db(&db_path, "cap").await?;
        let sandbox = ReadOnlySandbox::new(&db);
        let empty = BTreeMap::new();

        // Row cap truncation.
        let row_capped = sandbox
            .query(
                "SELECT id, name FROM v_a_items ORDER BY id",
                &empty,
                &SandboxQueryOptions::new().max_rows(2),
            )
            .await?;
        assert!(row_capped.truncated);
        assert_eq!(row_capped.rows.len(), 2);

        // Byte cap truncation (always returns at least 1 row, stops before
        // 2nd).
        let byte_capped = sandbox
            .query(
                "SELECT id, name FROM v_a_items ORDER BY id",
                &empty,
                &SandboxQueryOptions::new().max_bytes(20),
            )
            .await?;
        assert!(byte_capped.truncated);
        assert_eq!(byte_capped.rows.len(), 1);

        // SQLite compound select limit enforcement.
        let limit_err = sandbox
            .query(
                "SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3",
                &empty,
                &SandboxQueryOptions::new().limit_compound_select(2),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(limit_err, Error::Sandbox(SandboxError::LimitExceeded(_))),
            "expected LimitExceeded, got {limit_err:?}"
        );

        // Statement timeout enforcement via recursive CTE.
        let timeout_err = sandbox
            .query(
                "WITH RECURSIVE cnt(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM cnt WHERE x < 100000000)
                 SELECT sum(x) FROM cnt",
                &empty,
                &SandboxQueryOptions::new().timeout(Duration::from_millis(25)),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(timeout_err, Error::Sandbox(SandboxError::Timeout(_))),
            "expected Timeout, got {timeout_err:?}"
        );

        db.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn fleet_query_merges_multiple_files_and_archives_with_global_caps() -> anyhow::Result<()>
    {
        let dir = tempdir()?;
        let p1 = dir.path().join("one.db");
        let p2 = dir.path().join("two.db");
        let arch2 = dir.path().join("two-archive.db");

        let db1 = seed_db(&p1, "first").await?;
        let db2 = seed_db(&p2, "second").await?;
        db2.export_archive(&arch2).await?;

        let surface = db1.query_surface();
        db1.close().await?;
        db2.close().await?;

        let empty = BTreeMap::new();
        let all = fleet_query(
            &[&p1, &arch2],
            &surface,
            "SELECT a.id, a.name, b.tag FROM v_a_items a JOIN v_b_tags b ON b.item_id = a.id ORDER BY a.id",
            &empty,
            &SandboxQueryOptions::new(),
        )
        .await?;

        assert!(!all.truncated);
        assert_eq!(all.columns, ["source_file", "id", "name", "tag"]);
        assert_eq!(all.rows.len(), 10);
        assert!(all.rows[0][0].text()?.ends_with("one.db"));
        assert_eq!(all.rows[0][2].text()?, "first-item-1");
        assert!(all.rows[5][0].text()?.ends_with("two-archive.db"));
        assert_eq!(all.rows[5][2].text()?, "second-item-1");

        // Global max_rows cap across files.
        let capped = fleet_query(
            &[&p1, &arch2],
            &surface,
            "SELECT id, name FROM v_a_items ORDER BY id",
            &empty,
            &SandboxQueryOptions::new().max_rows(7),
        )
        .await?;
        assert!(capped.truncated);
        assert_eq!(capped.rows.len(), 7);

        // Immutable archive query via from_surface.
        let imm_sandbox = ReadOnlySandbox::from_surface(&arch2, true, surface);
        let imm_res = imm_sandbox
            .query(
                "SELECT id, a_secret_lookup(id) FROM v_a_items WHERE id = 2",
                &empty,
                &SandboxQueryOptions::new(),
            )
            .await?;
        assert_eq!(imm_res.rows.len(), 1);
        assert_eq!(imm_res.rows[0][1].text()?, "decoded-second-2");

        Ok(())
    }

    #[cfg(feature = "json")]
    #[tokio::test]
    async fn json_param_and_row_helpers_round_trip() -> anyhow::Result<()> {
        use musq::{json_params_to_values, value_to_json};
        use serde_json::json;

        let dir = tempdir()?;
        let db_path = dir.path().join("json.db");
        let db = seed_db(&db_path, "js").await?;

        let mut json_params = BTreeMap::new();
        json_params.insert("id".to_owned(), json!(2));
        let values = json_params_to_values(&json_params)?;

        let sandbox = ReadOnlySandbox::new(&db);
        let res = sandbox
            .query(
                "SELECT id, name FROM v_a_items WHERE id = :id",
                &values,
                &SandboxQueryOptions::new(),
            )
            .await?;
        let json_rows = res.rows_as_json();
        assert_eq!(json_rows, vec![vec![json!(2), json!("js-item-2")]]);

        // Blob renders as lowercase hex.
        let blob = Value::Blob {
            value: vec![0xde, 0xad, 0xbe, 0xef].into(),
            type_info: None,
        };
        assert_eq!(value_to_json(&blob), json!("deadbeef"));

        // Non-scalar JSON parameter rejected.
        let mut bad = BTreeMap::new();
        bad.insert("arr".to_owned(), json!([1, 2]));
        assert!(matches!(
            json_params_to_values(&bad).unwrap_err(),
            Error::Sandbox(SandboxError::InvalidParameter(_))
        ));

        db.close().await?;
        Ok(())
    }
}
