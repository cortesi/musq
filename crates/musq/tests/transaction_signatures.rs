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
}
