//! Integration tests for musq.

#[cfg(test)]
mod tests {
    use std::iter::empty;

    use musq::{Conditions, Error, Execute, Musq, QueryBuilder, query};

    #[test]
    fn push_values_empty_iterator_emits_nothing() -> anyhow::Result<()> {
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 1 WHERE 1 IN (");
        builder.push_values(empty::<i32>())?;
        builder.push_sql(")");
        assert_eq!(builder.build().sql(), "SELECT 1 WHERE 1 IN ()");
        Ok(())
    }

    #[test]
    fn push_idents_empty_iterator_returns_error() {
        let mut builder = QueryBuilder::new();
        let result = builder.push_idents(empty::<&str>());
        match result {
            Err(Error::Query(msg)) => assert!(msg.contains("empty idents")),
            other => panic!("expected query error, got {other:?}"),
        }
    }

    #[test]
    fn push_bind_named_normalizes_prefixed_names() -> anyhow::Result<()> {
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT ");
        builder.push_bind_named(":foo", &1_i32)?;
        let query = builder.build();
        assert_eq!(query.sql(), "SELECT :foo");
        Ok(())
    }

    #[test]
    fn empty_conditions_emit_nothing() -> anyhow::Result<()> {
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 1");
        builder.push_conditions(Conditions::new())?;
        assert_eq!(builder.build().sql(), "SELECT 1");
        Ok(())
    }

    #[test]
    fn conditions_join_typed_fragments_and_propagate_taint() -> anyhow::Result<()> {
        let grouped = query("(status = ? OR status = ?)")
            .try_bind("open")?
            .try_bind("pending")?;
        let mut raw = QueryBuilder::new();
        raw.push_raw("source = 'operator'");

        let conditions = Conditions::new()
            .with(query("owner = ?").try_bind("Ada")?)
            .with(grouped)
            .with(raw.build());
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT id FROM jobs");
        builder.push_conditions(conditions)?;
        let built = builder.build();

        assert_eq!(
            built.sql(),
            "SELECT id FROM jobs WHERE owner = ? AND (status = ? OR status = ?) AND source = 'operator'"
        );
        assert!(built.is_tainted());
        Ok(())
    }

    #[test]
    fn conditions_reject_numeric_parameters() {
        let conditions = Conditions::new().with(query("id = ?1").bind(1));
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT id FROM jobs");
        let error = builder.push_conditions(conditions).unwrap_err();
        assert!(error.to_string().contains("numeric SQL parameters"));
    }

    #[tokio::test]
    async fn conditions_preserve_argument_order_and_named_rebasing() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let conditions = Conditions::new()
            .with(query("? = :value").bind(1).bind_named("value", 1))
            .with(query(":value = ?").bind_named("value", 2).bind(2));
        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 7");
        builder.push_conditions(conditions)?;

        let value: i64 = builder
            .build()
            .try_map(|row| row.get_value_idx(0))
            .fetch_one(&pool)
            .await?;
        assert_eq!(value, 7);
        Ok(())
    }

    #[derive(Debug, PartialEq, musq::FromRow)]
    struct MappedRow {
        id: i64,
        name: String,
    }

    #[tokio::test]
    async fn build_query_as_maps_rows_and_decode_errors() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;

        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 7 AS id, 'Ada' AS name");
        let row = builder
            .build_query_as::<MappedRow>()
            .fetch_one(&pool)
            .await?;
        assert_eq!(
            row,
            MappedRow {
                id: 7,
                name: "Ada".into()
            }
        );

        let mut builder = QueryBuilder::new();
        builder.push_sql("SELECT 'not-an-integer' AS id, 'Ada' AS name");
        let error = builder
            .build_query_as::<MappedRow>()
            .fetch_one(&pool)
            .await
            .unwrap_err();
        assert!(matches!(error, Error::ColumnDecode { .. }));
        Ok(())
    }
}
