//! Per-connection SQLite authorizer and runtime limit tests.

mod support;

#[cfg(test)]
mod tests {
    use musq::{
        ActionCode, AuthResult, SqliteLimit, error::PrimaryErrCode, query, query_as, query_scalar,
    };

    use crate::support::connection;

    /// Authorizer policy that allows read-only queries exclusively through an
    /// allowlisted `TEMP` view and a fixed set of safe SQL functions.
    fn temp_view_only_authorizer(
        action: ActionCode,
        arg1: Option<&str>,
        arg2: Option<&str>,
        db_name: Option<&str>,
        trigger_or_view: Option<&str>,
    ) -> AuthResult {
        match action {
            ActionCode::Select => AuthResult::Ok,
            ActionCode::Read => {
                let is_allowed_view = db_name == Some("temp") && arg1 == Some("v_public_items");
                let via_allowed_view = trigger_or_view == Some("v_public_items");
                if is_allowed_view || via_allowed_view {
                    AuthResult::Ok
                } else {
                    AuthResult::Deny
                }
            }
            ActionCode::Function => {
                if matches!(arg2, Some("count" | "lower" | "upper" | "length" | "sum")) {
                    AuthResult::Ok
                } else {
                    AuthResult::Deny
                }
            }
            _ => AuthResult::Deny,
        }
    }

    #[tokio::test]
    async fn authorizer_allows_temp_view_and_denies_direct_table_pragma_attach_and_writes()
    -> anyhow::Result<()> {
        let conn = connection().await?;
        query(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, secret TEXT NOT NULL)",
        )
        .execute(&conn)
        .await?;
        query("INSERT INTO items (id, name, secret) VALUES (1, 'Alpha', 's1'), (2, 'Beta', 's2')")
            .execute(&conn)
            .await?;
        query("CREATE TEMP VIEW v_public_items AS SELECT id, name FROM main.items")
            .execute(&conn)
            .await?;

        // Warm the statement cache with a direct table read before installing
        // the authorizer to verify cache invalidation.
        let cached: String = query_scalar("SELECT secret FROM items WHERE id = 1")
            .fetch_one(&conn)
            .await?;
        assert_eq!(cached, "s1");

        conn.set_authorizer(temp_view_only_authorizer).await?;

        // Allowed: SELECT through TEMP view (qualified and unqualified) and
        // allowed SQL functions.
        let rows: Vec<(i64, String)> =
            query_as("SELECT id, lower(name) FROM temp.v_public_items ORDER BY id")
                .fetch_all(&conn)
                .await?;
        assert_eq!(rows, vec![(1, "alpha".into()), (2, "beta".into())]);

        let upper_name: String =
            query_scalar("SELECT upper(name) FROM v_public_items WHERE id = 2")
                .fetch_one(&conn)
                .await?;
        assert_eq!(upper_name, "BETA");

