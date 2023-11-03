use either::Either;
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::TryStreamExt;

use crate::{
    database::Database,
    describe::Describe,
    error::Error,
    executor::{Execute, Executor},
    pool::Pool,
    sqlite, Statement,
};

impl<'p, DB: Database> Executor<'p> for &'_ Pool<DB>
where
    for<'c> &'c mut DB::Connection: Executor<'c, Database = DB>,
{
    type Database = DB;

    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<DB::QueryResult, DB::Row>, Error>>
    where
        E: Execute<'q, Self::Database>,
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

    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<DB::Row>, Error>>
    where
        E: Execute<'q, Self::Database>,
    {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.fetch_optional(query).await })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        parameters: &'e [sqlite::TypeInfo],
    ) -> BoxFuture<'e, Result<Statement<'q>, Error>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.prepare_with(sql, parameters).await })
    }

    #[doc(hidden)]
    fn describe<'e, 'q: 'e>(self, sql: &'q str) -> BoxFuture<'e, Result<Describe, Error>> {
        let pool = self.clone();

        Box::pin(async move { pool.acquire().await?.describe(sql).await })
    }
}

// Causes an overflow when evaluating `&mut DB::Connection: Executor`.
//
//
// impl<'c, DB: Database> crate::executor::Executor<'c> for &'c mut crate::pool::PoolConnection<DB>
// where
//     &'c mut DB::Connection: Executor<'c, Database = DB>,
// {
//     type Database = DB;
//
//
//
//     #[inline]
//     fn fetch_many<'e, 'q: 'e, E: 'q>(
//         self,
//         query: E,
//     ) -> futures_core::stream::BoxStream<
//         'e,
//         Result<
//             either::Either<<DB as crate::database::Database>::QueryResult, DB::Row>,
//             crate::error::Error,
//         >,
//     >
//     where
//         'c: 'e,
//         E: crate::executor::Execute<'q, DB>,
//     {
//         (**self).fetch_many(query)
//     }
//
//     #[inline]
//     fn fetch_optional<'e, 'q: 'e, E: 'q>(
//         self,
//         query: E,
//     ) -> futures_core::future::BoxFuture<'e, Result<Option<DB::Row>, crate::error::Error>>
//     where
//         'c: 'e,
//         E: crate::executor::Execute<'q, DB>,
//     {
//         (**self).fetch_optional(query)
//     }
//
//     #[inline]
//     fn prepare_with<'e, 'q: 'e>(
//         self,
//         sql: &'q str,
//         parameters: &'e [<DB as crate::database::Database>::TypeInfo],
//     ) -> futures_core::future::BoxFuture<
//         'e,
//         Result<<DB as crate::database::HasStatement<'q>>::Statement, crate::error::Error>,
//     >
//     where
//         'c: 'e,
//     {
//         (**self).prepare_with(sql, parameters)
//     }
//
//     #[doc(hidden)]
//     #[inline]
//     fn describe<'e, 'q: 'e>(
//         self,
//         sql: &'q str,
//     ) -> futures_core::future::BoxFuture<
//         'e,
//         Result<crate::describe::Describe<DB>, crate::error::Error>,
//     >
//     where
//         'c: 'e,
//     {
//         (**self).describe(sql)
//     }
// }
