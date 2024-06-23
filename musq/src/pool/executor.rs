use either::Either;

use futures_core::{future::BoxFuture, stream::BoxStream};
use futures_util::TryStreamExt;

use crate::{
    executor::{Execute, Executor},
    pool::Pool,
    sqlite, try_stream, Connection, QueryResult, Result, Row, Statement,
};

impl<'p> Executor<'p> for &'_ Pool
where
    for<'c> &'c mut Connection: Executor<'c>,
{
    fn fetch_many<'e, 'q: 'e, E>(self, query: E) -> BoxStream<'e, Result<Either<QueryResult, Row>>>
    where
        E: Execute<'q> + 'q,
    {
        let pool = self.clone();

        Box::pin(try_stream! {
            let mut conn = pool.acquire().await?;
            let mut s = conn.fetch_many(query);

            while let Some(v) = s.try_next().await? {
                r#yield!(v);
            }

            Ok(())
        })
    }

    fn fetch_optional<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Option<Row>>>
    where
        E: Execute<'q> + 'q,
    {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.fetch_optional(query).await })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlite::SqliteDataType],
    ) -> BoxFuture<'e, Result<Statement>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.prepare_with(sql, parameters).await })
    }
}
