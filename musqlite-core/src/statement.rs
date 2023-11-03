#[macro_export]
macro_rules! impl_statement_query {
    ($A:ty) => {
        #[inline]
        pub fn query(&self) -> $crate::query::Query<'_, crate::sqlite::Sqlite, $A> {
            $crate::query::query_statement(self)
        }

        #[inline]
        pub fn query_with<'s, A>(
            &'s self,
            arguments: A,
        ) -> $crate::query::Query<'s, crate::sqlite::Sqlite, A>
        where
            A: $crate::arguments::IntoArguments<'s>,
        {
            $crate::query::query_statement_with(self, arguments)
        }

        #[inline]
        pub fn query_as<O>(
            &self,
        ) -> $crate::query_as::QueryAs<'_, crate::sqlite::Sqlite, O, Arguments<'_>>
        where
            O: for<'r> $crate::from_row::FromRow<
                'r,
                <crate::sqlite::Sqlite as $crate::database::Database>::Row,
            >,
        {
            $crate::query_as::query_statement_as(self)
        }

        #[inline]
        pub fn query_as_with<'s, O, A>(
            &'s self,
            arguments: A,
        ) -> $crate::query_as::QueryAs<'s, crate::sqlite::Sqlite, O, A>
        where
            O: for<'r> $crate::from_row::FromRow<
                'r,
                <crate::sqlite::Sqlite as $crate::database::Database>::Row,
            >,
            A: $crate::arguments::IntoArguments<'s>,
        {
            $crate::query_as::query_statement_as_with(self, arguments)
        }

        #[inline]
        pub fn query_scalar<O>(
            &self,
        ) -> $crate::query_scalar::QueryScalar<'_, crate::sqlite::Sqlite, O, Arguments<'_>>
        where
            (O,): for<'r> $crate::from_row::FromRow<
                'r,
                <crate::sqlite::Sqlite as $crate::database::Database>::Row,
            >,
        {
            $crate::query_scalar::query_statement_scalar(self)
        }

        #[inline]
        pub fn query_scalar_with<'s, O, A>(
            &'s self,
            arguments: A,
        ) -> $crate::query_scalar::QueryScalar<'s, crate::sqlite::Sqlite, O, A>
        where
            (O,): for<'r> $crate::from_row::FromRow<
                'r,
                <crate::sqlite::Sqlite as $crate::database::Database>::Row,
            >,
            A: $crate::arguments::IntoArguments<'s>,
        {
            $crate::query_scalar::query_statement_scalar_with(self, arguments)
        }
    };
}
