//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::result::Result as StdResult;

    use musq::{
        Error, Result as MusqResult, Value, Values, encode::Encode, error::EncodeError, sql,
        sql_as, values,
    };
    use musq_test::connection;

    #[derive(musq::FromRow, Debug, PartialEq)]
    struct User {
        id: i32,
        name: String,
        status: String,
        last_login: String,
    }

    #[tokio::test]
    async fn dynamic_values_workflow() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, status TEXT, last_login TEXT)"
        )?
        .execute(&conn)
        .await?;

        let data: Values =
            values! { "id": 1, "name": "Alice", "status": "active", "last_login": "now" }?;
        sql!("INSERT INTO users {insert:data}")?
            .execute(&conn)
            .await?;

        let mut changes = Values::new();
        changes.insert("name", "Alicia")?;
        changes.insert("status", "inactive")?;
        sql!("UPDATE users SET {set:changes} WHERE id = 1")?
            .execute(&conn)
            .await?;

        let filters: Values = values! { "status": "inactive" }?;
        let user: User =
            sql_as!("SELECT id, name, status, last_login FROM users WHERE {where:filters}")?
                .fetch_one(&conn)
                .await?;
        assert_eq!(user.name, "Alicia");

        let upsert: Values =
            values! { "id": 1, "name": "Alicia", "status": "active", "last_login": "later" }?;
        sql!(
            "INSERT INTO users {insert:upsert} ON CONFLICT(id) DO UPDATE SET {upsert:upsert, exclude: id}"
        )?
        .execute(&conn)
        .await?;

        let user: User = sql_as!("SELECT id, name, status, last_login FROM users WHERE id = 1")?
            .fetch_one(&conn)
            .await?;
        assert_eq!(user.status, "active");
        assert_eq!(user.last_login, "later");
        Ok(())
    }

    #[tokio::test]
    async fn fluent_builder_insert() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE fb (id INTEGER PRIMARY KEY, name TEXT)")?
            .execute(&conn)
            .await?;

        let vals = Values::new().val("id", 1)?.val("name", "Bob")?;
        sql!("INSERT INTO fb {insert:vals}")?.execute(&conn).await?;

        let row: (i32, String) = sql_as!("SELECT id, name FROM fb")?.fetch_one(&conn).await?;
        assert_eq!(row, (1, "Bob".into()));
        Ok(())
    }

    #[tokio::test]
    async fn where_and_empty_returns_all() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE wa (id INTEGER)")?.execute(&conn).await?;
        for i in 0..3 {
            sql!("INSERT INTO wa VALUES ({})", i)?
                .execute(&conn)
                .await?;
        }
        let empty = Values::new();
        let rows: Vec<(i32,)> = sql_as!("SELECT id FROM wa WHERE {where:empty}")?
            .fetch_all(&conn)
            .await?;
        assert_eq!(rows.len(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn where_and_combined_static() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE wc (id INTEGER, active INTEGER)")?
            .execute(&conn)
            .await?;
        for i in 0..3 {
            let active = if i == 1 { 1 } else { 0 };
            sql!("INSERT INTO wc VALUES ({}, {})", i, active)?
                .execute(&conn)
                .await?;
        }
        let filters: Values = values! { "id": 1 }?;
        let rows: Vec<(i32,)> = sql_as!("SELECT id FROM wc WHERE active = 1 AND {where:filters}")?
            .fetch_all(&conn)
            .await?;
        assert_eq!(rows, vec![(1,)]);
        Ok(())
    }

    #[tokio::test]
    async fn upsert_set_exclude_key() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE us (id INTEGER PRIMARY KEY, val TEXT)")?
            .execute(&conn)
            .await?;
        let vals: Values = values! { "id": 1, "val": "a" }?;
        sql!("INSERT INTO us {insert:vals}")?.execute(&conn).await?;
        let new_vals: Values = values! { "id": 1, "val": "b" }?;
        sql!(
            "INSERT INTO us {insert:new_vals} \
             ON CONFLICT(id) DO UPDATE SET {upsert:new_vals, exclude: id}"
        )?
        .execute(&conn)
        .await?;
        let row: (String,) = sql_as!("SELECT val FROM us WHERE id = 1")?
            .fetch_one(&conn)
            .await?;
        assert_eq!(row.0, "b");
        Ok(())
    }

    #[tokio::test]
    async fn upsert_with_subset_columns() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE sub (id INTEGER PRIMARY KEY, name TEXT, logins INTEGER)")?
            .execute(&conn)
            .await?;
        let initial: Values = values! { "id": 1, "name": "a", "logins": 1 }?;
        sql!("INSERT INTO sub {insert:initial}")?
            .execute(&conn)
            .await?;
        let up: Values = values! { "id": 1, "logins": 2 }?;
        sql!(
            "INSERT INTO sub {insert:up} \
             ON CONFLICT(id) DO UPDATE SET {upsert:up, exclude: id}"
        )?
        .execute(&conn)
        .await?;
        let row: (String, i32) = sql_as!("SELECT name, logins FROM sub WHERE id = 1")?
            .fetch_one(&conn)
            .await?;
        assert_eq!(row, ("a".into(), 2));
        Ok(())
    }

    #[tokio::test]
    async fn insert_update_various_types() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE ty (id INTEGER PRIMARY KEY, b INTEGER, r REAL, bl BLOB, t TEXT)")?
            .execute(&conn)
            .await?;
        let vals: Values = values! {
            "id": 1,
            "b": true,
            "r": 1.23_f64,
            "bl": b"blob".as_slice(),
            "t": Option::<String>::None,
        }?;
        sql!("INSERT INTO ty {insert:vals}")?.execute(&conn).await?;
        let upd: Values = values! { "r": 2.0, "t": Some("hi") }?;
        sql!("UPDATE ty SET {set:upd} WHERE id = 1")?
            .execute(&conn)
            .await?;
        let row: (bool, f64, Vec<u8>, Option<String>) =
            sql_as!("SELECT b, r, bl, t FROM ty WHERE id = 1")?
                .fetch_one(&conn)
                .await?;
        assert_eq!(row, (true, 2.0, b"blob".to_vec(), Some("hi".into())));
        Ok(())
    }

    #[tokio::test]
    async fn empty_values_error() -> anyhow::Result<()> {
        let empty = Values::new();
        assert!(matches!(
            sql!("INSERT INTO t {insert:empty}"),
            Err(Error::Protocol(_))
        ));
        assert!(matches!(
            sql!("UPDATE t SET {set:empty}"),
            Err(Error::Protocol(_))
        ));
        Ok(())
    }

    struct Bad;
    impl Encode for Bad {
        fn encode(&self) -> StdResult<Value, EncodeError> {
            Err(EncodeError::Conversion("bad".into()))
        }
    }

    #[tokio::test]
    async fn values_macro_encode_error() -> anyhow::Result<()> {
        let res: MusqResult<Values> = (|| {
            values! { "b": Bad }
        })();
        assert!(res.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn macro_combination() -> anyhow::Result<()> {
        let conn = connection().await?;
        sql!("CREATE TABLE mix (id INTEGER PRIMARY KEY, name TEXT)")?
            .execute(&conn)
            .await?;
        let table = "mix";
        let vals: Values = values! { "id": 5, "name": "Old" }?;
        sql!("INSERT INTO {ident:table} {insert:vals}")?
            .execute(&conn)
            .await?;
        let changes: Values = values! { "name": "New" }?;
        let id = 5;
        sql!("UPDATE {ident:table} SET {set:changes} WHERE id = {}", id)?
            .execute(&conn)
            .await?;
        let row: (String,) = sql_as!("SELECT name FROM mix WHERE id = 5")?
            .fetch_one(&conn)
            .await?;
        assert_eq!(row.0, "New");
        Ok(())
    }

    #[tokio::test]
    async fn update_set_with_where_named_param() -> anyhow::Result<()> {
        use musq::{Values, sql, sql_as, values};
        let conn = connection().await?;

        sql!(
            "CREATE TABLE flows_repro (
                request_id TEXT PRIMARY KEY,
                response_status INTEGER,
                resource_ip_address_space TEXT
            )"
        )?
        .execute(&conn)
        .await?;

        let req_id_target = "req-123";
        let req_id_other = "req-999";
        sql!("INSERT INTO flows_repro (request_id, response_status, resource_ip_address_space) VALUES ({}, NULL, NULL)", req_id_target)?
            .execute(&conn)
            .await?;
        sql!("INSERT INTO flows_repro (request_id, response_status, resource_ip_address_space) VALUES ({}, NULL, NULL)", req_id_other)?
            .execute(&conn)
            .await?;

        let status_code: i64 = 204;
        let resource_ip_option = Some(String::from("Public"));
        let resource_ip_lower: Option<String> =
            resource_ip_option.as_deref().map(|s| s.to_lowercase());

        // Build the same `changes` map as in your function
        let changes: Values = values! {
            "response_status": status_code,
            "resource_ip_address_space": resource_ip_lower
        }?;

        sql!(
            "UPDATE flows_repro SET {set:changes} WHERE request_id = {request_id}",
            request_id = req_id_target
        )?
        .execute(&conn)
        .await?;

        let updated: (i64, String) = sql_as!(
            "SELECT response_status, resource_ip_address_space FROM flows_repro WHERE request_id = {}",
            req_id_target
        )?
        .fetch_one(&conn)
        .await?;
        assert_eq!(updated, (204, "public".to_string()));

        let untouched: (Option<i64>, Option<String>) = sql_as!(
            "SELECT response_status, resource_ip_address_space FROM flows_repro WHERE request_id = {}",
            req_id_other
        )?
        .fetch_one(&conn)
        .await?;
        assert_eq!(untouched, (None, None));

        Ok(())
    }
}
