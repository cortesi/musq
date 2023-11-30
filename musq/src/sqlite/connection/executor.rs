use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{TryFutureExt, TryStreamExt};

use crate::{
    error::Error,
    executor::{Execute, Executor},
    sqlite::{Connection, SqliteDataType, Statement},
    Either, QueryResult, Row,
};

impl<'c> Executor<'c> for &'c mut Connection {
    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        mut query: E,
    ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
    where
        'c: 'e,
        E: Execute<'q>,
    {
        let sql = query.sql();
        let arguments = query.take_arguments();

        Box::pin(
            self.worker
                .execute(sql, arguments, self.row_channel_size)
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream(),
        )
    }

    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        mut query: E,
    ) -> BoxFuture<'e, Result<Option<Row>, Error>>
    where
        'c: 'e,
        E: Execute<'q>,
    {
        let sql = query.sql();
        let arguments = query.take_arguments();

        Box::pin(async move {
            let stream = self
                .worker
                .execute(sql, arguments, self.row_channel_size)
                .map_ok(flume::Receiver::into_stream)
                .try_flatten_stream();

            futures_util::pin_mut!(stream);

            while let Some(res) = stream.try_next().await? {
                if let Either::Right(row) = res {
                    return Ok(Some(row));
                }
            }

            Ok(None)
        })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        _parameters: &[SqliteDataType],
    ) -> BoxFuture<'e, Result<Statement, Error>>
    where
        'c: 'e,
    {
        Box::pin(async move {
            let statement = self.worker.prepare(sql).await?;

            Ok(Statement {
                sql: sql.into(),
                ..statement
            })
        })
    }
}
