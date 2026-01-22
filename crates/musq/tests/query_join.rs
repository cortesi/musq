//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use musq::{Execute, query::Query, sql};

    use crate::support::connection;

    // Helper to extract i32 from first column
    async fn fetch_vals(q: Query) -> anyhow::Result<Vec<i32>> {
        let conn = connection().await?;
        Ok(q.try_map(|row| row.get_value_idx::<i32>(0))
            .fetch_all(&conn)
            .await?)
    }

    #[tokio::test]
    async fn join_positional() -> anyhow::Result<()> {
        let q1 = sql!("SELECT {}", 1)?;
        let q2 = sql!("UNION SELECT {}", 2)?;
        let q = q1.join(q2);
        assert_eq!(q.sql(), "SELECT ? UNION SELECT ?");
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![1, 2]);
        Ok(())
    }

    #[tokio::test]
    async fn join_named() -> anyhow::Result<()> {
        let q1 = sql!("SELECT {a}", a = 3)?;
        let q2 = sql!("UNION SELECT {b}", b = 4)?;
        let q = q1.join(q2);
        assert_eq!(q.sql(), "SELECT :a UNION SELECT :b");
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![3, 4]);
        Ok(())
    }

    #[tokio::test]
    async fn join_named_collision() -> anyhow::Result<()> {
        let q1 = sql!("SELECT {a}", a = 1)?;
        let q2 = sql!("UNION SELECT {a}", a = 2)?;
        let q = q1.join(q2);
        assert_eq!(q.sql(), "SELECT :a UNION SELECT :a_1");
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![1, 2]);
        Ok(())
    }

    #[tokio::test]
    async fn join_mixed_args() -> anyhow::Result<()> {
        let q1 = sql!("SELECT {a}", a = 5)?;
        let q2 = sql!("UNION SELECT {}", 6)?;
        let q = q1.join(q2);
        assert_eq!(q.sql(), "SELECT :a UNION SELECT ?");
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![5, 6]);
        Ok(())
    }

    #[tokio::test]
    async fn join_limit_clause() -> anyhow::Result<()> {
        let base =
            sql!("SELECT value FROM (SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3)")?;
        let limit = sql!("LIMIT {}", 2)?;
        let q = base.join(limit);
        assert_eq!(
            q.sql(),
            "SELECT value FROM (SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3) LIMIT ?"
        );
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![1, 2]);
        Ok(())
    }

    #[tokio::test]
    async fn join_dynamic_where() -> anyhow::Result<()> {
        let mut q = sql!(
            "SELECT value FROM (SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3) WHERE 1 = 1"
        )?;
        let add = Some(2);
        if let Some(v) = add {
            q = q.join(sql!("AND value >= {}", v)?);
        }
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![2, 3]);
        Ok(())
    }

    fn order_by_desc() -> Query {
        sql!("ORDER BY value DESC").unwrap()
    }

    #[tokio::test]
    async fn join_reusable_fragment() -> anyhow::Result<()> {
        let q =
            sql!("SELECT value FROM (SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3)")?
                .join(order_by_desc());
        assert_eq!(
            q.sql(),
            "SELECT value FROM (SELECT 1 AS value UNION ALL SELECT 2 UNION ALL SELECT 3) ORDER BY value DESC"
        );
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![3, 2, 1]);
        Ok(())
    }

    #[tokio::test]
    async fn join_cte() -> anyhow::Result<()> {
        let with = sql!("WITH nums(x) AS (VALUES (1),(2))")?;
        let select = sql!("SELECT x FROM nums")?;
        let q = with.join(select);
        assert_eq!(
            q.sql(),
            "WITH nums(x) AS (VALUES (1),(2)) SELECT x FROM nums"
        );
        let vals = fetch_vals(q).await?;
        assert_eq!(vals, vec![1, 2]);
        Ok(())
    }
}