        // Denied: direct base-table SELECT (even though it was previously
        // cached).
        let direct_err = query_scalar::<String>("SELECT secret FROM items WHERE id = 1")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            direct_err.as_sqlite().map(|err| err.primary),
            Some(PrimaryErrCode::Auth)
        );
        let direct_public_col_err = query_scalar::<i64>("SELECT id FROM items WHERE id = 1")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            direct_public_col_err.as_sqlite().map(|err| err.primary),
            Some(PrimaryErrCode::Auth)
        );

        // Denied: PRAGMA.
        let pragma_err = query("PRAGMA table_info(items)")
            .execute(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            pragma_err.as_sqlite().map(|err| err.primary),
            Some(PrimaryErrCode::Auth)
        );

        // Denied: ATTACH.
        let attach_err = query("ATTACH DATABASE ':memory:' AS extra")
            .execute(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            attach_err.as_sqlite().map(|err| err.primary),
            Some(PrimaryErrCode::Auth)
        );

        // Denied: writes (INSERT, UPDATE, DELETE).
        for sql in [
            "INSERT INTO items (id, name, secret) VALUES (3, 'Gamma', 's3')",
            "UPDATE items SET name = 'X' WHERE id = 1",
            "DELETE FROM items WHERE id = 1",
        ] {
            let err = query(sql).execute(&conn).await.unwrap_err();
            assert_eq!(
                err.as_sqlite().map(|sqlite| sqlite.primary),
                Some(PrimaryErrCode::Auth),
                "expected auth failure for: {sql}"
            );
        }

        // Denied: unapproved SQL function.
        let fn_err = query_scalar::<String>("SELECT sqlite_version()")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        let fn_sqlite = fn_err.as_sqlite().expect("SQLite authorization error");
        assert!(
            fn_sqlite.primary == PrimaryErrCode::Auth
                || fn_sqlite.message.contains("not authorized"),
            "unexpected function denial error: {fn_sqlite:?}"
        );

        // Clearing the authorizer restores full access.
        conn.clear_authorizer().await?;
        let restored: String = query_scalar("SELECT secret FROM items WHERE id = 1")
            .fetch_one(&conn)
            .await?;
        assert_eq!(restored, "s1");

        conn.close().await?;
        Ok(())
    }

    #[tokio::test]
    async fn authorizer_ignore_masks_column_as_null() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE accounts (id INTEGER PRIMARY KEY, secret TEXT NOT NULL)")
            .execute(&conn)
            .await?;
        query("INSERT INTO accounts (id, secret) VALUES (1, 'top-secret')")
            .execute(&conn)
            .await?;

        conn.set_authorizer(|action, _arg1, arg2, _db, _view| match action {
            ActionCode::Select => AuthResult::Ok,
            ActionCode::Read if arg2 == Some("secret") => AuthResult::Ignore,
            ActionCode::Read => AuthResult::Ok,
            _ => AuthResult::Deny,
        })
        .await?;

        let row: (i64, Option<String>) = query_as("SELECT id, secret FROM accounts WHERE id = 1")
            .fetch_one(&conn)
            .await?;
        assert_eq!(row, (1, None));
        Ok(())
    }

    #[tokio::test]
    async fn authorizer_panic_fails_closed() -> anyhow::Result<()> {
        let conn = connection().await?;
        query("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;

        conn.set_authorizer(|_, _, _, _, _| panic!("authorizer boom"))
            .await?;

        let err = query_scalar::<i64>("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_sqlite().map(|sqlite| sqlite.primary),
            Some(PrimaryErrCode::Auth)
        );

        // Replacing or clearing the authorizer leaves the connection usable.
        conn.clear_authorizer().await?;
        let count: i64 = query_scalar("SELECT COUNT(*) FROM t")
            .fetch_one(&conn)
            .await?;
        assert_eq!(count, 0);
        Ok(())
    }

    #[tokio::test]
    async fn runtime_limits_can_be_inspected_and_enforced() -> anyhow::Result<()> {
        let conn = connection().await?;

        let prev_compound = conn.set_limit(SqliteLimit::CompoundSelect, 2).await?;
        assert!(prev_compound >= 2);
        assert_eq!(conn.limit(SqliteLimit::CompoundSelect).await?, 2);

        let err = query_scalar::<i64>("SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert!(err.as_sqlite().is_some());

        let prev_expr = conn.set_limit(SqliteLimit::ExprDepth, 4).await?;
        assert!(prev_expr >= 4);
        assert_eq!(conn.limit(SqliteLimit::ExprDepth).await?, 4);

        let deep_expr = format!("SELECT {}1{}", "(1 + ".repeat(20), ")".repeat(20));
        let err = query_scalar::<i64>(&deep_expr)
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert!(err.as_sqlite().is_some());

        let prev_len = conn.set_limit(SqliteLimit::Length, 1024).await?;
        assert!(prev_len >= 1024);
        assert_eq!(conn.limit(SqliteLimit::Length).await?, 1024);

        let err = query_scalar::<Vec<u8>>("SELECT zeroblob(4096)")
            .fetch_one(&conn)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_sqlite().map(|sqlite| sqlite.primary),
            Some(PrimaryErrCode::TooBig)
        );

        Ok(())
    }
}
