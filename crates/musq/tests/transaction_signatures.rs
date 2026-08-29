//! Compile coverage for transaction type defaults.

#[cfg(test)]
mod tests {
    use musq::{Connection, Musq, Transaction};

    fn accepts_pool_transaction(_: &Transaction) {}

    fn accepts_borrowed_transaction(_: &Transaction<&mut Connection>) {}

    fn accepts_nested_transaction(_: &Transaction<&mut Connection>) {}

    #[tokio::test]
    async fn transaction_signatures_compile() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        let pool_transaction = pool.begin().await?;
        accepts_pool_transaction(&pool_transaction);

        let mut connection = Connection::connect_with(&Musq::new()).await?;
        let mut borrowed = connection.begin().await?;
        accepts_borrowed_transaction(&borrowed);

        let nested = borrowed.begin().await?;
        accepts_nested_transaction(&nested);
        nested.rollback().await?;
        borrowed.rollback().await?;
        pool_transaction.rollback().await?;
        Ok(())
    }

    #[tokio::test]
    async fn transaction_closure_borrows_local_across_await() -> anyhow::Result<()> {
        let pool = Musq::new().open_in_memory().await?;
        musq::query("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
            .execute(&pool)
            .await?;
        let name = String::from("Alice");
        pool.transaction(async |tx| {
            musq::query("INSERT INTO users (id, name) VALUES (1, ?)")
                .bind(&name)
                .execute(&*tx)
                .await?;
            Ok::<_, musq::Error>(())
        })
        .await?;
        let count: i64 = musq::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&pool)
            .await?;
        assert_eq!(count, 1);
        Ok(())
    }
}
