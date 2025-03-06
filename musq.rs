pub mod musq {
    pub mod async_stream {
        pub struct TryAsyncStream<'a, T> {}

        impl<'a, T> TryAsyncStream<'a, T> {
            pub fn new<F, Fut>(f: F) -> Self
            where
                F: FnOnce(mpsc::Sender<Result<T, Error>>) -> Fut + Send,
                Fut: 'a + Future<Output = Result<(), Error>> + Send,
                T: 'a + Send, {
            }
        }

        impl<'a, T> Stream for TryAsyncStream<'a, T> {
            type Item = Result<T, Error>;
            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {}
        }
    }

    pub mod decode {
        //! Provides [`Decode`] for decoding values from the database.

        /// A type that can be decoded from the database.
        pub trait Decode<'r>: Sized {
            /// Decode a new value of this type using a raw value from the database.
            fn decode(value: &'r Value) -> Result<Self, DecodeError>;
        }
    }

    pub mod encode {
        //! Provides [`Encode`] for encoding values for the database.

        /// Encode a single value to be sent to the database.
        pub trait Encode {
            /// Writes the value of `self` into `buf` in the expected format for the database, consuming the value. Encoders are
            /// implemented for reference counted types where a shift in ownership is not wanted.
            fn encode(self) -> ArgumentValue
            where
                Self: Sized;
        }
    }

    pub mod pool {
        //! Provides the connection pool for asynchronous connections.
        //!
        //! Opening a database connection for each and every operation to the database can quickly
        //! become expensive. Furthermore, sharing a database connection between threads and functions
        //! can be difficult to express in Rust.
        //!
        //! A connection pool is a standard technique that can manage opening and re-using connections.
        //! Normally it also enforces a maximum number of connections as these are an expensive resource
        //! on the database server.

        pub mod maybe {
            pub enum MaybePoolConnection<'c> {
                Connection(&'c mut crate::Connection),
                PoolConnection(crate::pool::PoolConnection),
            }
        }

        /// A connection managed by a [`Pool`][crate::pool::Pool].
        ///
        /// Will be returned to the pool on-drop.
        pub struct PoolConnection {}

        impl PoolConnection {
            /// Close this connection, allowing the pool to open a replacement.
            ///
            /// Equivalent to calling [`.detach()`] then [`.close()`], but the connection permit is retained
            /// for the duration so that the pool may not exceed `max_connections`.
            ///
            /// [`.detach()`]: PoolConnection::detach
            /// [`.close()`]: Connection::close
            pub async fn close(self) -> Result<(), Error> {}

            /// Detach this connection from the pool, allowing it to open a replacement.
            ///
            /// Note that if your application uses a single shared pool, this
            /// effectively lets the application exceed the [`max_connections`] setting.
            ///
            /// If [`min_connections`] is nonzero, a task will be spawned to replace this connection.
            ///
            /// If you want the pool to treat this connection as permanently checked-out,
            /// use [`.leak()`][Self::leak] instead.
            ///
            /// [`max_connections`]: crate::pool::PoolOptions::max_connections
            /// [`min_connections`]: crate::pool::PoolOptions::min_connections
            pub fn detach(self) -> Connection {}

            /// Detach this connection from the pool, treating it as permanently checked-out.
            ///
            /// This effectively will reduce the maximum capacity of the pool by 1 every time it is used.
            ///
            /// If you don't want to impact the pool's capacity, use [`.detach()`][Self::detach] instead.
            pub fn leak(self) -> Connection {}
        }

        impl<'c> From<PoolConnection> for MaybePoolConnection<'c> {
            fn from(v: PoolConnection) -> Self {}
        }

        impl Debug for PoolConnection {
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {}
        }

        impl Deref for PoolConnection {
            type Target = Connection;
            fn deref(&self) -> &Self::Target {}
        }

        impl DerefMut for PoolConnection {
            fn deref_mut(&mut self) -> &mut Self::Target {}
        }

        impl AsRef<Connection> for PoolConnection {
            fn as_ref(&self) -> &Connection {}
        }

        impl AsMut<Connection> for PoolConnection {
            fn as_mut(&mut self) -> &mut Connection {}
        }

        /// Returns the connection to the [`Pool`][crate::pool::Pool] it was checked-out from.
        impl Drop for PoolConnection {
            fn drop(&mut self) {}
        }

        /// An asynchronous pool of database connections.
        ///
        /// Create a pool with [Pool::connect] or [Pool::connect_with] and then call [Pool::acquire] to get a connection from
        /// the pool; when the connection is dropped it will return to the pool so it can be reused.
        ///
        /// You can also pass `&Pool` directly anywhere an `Executor` is required; this will automatically checkout a connection
        /// for you.
        ///
        /// See [the module documentation](crate::pool) for examples.
        ///
        /// The pool has a maximum connection limit that it will not exceed; if `acquire()` is called when at this limit and all
        /// connections are checked out, the task will be made to wait until a connection becomes available.
        ///
        /// You can configure the connection limit, and other parameters, using [PoolOptions][crate::pool::PoolOptions].
        ///
        /// Calls to `acquire()` are fair, i.e. fulfilled on a first-come, first-serve basis.
        ///
        /// `Pool` is `Send`, `Sync` and `Clone`. It is intended to be created once at the start of your program and then shared
        /// with all tasks throughout the process' lifetime. How best to accomplish this depends on your program architecture.
        ///
        /// Cloning `Pool` is cheap as it is simply a reference-counted handle to the inner pool state. When the last remaining
        /// handle to the pool is dropped, the connections owned by the pool are immediately closed (also by dropping).
        /// `PoolConnection` returned by [Pool::acquire] and `Transaction` returned by [Pool::begin] both implicitly hold a
        /// reference to the pool for their lifetimes.
        ///
        /// We recommend calling [`.close().await`] to gracefully close the pool and its connections when you are done using it.
        /// This will also wake any tasks that are waiting on an `.acquire()` call, so for long-lived applications it's a good
        /// idea to call `.close()` during shutdown.
        ///
        /// If you're writing tests, consider using `#[test]` which handles the lifetime of the pool for you.
        ///
        /// [`.close().await`]: Pool::close
        pub struct Pool(_);

        impl Pool {}

        impl Pool {
            /// Retrieves a connection from the pool.
            ///
            /// The total time this method is allowed to execute is capped by
            /// [`PoolOptions::acquire_timeout`].
            /// If that timeout elapses, this will return [`Error::PoolClosed`].
            ///
            /// ### Note: Cancellation/Timeout May Drop Connections
            /// If `acquire` is cancelled or times out after it acquires a connection from the idle queue or
            /// opens a new one, it will drop that connection because we don't want to assume it
            /// is safe to return to the pool, and testing it to see if it's safe to release could introduce
            /// subtle bugs if not implemented correctly. To avoid that entirely, we've decided to not
            /// gracefully handle cancellation here.
            ///
            /// However, if your workload is sensitive to dropped connections such as using an in-memory
            /// SQLite database with a pool size of 1, you can pretty easily ensure that a cancelled
            /// `acquire()` call will never drop connections by tweaking your [`PoolOptions`]:
            ///
            /// * Set [`test_before_acquire(false)`][PoolOptions::test_before_acquire]
            /// * Never set [`before_acquire`][PoolOptions::before_acquire] or
            ///   [`after_connect`][PoolOptions::after_connect].
            ///
            /// This should eliminate any potential `.await` points between acquiring a connection and
            /// returning it.
            pub fn acquire(&self) -> impl Future<Output = Result<PoolConnection>> + 'static {}

            /// Attempts to retrieve a connection from the pool if there is one available.
            ///
            /// Returns `None` immediately if there are no idle connections available in the pool
            /// or there are tasks waiting for a connection which have yet to wake.
            pub fn try_acquire(&self) -> Option<PoolConnection> {}

            /// Retrieves a connection and immediately begins a new transaction.
            pub async fn begin(&self) -> Result<Transaction<'static>> {}

            /// Attempts to retrieve a connection and immediately begins a new transaction if successful.
            pub async fn try_begin(&self) -> Result<Option<Transaction<'static>>> {}

            /// Shut down the connection pool, immediately waking all tasks waiting for a connection.
            ///
            /// Upon calling this method, any currently waiting or subsequent calls to [`Pool::acquire`] and
            /// the like will immediately return [`Error::PoolClosed`] and no new connections will be opened.
            /// Checked-out connections are unaffected, but will be gracefully closed on-drop
            /// rather than being returned to the pool.
            ///
            /// Returns a `Future` which can be `.await`ed to ensure all connections are
            /// gracefully closed. It will first close any idle connections currently waiting in the pool,
            /// then wait for all checked-out connections to be returned or closed.
            ///
            /// Waiting for connections to be gracefully closed is optional, but will allow the database
            /// server to clean up the resources sooner rather than later. This is especially important
            /// for tests that create a new pool every time, otherwise you may see errors about connection
            /// limits being exhausted even when running tests in a single thread.
            ///
            /// If the returned `Future` is not run to completion, any remaining connections will be dropped
            /// when the last handle for the given pool instance is dropped, which could happen in a task
            /// spawned by `Pool` internally and so may be unpredictable otherwise.
            ///
            /// `.close()` may be safely called and `.await`ed on multiple handles concurrently.
            pub fn close(&self) -> impl Future<Output = ()> + '_ {}

            /// Returns `true` if [`.close()`][Pool::close] has been called on the pool, `false` otherwise.
            pub fn is_closed(&self) -> bool {}

            /// Get a future that resolves when [`Pool::close()`] is called.
            ///
            /// If the pool is already closed, the future resolves immediately.
            ///
            /// This can be used to cancel long-running operations that hold onto a [`PoolConnection`]
            /// so they don't prevent the pool from closing (which would otherwise wait until all
            /// connections are returned).
            pub fn close_event(&self) -> CloseEvent {}

            /// Returns the number of connections currently active. This includes idle connections.
            pub fn size(&self) -> u32 {}

            /// Returns the number of connections active and idle (not in use).
            pub fn num_idle(&self) -> usize {}
        }

        impl<'p> Executor<'p> for &crate::pool::Pool
        where
            for<'c> &'c mut crate::Connection: Executor<'c>,
        {
            fn fetch_many<'e, 'q: 'e, E>(
                self,
                query: E,
            ) -> BoxStream<'e, Result<Either<QueryResult, Row>>>
            where
                E: Execute + 'q, {
            }

            fn fetch_optional<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Option<Row>>>
            where
                E: Execute + 'q, {
            }

            fn prepare_with<'e, 'q: 'e>(
                self,
                sql: &'q str,
                parameters: &'e [sqlite::SqliteDataType],
            ) -> BoxFuture<'e, Result<Statement>> {
            }
        }

        /// Returns a new [Pool] tied to the same shared connection pool.
        impl Clone for Pool {
            fn clone(&self) -> Self {}
        }

        impl Debug for Pool {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {}
        }

        /// A future that resolves when the pool is closed.
        ///
        /// See [`Pool::close_event()`] for details.
        pub struct CloseEvent {}

        impl CloseEvent {
            /// Execute the given future until it returns or the pool is closed.
            ///
            /// Cancels the future and returns `Err(PoolClosed)` if/when the pool is closed.
            /// If the pool was already closed, the future is never run.
            pub async fn do_until<Fut: Future>(&mut self, fut: Fut) -> Result<Fut::Output> {}
        }

        impl Future for CloseEvent {
            type Output = ();
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {}
        }

        impl FusedFuture for CloseEvent {
            fn is_terminated(&self) -> bool {}
        }
    }

    pub mod query {
        /// Raw SQL query with bind parameters. Returned by [`query`][crate::query::query].
        pub struct Query<A> {}

        impl<'q> Query<crate::Arguments> {
            /// Bind a value for use with this SQL query.
            ///
            /// If the number of times this is called does not match the number of bind parameters that appear in the query then
            /// an error will be returned when this query is executed.
            pub fn bind<T: 'q + Send + Encode>(self, value: T) -> Self {}
        }

        impl<'q, A> Query<A>
        where
            A: 'q + IntoArguments + Send,
        {
            /// Map each row in the result to another type.
            ///
            /// See [`try_map`](Query::try_map) for a fallible version of this method.
            ///
            /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
            /// a [`FromRow`](super::from_row::FromRow) implementation.
            pub fn map<F, O>(self, f: F) -> Map<impl FnMut(Row) -> Result<O, Error> + Send, A>
            where
                F: FnMut(Row) -> O + Send,
                O: Unpin, {
            }

            /// Map each row in the result to another type.
            ///
            /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
            /// a [`FromRow`](super::from_row::FromRow) implementation.
            pub fn try_map<F, O>(self, f: F) -> Map<F, A>
            where
                F: FnMut(Row) -> Result<O, Error> + Send,
                O: Unpin, {
            }

            /// Execute the query and return the total number of rows affected.
            pub async fn execute<'e, 'c: 'e, E>(self, executor: E) -> Result<QueryResult, Error>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute multiple queries and return the rows affected from each query, in a stream.
            pub async fn execute_many<'e, 'c: 'e, E>(
                self,
                executor: E,
            ) -> BoxStream<'e, Result<QueryResult, Error>>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute the query and return the generated results as a stream.
            pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<Row, Error>>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute multiple queries and return the generated results as a stream
            /// from each query, in a stream.
            pub fn fetch_many<'e, 'c: 'e, E>(
                self,
                executor: E,
            ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute the query and return all the generated results, collected into a [`Vec`].
            pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<Row>, Error>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute the query and returns exactly one row.
            pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<Row, Error>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }

            /// Execute the query and returns at most one row.
            pub async fn fetch_optional<'e, 'c: 'e, E>(
                self,
                executor: E,
            ) -> Result<Option<Row>, Error>
            where
                A: 'e,
                E: Executor<'c>,
                'q: 'e, {
            }
        }

        impl<A> Execute for Query<A>
        where
            A: Send + IntoArguments,
        {
            fn sql(&self) -> &str {}

            fn statement(&self) -> Option<&Statement> {}

            fn take_arguments(&mut self) -> Option<Arguments> {}
        }

        /// SQL query that will map its results to owned Rust types.
        ///
        /// Returned by [`Query::try_map`], `query!()`, etc. Has most of the same methods as [`Query`] but
        /// the return types are changed to reflect the mapping. However, there is no equivalent of
        /// [`Query::execute`] as it doesn't make sense to map the result type and then ignore it.
        ///
        /// [`Query::bind`] is also omitted; stylistically we recommend placing your `.bind()` calls
        /// before `.try_map()`. This is also to prevent adding superfluous binds to the result of
        /// `query!()` et al.
        pub struct Map<F, A> {}

        impl<'q, F, O, A> Map<F, A>
        where
            F: FnMut(crate::Row) -> Result<O, crate::error::Error> + Send,
            O: Send + Unpin,
            A: 'q + Send + IntoArguments,
        {
            /// Map each row in the result to another type.
            ///
            /// See [`try_map`](Map::try_map) for a fallible version of this method.
            ///
            /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
            /// a [`FromRow`](super::from_row::FromRow) implementation.
            pub fn map<G, P>(self, g: G) -> Map<impl FnMut(Row) -> Result<P, Error> + Send, A>
            where
                G: FnMut(O) -> P + Send,
                P: Unpin, {
            }

            /// Map each row in the result to another type.
            ///
            /// The [`query_as`](super::query_as::query_as) method will construct a mapped query using
            /// a [`FromRow`](super::from_row::FromRow) implementation.
            pub fn try_map<G, P>(self, g: G) -> Map<impl FnMut(Row) -> Result<P, Error> + Send, A>
            where
                G: FnMut(O) -> Result<P, Error> + Send,
                P: Unpin, {
            }

            /// Execute the query and return the generated results as a stream.
            pub fn fetch<'e, 'c: 'e, E>(self, executor: E) -> BoxStream<'e, Result<O, Error>>
            where
                E: 'e + Executor<'c>,
                F: 'e,
                O: 'e,
                'q: 'e, {
            }

            /// Execute multiple queries and return the generated results as a stream
            /// from each query, in a stream.
            pub fn fetch_many<'e, 'c: 'e, E>(
                self,
                executor: E,
            ) -> BoxStream<'e, Result<Either<QueryResult, O>, Error>>
            where
                E: 'e + Executor<'c>,
                F: 'e,
                O: 'e,
                'q: 'e, {
            }

            /// Execute the query and return all the generated results, collected into a [`Vec`].
            pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, Error>
            where
                E: 'e + Executor<'c>,
                F: 'e,
                O: 'e,
                'q: 'e, {
            }

            /// Execute the query and returns exactly one row.
            pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, Error>
            where
                E: 'e + Executor<'c>,
                F: 'e,
                O: 'e,
                'q: 'e, {
            }

            /// Execute the query and returns at most one row.
            pub async fn fetch_optional<'e, 'c: 'e, E>(
                self,
                executor: E,
            ) -> Result<Option<O>, Error>
            where
                E: 'e + Executor<'c>,
                F: 'e,
                O: 'e,
                'q: 'e, {
            }
        }

        impl<F: Send, A> Execute for Map<F, A>
        where
            A: IntoArguments + Send,
        {
            fn sql(&self) -> &str {}

            fn statement(&self) -> Option<&Statement> {}

            fn take_arguments(&mut self) -> Option<Arguments> {}
        }

        pub fn query_statement(statement: &crate::Statement) -> Query<crate::Arguments> {}

        pub fn query_statement_with<A>(statement: &crate::Statement, arguments: A) -> Query<A>
        where
            A: IntoArguments, {
        }

        /// Make a SQL query.
        pub fn query(sql: &str) -> Query<crate::Arguments> {}

        /// Make a SQL query, with the given arguments.
        pub fn query_with<A>(sql: &str, arguments: A) -> Query<A>
        where
            A: IntoArguments, {
        }
    }

    pub mod types {
        //! Conversions between Rust and **SQLite** types.
        //!
        //! # Types
        //!
        //! | Rust type                             | SQLite type(s)      |
        //! |---------------------------------------|---------------------|
        //! | `bool`                                | BOOLEAN             |
        //! | `i8`                                  | INTEGER             |
        //! | `i16`                                 | INTEGER             |
        //! | `i32`                                 | INTEGER             |
        //! | `i64`                                 | BIGINT, INT8        |
        //! | `u8`                                  | INTEGER             |
        //! | `u16`                                 | INTEGER             |
        //! | `u32`                                 | INTEGER             |
        //! | `f32`                                 | REAL                |
        //! | `f64`                                 | REAL                |
        //! | `&str`, [`String`]                    | TEXT                |
        //! | `&[u8]`, `Vec<u8>`                    | BLOB                |
        //! | `time::PrimitiveDateTime`             | DATETIME            |
        //! | `time::OffsetDateTime`                | DATETIME            |
        //! | `time::Date`                          | DATE                |
        //! | `time::Time`                          | TIME                |
        //! | `bstr::BString`                       | BLOB                |
        //!
        //! #### Note: Unsigned Integers
        //!
        //! The unsigned integer types `u8`, `u16` and `u32` are implemented by zero-extending to the next-larger signed type.
        //! So `u8` becomes `i16`, `u16` becomes `i32`, and `u32` becomes `i64` while still retaining their semantic values.
        //!
        //! SQLite stores integers in a variable-width encoding and always handles them in memory as 64-bit signed values, so no
        //! space is wasted by this implicit widening.
        //!
        //! There is no corresponding larger type for `u64` in SQLite (it would require a `i128`), and so it is not supported.
        //! Bit-casting it to `i64` or storing it as `REAL`, `BLOB` or `TEXT` would change the semantics of the value in SQL and
        //! so violates the principle of least surprise.
        //!
        //! # Nullable
        //!
        //! `Option<T>` is supported where `T` implements `Encode` or `Decode`. An `Option<T>` represents a potentially `NULL`
        //! value from SQLite.

        pub mod bstr {
            pub use bstr::BStr;
            pub use bstr::BString;
            pub use bstr::ByteSlice;
        }

        pub mod time {
            pub use time::Date;
            pub use time::OffsetDateTime;
            pub use time::PrimitiveDateTime;
            pub use time::Time;
            pub use time::UtcOffset;
        }

        #[macro_export]
        macro_rules! compatible {
    ($x:expr, $($y:path)|+) => { ... };
}
    }

    pub use either::Either;
    pub use indexmap::IndexMap;
    pub struct Column {}

    impl Column {
        pub fn ordinal(&self) -> usize {}

        pub fn name(&self) -> &str {}

        pub fn type_info(&self) -> &SqliteDataType {}
    }

    impl Debug for Column {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl Clone for Column {
        fn clone(&self) -> Column {}
    }

    impl Serialize for Column {
        fn serialize<__S>(
            &self,
            __serializer: __S,
        ) -> _serde::__private::Result<__S::Ok, __S::Error>
        where
            __S: _serde::Serializer, {
        }
    }

    impl<'de> Deserialize<'de> for Column {
        fn deserialize<__D>(__deserializer: __D) -> _serde::__private::Result<Self, __D::Error>
        where
            __D: _serde::Deserializer<'de>, {
        }
    }

    pub enum DecodeError {
        DataType(crate::SqliteDataType),
        Conversion(String),
    }

    /// Represents all the ways a method can fail within SQLx.
    pub enum Error {
        /// Error returned from the database.
        Sqlite(sqlite::error::SqliteError),
        /// Error communicating with the database backend.
        Io(io::Error),
        /// Unexpected or invalid data encountered while communicating with the database.
        ///
        /// This should indicate there is a programming error in a SQLx driver or there
        /// is something corrupted with the connection to the database itself.
        Protocol(String),
        /// No rows returned by a query that expected to return at least one row.
        RowNotFound,
        /// Type in query doesn't exist. Likely due to typo or missing user type.
        TypeNotFound { type_name: String },
        /// Column index was out of bounds.
        ColumnIndexOutOfBounds { index: usize, len: usize },
        /// No column found for the given name.
        ColumnNotFound(String),
        /// Error occurred while decoding a value from a specific column.
        ColumnDecode { index: String, source: DecodeError },
        /// Error occurred while decoding a value.
        Decode(DecodeError),
        /// A [`Pool::acquire`] timed out due to connections not becoming available or
        /// because another task encountered too many errors while trying to open a new connection.
        ///
        /// [`Pool::acquire`]: crate::pool::Pool::acquire
        PoolTimedOut,
        /// [`Pool::close`] was called while we were waiting in [`Pool::acquire`].
        ///
        /// [`Pool::acquire`]: crate::pool::Pool::acquire
        /// [`Pool::close`]: crate::pool::Pool::close
        PoolClosed,
        /// A background worker has crashed.
        WorkerCrashed,
    }

    /// A specialized `Result` type for SQLx.
    pub type Result<T, E = Error> = std::result::Result<T, E>;

    /// A type that may be executed against a database connection.
    ///
    /// Implemented for the following:
    ///
    ///  * [`&str`](std::str)
    ///  * [`Query`](super::query::Query)
    pub trait Execute: Send + Sized {
        /// Gets the SQL that will be executed.
        fn sql(&self) -> &str;

        /// Gets the previously cached statement, if available.
        fn statement(&self) -> Option<&Statement>;

        /// Returns the arguments to be bound against the query string.
        ///
        /// Returning `None` for `Arguments` indicates to use a "simple" query protocol and to not
        /// prepare the query. Returning `Some(Default::default())` is an empty arguments object that
        /// will be prepared (and cached) before execution.
        fn take_arguments(&mut self) -> Option<Arguments>;
    }

    /// A type that contains or can provide a database connection to use for executing queries against
    /// the database.
    ///
    /// No guarantees are provided that successive queries run on the same physical database
    /// connection.
    ///
    /// A [`Connection`](crate::connection::Connection) is an `Executor` that guarantees that
    /// successive queries are ran on the same physical database connection.
    ///
    /// Implemented for the following:
    ///
    ///  * [`&Pool`](super::pool::Pool)
    ///  * [`&mut PoolConnection`](super::pool::PoolConnection)
    ///  * [`&mut Connection`](super::connection::Connection)
    pub trait Executor<'c>: Send + Debug + Sized {
        /// Execute the query and return the total number of rows affected.
        fn execute<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<QueryResult, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        /// Execute multiple queries and return the rows affected from each query, in a stream.
        fn execute_many<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxStream<'e, Result<QueryResult, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        /// Execute the query and return the generated results as a stream.
        fn fetch<'e, 'q: 'e, E>(self, query: E) -> BoxStream<'e, Result<Row, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        /// Execute multiple queries and return the generated results as a stream
        /// from each query, in a stream.
        fn fetch_many<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
        where
            E: Execute + 'q,
            'c: 'e;

        /// Execute the query and return all the generated results, collected into a [`Vec`].
        fn fetch_all<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Vec<Row>, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        /// Execute the query and returns exactly one row.
        fn fetch_one<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Row, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        /// Execute the query and returns at most one row.
        fn fetch_optional<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxFuture<'e, Result<Option<Row>, Error>>
        where
            E: Execute + 'q,
            'c: 'e;

        /// Prepare the SQL query to inspect the type information of its parameters
        /// and results.
        ///
        /// Be advised that when using the `query`, `query_as`, or `query_scalar` functions, the query
        /// is transparently prepared and executed.
        ///
        /// This explicit API is provided to allow access to the statement metadata available after
        /// it prepared but before the first row is returned.
        fn prepare<'e, 'q: 'e>(self, query: &'q str) -> BoxFuture<'e, Result<Statement, Error>>
        where
            'c: 'e, {
        }

        /// Prepare the SQL query, with parameter type information, to inspect the
        /// type information about its parameters and results.
        ///
        /// Only some database drivers (PostgreSQL, MSSQL) can take advantage of
        /// this extra information to influence parameter type inference.
        fn prepare_with<'e, 'q: 'e>(
            self,
            sql: &'q str,
            parameters: &'e [sqlite::SqliteDataType],
        ) -> BoxFuture<'e, Result<Statement, Error>>
        where
            'c: 'e;
    }

    /// A record that can be built from a row returned by the database.
    ///
    /// To use [`query_as`](crate::query_as) the output type must implement `FromRow`.
    ///
    /// ## Derivable
    ///
    /// This trait can be derived for structs. The generated implementation will make a sequence of calls to
    /// [`Row::get_value`] using the name from each struct field.
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    /// }
    /// ```
    ///
    /// ### Field attributes
    ///
    /// Several attributes can be specified to customize how each column in a row is read:
    ///
    /// #### `rename`
    ///
    /// When the name of a field in Rust does not match the name of its corresponding column, you can use the `rename`
    /// attribute to specify the name that the field has in the row. For example:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    ///     #[musq(rename = "description")]
    ///     about_me: String
    /// }
    /// ```
    ///
    /// Given a query such as:
    ///
    /// ```sql
    /// SELECT id, name, description FROM users;
    /// ```
    ///
    /// will read the content of the column `description` into the field `about_me`.
    ///
    /// #### `rename_all`
    /// By default, field names are expected verbatim (with the exception of the raw identifier prefix `r#`, if present).
    /// Placed at the struct level, this attribute changes how the field name is mapped to its SQL column name:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// #[musq(rename_all = "camelCase")]
    /// struct UserPost {
    ///     id: i32,
    ///     // remapped to "userId"
    ///     user_id: i32,
    ///     contents: String
    /// }
    /// ```
    ///
    /// The supported values are `snake_case` (available if you have non-snake-case field names for some reason),
    /// `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE` and `kebab-case`. The styling of each
    /// option is intended to be an example of its behavior.
    ///
    /// #### `default`
    ///
    /// When your struct contains a field that is not present in your query, if the field type has an implementation for
    /// [`Default`], you can use the `default` attribute to assign the default value to said field. For example:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    ///     #[musq(default)]
    ///     location: Option<String>
    /// }
    /// ```
    ///
    /// Given a query such as:
    ///
    /// ```sql
    /// SELECT id, name FROM users;
    /// ```
    ///
    /// will set the value of the field `location` to the default value of `Option<String>`, which is `None`.
    ///
    /// ### `flatten`
    ///
    /// If you want to handle a field that implements [`FromRow`], you can use the `flatten` attribute to specify that you
    /// want it to use [`FromRow`] for parsing rather than the usual method. For example:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct Address {
    ///     country: String,
    ///     city: String,
    ///     road: String,
    /// }
    ///
    /// #[derive(FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    ///     #[musq(flatten)]
    ///     address: Address,
    /// }
    /// ```
    /// Given a query such as:
    ///
    /// ```sql
    /// SELECT id, name, country, city, road FROM users;
    /// ```
    ///
    /// This field is compatible with the `default` attribute.
    ///
    /// #### `skip`
    ///
    /// This is a variant of the `default` attribute which instead always takes the value from the `Default` implementation
    /// for this field type ignoring any results in your query. This can be useful, if some field does not satifisfy the
    /// trait bounds (i.e. `decode::Decode`, `type::Type`), in particular in case of nested structures. For example:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct Address {
    ///     user_name: String,
    ///     street: String,
    ///     city: String,
    /// }
    ///
    /// #[derive(FromRow)]
    /// struct User {
    ///     name: String,
    ///     #[musq(skip)]
    ///     addresses: Vec<Address>,
    /// }
    /// ```
    ///
    /// Then when querying into `User`, only `name` needs to be set:
    ///
    /// ```rust,ignore
    /// let user: User = query_as("SELECT name FROM users")
    ///    .fetch_one(&mut some_connection)
    ///    .await?;
    ///
    /// `Default` for `Vec<Address>` is an empty vector.
    /// assert!(user.addresses.is_empty());
    /// ```
    ///
    /// ## Manual implementation
    ///
    /// You can also implement the [`FromRow`] trait by hand. This can be useful if you have a struct with a field that
    /// needs manual decoding:
    ///
    ///
    /// ```rust,ignore
    /// use {FromRow, Row};
    /// struct MyCustomType {
    ///     custom: String,
    /// }
    ///
    /// struct Foo {
    ///     bar: MyCustomType,
    /// }
    ///
    /// impl FromRow<'_> for Foo {
    ///     fn from_row(row: &Row) -> Result<Self> {
    ///         Ok(Self {
    ///             bar: MyCustomType {
    ///                 custom: row.get_value("custom")?
    ///             }
    ///         })
    ///     }
    /// }
    /// ```
    ///
    /// #### `try_from`
    ///
    /// When your struct contains a field whose type is not matched with the database type, if the field type has an
    /// implementation [`TryFrom`] for the database type, you can use the `try_from` attribute to convert the database type
    /// to the field type. For example:
    ///
    /// ```rust,ignore
    /// #[derive(FromRow)]
    /// struct User {
    ///     id: i32,
    ///     name: String,
    ///     #[musq(try_from = "i64")]
    ///     bigIntInMySql: u64
    /// }
    /// ```
    pub trait FromRow<'r>: Sized {
        fn from_row(prefix: &str, row: &'r Row) -> Result<Self, Error>;
    }

    pub enum AutoVacuum {
        None,
        Full,
        Incremental,
    }

    /// Refer to [SQLite documentation] for the meaning of the database journaling mode.
    ///
    /// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_journal_mode
    pub enum JournalMode {
        Delete,
        Truncate,
        Persist,
        Memory,
        Wal,
        Off,
    }

    /// Refer to [SQLite documentation] for the meaning of the connection locking mode.
    ///
    /// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_locking_mode
    pub enum LockingMode {
        Normal,
        Exclusive,
    }

    /// Create a Musq connection
    pub struct Musq {}

    impl Musq {
        /// Construct `Self` with default options.
        ///
        /// See the source of this method for the current defaults.
        pub fn new() -> Self {}

        /// Set the filename as in-memory. Use the `open_in_memory` method instead, unless you have a very particular use
        /// case.
        pub fn in_memory(self, val: bool) -> Self {}

        /// Sets the name of the database file.
        pub fn filename(self, filename: impl AsRef<Path>) -> Self {}

        /// Set the enforcement of [foreign key constraints](https://www.sqlite.org/pragma.html#pragma_foreign_keys).
        ///
        /// SQLx chooses to enable this by default so that foreign keys function as expected,
        /// compared to other database flavors.
        pub fn foreign_keys(self, on: bool) -> Self {}

        /// Set the [`SQLITE_OPEN_SHAREDCACHE` flag](https://sqlite.org/sharedcache.html).
        ///
        /// By default, this is disabled.
        pub fn shared_cache(self, on: bool) -> Self {}

        /// Sets the [journal mode](https://www.sqlite.org/pragma.html#pragma_journal_mode) for the database connection.
        ///
        /// Journal modes are ephemeral per connection, with the exception of the
        /// [Write-Ahead Log (WAL) mode](https://www.sqlite.org/wal.html).
        ///
        /// A database created in WAL mode retains the setting and will apply it to all connections
        /// opened against it that don't set a `journal_mode`.
        ///
        /// Opening a connection to a database created in WAL mode with a different `journal_mode` will
        /// erase the setting on the database, requiring an exclusive lock to do so.
        /// You may get a `database is locked` (corresponding to `SQLITE_BUSY`) error if another
        /// connection is accessing the database file at the same time.
        ///
        /// SQLx does not set a journal mode by default, to avoid unintentionally changing a database
        /// into or out of WAL mode.
        ///
        /// The default journal mode for non-WAL databases is `DELETE`, or `MEMORY` for in-memory
        /// databases.
        ///
        /// For consistency, any commands in `sqlx-cli` which create a SQLite database will create it
        /// in WAL mode.
        pub fn journal_mode(self, mode: JournalMode) -> Self {}

        /// Sets the [locking mode](https://www.sqlite.org/pragma.html#pragma_locking_mode) for the database connection.
        ///
        /// The default locking mode is NORMAL.
        pub fn locking_mode(self, mode: LockingMode) -> Self {}

        /// Sets the [access mode](https://www.sqlite.org/c3ref/open.html) to open the database
        /// for read-only access.
        pub fn read_only(self, read_only: bool) -> Self {}

        /// Sets the [access mode](https://www.sqlite.org/c3ref/open.html) to create the database file
        /// if the file does not exist.
        ///
        /// By default, a new file **will not be created** if one is not found.
        pub fn create_if_missing(self, create: bool) -> Self {}

        /// Sets a timeout value to wait when the database is locked, before
        /// returning a busy timeout error.
        ///
        /// The default busy timeout is 5 seconds.
        pub fn busy_timeout(self, timeout: Duration) -> Self {}

        /// Sets the [synchronous](https://www.sqlite.org/pragma.html#pragma_synchronous) setting for the database connection.
        ///
        /// The default synchronous settings is FULL. However, if durability is not a concern,
        /// then NORMAL is normally all one needs in WAL mode.
        pub fn synchronous(self, synchronous: Synchronous) -> Self {}

        /// Sets the [auto_vacuum](https://www.sqlite.org/pragma.html#pragma_auto_vacuum) setting for the database connection.
        ///
        /// The default auto_vacuum setting is NONE.
        ///
        /// For existing databases, a change to this value does not take effect unless a
        /// [`VACUUM` command](https://www.sqlite.org/lang_vacuum.html) is executed.
        pub fn auto_vacuum(self, auto_vacuum: AutoVacuum) -> Self {}

        /// Sets the [page_size](https://www.sqlite.org/pragma.html#pragma_page_size) setting for the database connection.
        ///
        /// The default page_size setting is 4096.
        ///
        /// For existing databases, a change to this value does not take effect unless a
        /// [`VACUUM` command](https://www.sqlite.org/lang_vacuum.html) is executed.
        /// However, it cannot be changed in WAL mode.
        pub fn page_size(self, page_size: u32) -> Self {}

        /// Sets custom initial pragma for the database connection.
        pub fn pragma(self, key: &str, value: &str) -> Self {}

        /// Set to `true` to signal to SQLite that the database file is on read-only media.
        ///
        /// If enabled, SQLite assumes the database file _cannot_ be modified, even by higher
        /// privileged processes, and so disables locking and change detection. This is intended
        /// to improve performance but can produce incorrect query results or errors if the file
        /// _does_ change.
        ///
        /// Note that this is different from the `SQLITE_OPEN_READONLY` flag set by
        /// [`.read_only()`][Self::read_only], though the documentation suggests that this
        /// does _imply_ `SQLITE_OPEN_READONLY`.
        ///
        /// See [`sqlite3_open`](https://www.sqlite.org/capi3ref.html#sqlite3_open) (subheading
        /// "URI Filenames") for details.
        pub fn immutable(self, immutable: bool) -> Self {}

        /// Sets the [threading mode](https://www.sqlite.org/threadsafe.html) for the database connection.
        ///
        /// The default setting is `false` corresponding to using `OPEN_NOMUTEX`.
        /// If set to `true` then `OPEN_FULLMUTEX`.
        ///
        /// See [open](https://www.sqlite.org/c3ref/open.html) for more details.
        ///
        /// ### Note
        /// Setting this to `true` may help if you are getting access violation errors or segmentation
        /// faults, but will also incur a significant performance penalty. You should leave this
        /// set to `false` if at all possible.
        ///
        /// If you do end up needing to set this to `true` for some reason, please
        /// [open an issue](https://github.com/launchbadge/sqlx/issues/new/choose) as this may indicate
        /// a concurrency bug in SQLx. Please provide clear instructions for reproducing the issue,
        /// including a sample database schema if applicable.
        pub fn serialized(self, serialized: bool) -> Self {}

        /// Provide a callback to generate the name of the background worker thread.
        ///
        /// The value passed to the callback is an auto-incremented integer for use as the thread ID.
        pub fn thread_name(
            self,
            generator: impl Fn(u64) -> String + Send + Sync + 'static,
        ) -> Self {
        }

        /// Set the maximum number of commands to buffer for the worker thread before backpressure is
        /// applied.
        ///
        /// Given that most commands sent to the worker thread involve waiting for a result,
        /// the command channel is unlikely to fill up unless a lot queries are executed in a short
        /// period but cancelled before their full resultsets are returned.
        pub fn command_buffer_size(self, size: usize) -> Self {}

        /// Set the maximum number of rows to buffer back to the calling task when a query is executed.
        ///
        /// If the calling task cannot keep up, backpressure will be applied to the worker thread
        /// in order to limit CPU and memory usage.
        pub fn row_buffer_size(self, size: usize) -> Self {}

        /// Sets the [`vfs`](https://www.sqlite.org/vfs.html) parameter of the database connection.
        ///
        /// The default value is empty, and sqlite will use the default VFS object depending on the
        /// operating system.
        pub fn vfs(self, vfs_name: &str) -> Self {}

        /// Execute `PRAGMA optimize;` on the SQLite connection before closing.
        ///
        /// The SQLite manual recommends using this for long-lived databases.
        ///
        /// This will collect and store statistics about the layout of data in your tables to help the query planner make
        /// better decisions. Over the connection's lifetime, the query planner will make notes about which tables could use
        /// up-to-date statistics so this command doesn't have to scan the whole database every time. Thus, the best time to
        /// execute this is on connection close.
        ///
        /// `analysis_limit` sets a soft limit on the maximum number of rows to scan per index. It is equivalent to setting
        /// [`Self::analysis_limit`] but only takes effect for the `PRAGMA optimize;` call and does not affect the behavior
        /// of any `ANALYZE` statements made during the connection's lifetime.
        ///
        /// If not `None`, the `analysis_limit` here overrides the global `analysis_limit` setting, but only for the `PRAGMA
        /// optimize;` call.
        ///
        /// Not enabled by default.
        ///
        /// See [the SQLite manual](https://www.sqlite.org/lang_analyze.html#automatically_running_analyze) for details.
        pub fn optimize_on_close(
            self,
            enabled: bool,
            analysis_limit: impl Into<Option<u32>>,
        ) -> Self {
        }

        /// Set a soft limit on the number of rows that `ANALYZE` touches per index.
        ///
        /// This also affects `PRAGMA optimize` which is set by [Self::optimize_on_close].
        ///
        /// The value recommended by SQLite is `400`. There is no default.
        ///
        /// See [the SQLite manual](https://www.sqlite.org/lang_analyze.html#approx) for details.
        pub fn analysis_limit(self, limit: Option<u32>) -> Self {}

        pub fn log_statements(self, level: LevelFilter) -> Self {}

        pub fn log_slow_statements(self, level: LevelFilter, duration: Duration) -> Self {}

        /// Set the maximum number of connections that this pool should maintain.
        ///
        /// Be mindful of the connection limits for your database as well as other applications
        /// which may want to connect to the same database (or even multiple instances of the same
        /// application in high-availability deployments).
        pub fn max_connections(self, max: u32) -> Self {}

        /// Set the maximum amount of time to spend waiting for a connection in [`Pool::acquire()`].
        ///
        /// Caps the total amount of time `Pool::acquire()` can spend waiting across multiple phases:
        ///
        /// * First, it may need to wait for a permit from the semaphore, which grants it the privilege
        ///   of opening a connection or popping one from the idle queue.
        /// * If an existing idle connection is acquired, by default it will be checked for liveness
        ///   and integrity before being returned, which may require executing a command on the
        ///   connection. This can be disabled with [`test_before_acquire(false)`][Self::test_before_acquire].
        ///     * If [`before_acquire`][Self::before_acquire] is set, that will also be executed.
        /// * If a new connection needs to be opened, that will obviously require I/O, handshaking,
        ///   and initialization commands.
        ///     * If [`after_connect`][Self::after_connect] is set, that will also be executed.
        pub fn acquire_timeout(self, timeout: Duration) -> Self {}

        /// Open a file
        pub async fn open(self, filename: impl AsRef<Path>) -> Result<pool::Pool> {}

        /// Open an in-memory database
        pub async fn open_in_memory(self) -> Result<pool::Pool> {}
    }

    impl Clone for Musq {
        fn clone(&self) -> Musq {}
    }

    impl Debug for Musq {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl Default for Musq {
        fn default() -> Self {}
    }

    /// Refer to [SQLite documentation] for the meaning of various synchronous settings.
    ///
    /// [SQLite documentation]: https://www.sqlite.org/pragma.html#pragma_synchronous
    pub enum Synchronous {
        Off,
        Normal,
        Full,
        Extra,
    }

    /// An asynchronous pool of database connections.
    ///
    /// Create a pool with [Pool::connect] or [Pool::connect_with] and then call [Pool::acquire] to get a connection from
    /// the pool; when the connection is dropped it will return to the pool so it can be reused.
    ///
    /// You can also pass `&Pool` directly anywhere an `Executor` is required; this will automatically checkout a connection
    /// for you.
    ///
    /// See [the module documentation](crate::pool) for examples.
    ///
    /// The pool has a maximum connection limit that it will not exceed; if `acquire()` is called when at this limit and all
    /// connections are checked out, the task will be made to wait until a connection becomes available.
    ///
    /// You can configure the connection limit, and other parameters, using [PoolOptions][crate::pool::PoolOptions].
    ///
    /// Calls to `acquire()` are fair, i.e. fulfilled on a first-come, first-serve basis.
    ///
    /// `Pool` is `Send`, `Sync` and `Clone`. It is intended to be created once at the start of your program and then shared
    /// with all tasks throughout the process' lifetime. How best to accomplish this depends on your program architecture.
    ///
    /// Cloning `Pool` is cheap as it is simply a reference-counted handle to the inner pool state. When the last remaining
    /// handle to the pool is dropped, the connections owned by the pool are immediately closed (also by dropping).
    /// `PoolConnection` returned by [Pool::acquire] and `Transaction` returned by [Pool::begin] both implicitly hold a
    /// reference to the pool for their lifetimes.
    ///
    /// We recommend calling [`.close().await`] to gracefully close the pool and its connections when you are done using it.
    /// This will also wake any tasks that are waiting on an `.acquire()` call, so for long-lived applications it's a good
    /// idea to call `.close()` during shutdown.
    ///
    /// If you're writing tests, consider using `#[test]` which handles the lifetime of the pool for you.
    ///
    /// [`.close().await`]: Pool::close
    pub struct Pool(_);

    impl Pool {}

    impl Pool {
        /// Retrieves a connection from the pool.
        ///
        /// The total time this method is allowed to execute is capped by
        /// [`PoolOptions::acquire_timeout`].
        /// If that timeout elapses, this will return [`Error::PoolClosed`].
        ///
        /// ### Note: Cancellation/Timeout May Drop Connections
        /// If `acquire` is cancelled or times out after it acquires a connection from the idle queue or
        /// opens a new one, it will drop that connection because we don't want to assume it
        /// is safe to return to the pool, and testing it to see if it's safe to release could introduce
        /// subtle bugs if not implemented correctly. To avoid that entirely, we've decided to not
        /// gracefully handle cancellation here.
        ///
        /// However, if your workload is sensitive to dropped connections such as using an in-memory
        /// SQLite database with a pool size of 1, you can pretty easily ensure that a cancelled
        /// `acquire()` call will never drop connections by tweaking your [`PoolOptions`]:
        ///
        /// * Set [`test_before_acquire(false)`][PoolOptions::test_before_acquire]
        /// * Never set [`before_acquire`][PoolOptions::before_acquire] or
        ///   [`after_connect`][PoolOptions::after_connect].
        ///
        /// This should eliminate any potential `.await` points between acquiring a connection and
        /// returning it.
        pub fn acquire(&self) -> impl Future<Output = Result<PoolConnection>> + 'static {}

        /// Attempts to retrieve a connection from the pool if there is one available.
        ///
        /// Returns `None` immediately if there are no idle connections available in the pool
        /// or there are tasks waiting for a connection which have yet to wake.
        pub fn try_acquire(&self) -> Option<PoolConnection> {}

        /// Retrieves a connection and immediately begins a new transaction.
        pub async fn begin(&self) -> Result<Transaction<'static>> {}

        /// Attempts to retrieve a connection and immediately begins a new transaction if successful.
        pub async fn try_begin(&self) -> Result<Option<Transaction<'static>>> {}

        /// Shut down the connection pool, immediately waking all tasks waiting for a connection.
        ///
        /// Upon calling this method, any currently waiting or subsequent calls to [`Pool::acquire`] and
        /// the like will immediately return [`Error::PoolClosed`] and no new connections will be opened.
        /// Checked-out connections are unaffected, but will be gracefully closed on-drop
        /// rather than being returned to the pool.
        ///
        /// Returns a `Future` which can be `.await`ed to ensure all connections are
        /// gracefully closed. It will first close any idle connections currently waiting in the pool,
        /// then wait for all checked-out connections to be returned or closed.
        ///
        /// Waiting for connections to be gracefully closed is optional, but will allow the database
        /// server to clean up the resources sooner rather than later. This is especially important
        /// for tests that create a new pool every time, otherwise you may see errors about connection
        /// limits being exhausted even when running tests in a single thread.
        ///
        /// If the returned `Future` is not run to completion, any remaining connections will be dropped
        /// when the last handle for the given pool instance is dropped, which could happen in a task
        /// spawned by `Pool` internally and so may be unpredictable otherwise.
        ///
        /// `.close()` may be safely called and `.await`ed on multiple handles concurrently.
        pub fn close(&self) -> impl Future<Output = ()> + '_ {}

        /// Returns `true` if [`.close()`][Pool::close] has been called on the pool, `false` otherwise.
        pub fn is_closed(&self) -> bool {}

        /// Get a future that resolves when [`Pool::close()`] is called.
        ///
        /// If the pool is already closed, the future resolves immediately.
        ///
        /// This can be used to cancel long-running operations that hold onto a [`PoolConnection`]
        /// so they don't prevent the pool from closing (which would otherwise wait until all
        /// connections are returned).
        pub fn close_event(&self) -> CloseEvent {}

        /// Returns the number of connections currently active. This includes idle connections.
        pub fn size(&self) -> u32 {}

        /// Returns the number of connections active and idle (not in use).
        pub fn num_idle(&self) -> usize {}
    }

    impl<'p> Executor<'p> for &crate::pool::Pool
    where
        for<'c> &'c mut crate::Connection: Executor<'c>,
    {
        fn fetch_many<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxStream<'e, Result<Either<QueryResult, Row>>>
        where
            E: Execute + 'q, {
        }

        fn fetch_optional<'e, 'q: 'e, E>(self, query: E) -> BoxFuture<'e, Result<Option<Row>>>
        where
            E: Execute + 'q, {
        }

        fn prepare_with<'e, 'q: 'e>(
            self,
            sql: &'q str,
            parameters: &'e [sqlite::SqliteDataType],
        ) -> BoxFuture<'e, Result<Statement>> {
        }
    }

    /// Returns a new [Pool] tied to the same shared connection pool.
    impl Clone for Pool {
        fn clone(&self) -> Self {}
    }

    impl Debug for Pool {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    /// Make a SQL query.
    pub fn query(sql: &str) -> Query<crate::Arguments> {}

    /// Make a SQL query, with the given arguments.
    pub fn query_with<A>(sql: &str, arguments: A) -> Query<A>
    where
        A: IntoArguments, {
    }

    /// Make a SQL query that is mapped to a concrete type
    /// using [`FromRow`].
    pub fn query_as<'q, O>(sql: &'q str) -> QueryAs<O, crate::Arguments>
    where
        O: for<'r> FromRow<'r>, {
    }

    /// Make a SQL query, with the given arguments, that is mapped to a concrete type
    /// using [`FromRow`].
    pub fn query_as_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryAs<O, A>
    where
        A: IntoArguments,
        O: for<'r> FromRow<'r>, {
    }

    pub struct QueryResult {}

    impl QueryResult {
        pub fn rows_affected(&self) -> u64 {}

        pub fn last_insert_rowid(&self) -> i64 {}
    }

    impl Debug for QueryResult {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl Default for QueryResult {
        fn default() -> QueryResult {}
    }

    impl Extend<QueryResult> for QueryResult {
        fn extend<T: IntoIterator<Item = QueryResult>>(&mut self, iter: T) {}
    }

    /// Make a SQL query that is mapped to a single concrete type
    /// using [`FromRow`].
    pub fn query_scalar<'q, O>(sql: &'q str) -> QueryScalar<O, crate::Arguments>
    where
        (O): for<'r> FromRow<'r>, {
    }

    /// Make a SQL query, with the given arguments, that is mapped to a single concrete type
    /// using [`FromRow`].
    pub fn query_scalar_with<'q, O, A>(sql: &'q str, arguments: A) -> QueryScalar<O, A>
    where
        A: IntoArguments,
        (O): for<'r> FromRow<'r>, {
    }

    /// Implementation of [`Row`] for SQLite.
    pub struct Row {
        pub values: Box<[crate::sqlite::Value]>,
        pub columns: std::sync::Arc<Vec<crate::Column>>,
    }

    impl Row {
        /// Returns `true` if this row has no columns.
        pub fn is_empty(&self) -> bool {}

        /// Get a single value from the row by column index.
        pub fn get_value_idx<'r, T>(&self, index: usize) -> Result<T>
        where
            T: Decode<'r>, {
        }

        /// Get a single value from the row by column name.
        pub fn get_value<'r, T>(&self, column: &str) -> Result<T>
        where
            T: Decode<'r>, {
        }
    }

    impl Send for Row {}

    impl Sync for Row {}

    /// Extended Sqlite error codes
    pub enum ExtendedErrCode {
        ErrorMissingCollseq,
        ErrorRetry,
        ErrorSnapshot,
        IOErrRead,
        IOErrShortRead,
        IOErrWrite,
        IOErrFsync,
        IOErrDirFsync,
        IOErrTruncate,
        IOErrFstat,
        IOErrUnlock,
        IOErrRdlock,
        IOErrDelete,
        IOErrBlocked,
        IOErrNoMem,
        IOErrAccess,
        IOErrCheckReservedLock,
        IOErrLock,
        IOErrClose,
        IOErrDirClose,
        IOErrShmopen,
        IOErrShmsize,
        IOErrShmlock,
        IOErrShmmap,
        IOErrSeek,
        IOErrDeleteNoent,
        IOErrMmap,
        IOErrGetTempPath,
        IOErrConvPath,
        IOErrVnode,
        IOErrAuth,
        IOErrBeginAtomic,
        IOErrCommitAtomic,
        IOErrRollbackAtomic,
        IOErrData,
        IOErrCorruptFs,
        LockedSharedCache,
        LockedVTab,
        BusyRecovery,
        BusySnapshot,
        BusyTimeout,
        CantOpenNoTempDir,
        CantOpenIsDir,
        CantOpenFullPath,
        CantOpenConvPath,
        CantOpenDirtyWal,
        CantOpenSymlink,
        CorruptVTab,
        CorruptSequence,
        CorruptIndex,
        ReadOnlyRecovery,
        ReadOnlyCantLock,
        ReadOnlyRollback,
        ReadOnlyDbMoved,
        ReadOnlyCantInit,
        ReadOnlyDirectory,
        AbortRollback,
        ConstraintCheck,
        ConstraintCommitHook,
        ConstraintForeignKey,
        ConstraintFunction,
        ConstraintNotNull,
        ConstraintPrimaryKey,
        ConstraintTrigger,
        ConstraintUnique,
        ConstraintVTab,
        ConstraintRowId,
        ConstraintPinned,
        ConstraintDataType,
        NoticeRecoverWal,
        NoticeRecoverRollback,
        WarningAutoIndex,
        AuthUser,
        OkLoadPermanently,
        OkSymlink,
        Unknown(u32),
    }

    /// Primary Sqlite error codes
    pub enum PrimaryErrCode {
        Error,
        Internal,
        Perm,
        Abort,
        Busy,
        Locked,
        NoMem,
        ReadOnly,
        Interrupt,
        IoErr,
        Corrupt,
        NotFound,
        Full,
        CantOpen,
        Protocol,
        Empty,
        Schema,
        TooBig,
        Constraint,
        Mismatch,
        Misuse,
        NoLfs,
        Auth,
        Format,
        Range,
        NotADB,
        Notice,
        Warning,
        Unknown(u32),
    }

    pub enum ArgumentValue {
        Null,
        Text(std::sync::Arc<String>),
        Blob(std::sync::Arc<Vec<u8>>),
        Double(f64),
        Int(i32),
        Int64(i64),
    }

    pub struct Arguments {}

    impl Arguments {
        pub fn add<T>(&mut self, value: T)
        where
            T: Encode, {
        }
    }

    impl Default for Arguments {
        fn default() -> Arguments {}
    }

    impl Debug for Arguments {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl IntoArguments for Arguments {
        fn into_arguments(self) -> Arguments {}
    }

    /// A connection to an open [Sqlite] database.
    ///
    /// Because SQLite is an in-process database accessed by blocking API calls, SQLx uses a background
    /// thread and communicates with it via channels to allow non-blocking access to the database.
    ///
    /// Dropping this struct will signal the worker thread to quit and close the database, though
    /// if an error occurs there is no way to pass it back to the user this way.
    ///
    /// You can explicitly call [`.close()`][Self::close] to ensure the database is closed successfully
    /// or get an error otherwise.
    pub struct Connection {}

    impl Connection {
        /// Lock the SQLite database handle out from the worker thread so direct SQLite API calls can
        /// be made safely.
        ///
        /// Returns an error if the worker thread crashed.
        pub async fn lock_handle(&mut self) -> Result<LockedSqliteHandle<'_>> {}

        /// Explicitly close this database connection.
        ///
        /// This notifies the database server that the connection is closing so that it can
        /// free up any server-side resources in use.
        ///
        /// While connections can simply be dropped to clean up local resources,
        /// the `Drop` handler itself cannot notify the server that the connection is being closed
        /// because that may require I/O to send a termination message. That can result in a delay
        /// before the server learns that the connection is gone, usually from a TCP keepalive timeout.
        ///
        /// Creating and dropping many connections in short order without calling `.close()` may
        /// lead to errors from the database server because those senescent connections will still
        /// count against any connection limit or quota that is configured.
        ///
        /// Therefore it is recommended to call `.close()` on a connection when you are done using it
        /// and to `.await` the result to ensure the termination message is sent.
        pub async fn close(self) -> Result<()> {}

        /// Immediately close the connection without sending a graceful shutdown.
        pub async fn close_hard(self) -> Result<()> {}

        /// Begin a new transaction or establish a savepoint within the active transaction.
        ///
        /// Returns a [`Transaction`] for controlling and tracking the new transaction.
        pub fn begin(&mut self) -> BoxFuture<'_, Result<Transaction<'_>>>
        where
            Self: Sized, {
        }

        pub fn cached_statements_size(&self) -> usize {}

        pub async fn clear_cached_statements(&mut self) -> Result<()> {}

        pub fn shrink_buffers(&mut self) {}

        /// Execute the function inside a transaction.
        ///
        /// If the function returns an error, the transaction will be rolled back. If it does not
        /// return an error, the transaction will be committed.
        pub async fn transaction<'a, F, R, E>(&mut self, callback: F) -> Result<R, E>
        where
            for<'c> F:
                FnOnce(&'c mut Transaction<'_>) -> BoxFuture<'c, Result<R, E>> + 'a + Send + Sync,
            Self: Sized,
            R: Send,
            E: From<Error> + Send, {
        }

        /// Establish a new database connection with the provided options.
        pub async fn connect_with(options: &Musq) -> Result<Self>
        where
            Self: Sized, {
        }
    }

    impl<'c> Executor<'c> for &'c mut crate::sqlite::Connection {
        fn fetch_many<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxStream<'e, Result<Either<QueryResult, Row>, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        fn fetch_optional<'e, 'q: 'e, E>(
            self,
            query: E,
        ) -> BoxFuture<'e, Result<Option<Row>, Error>>
        where
            E: Execute + 'q,
            'c: 'e, {
        }

        fn prepare_with<'e, 'q: 'e>(
            self,
            sql: &'q str,
            _parameters: &[SqliteDataType],
        ) -> BoxFuture<'e, Result<Statement, Error>>
        where
            'c: 'e, {
        }
    }

    impl Debug for Connection {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {}
    }

    impl<'c> From<&'c mut Connection> for MaybePoolConnection<'c> {
        fn from(v: &'c mut Connection) -> Self {}
    }

    impl AsRef<Connection> for PoolConnection {
        fn as_ref(&self) -> &Connection {}
    }

    impl AsMut<Connection> for PoolConnection {
        fn as_mut(&mut self) -> &mut Connection {}
    }

    pub trait IntoArguments: Sized + Send {
        fn into_arguments(self) -> Arguments;
    }

    /// Data types supported by SQLite.
    pub enum SqliteDataType {
        Null,
        Int,
        Float,
        Text,
        Blob,
        Numeric,
        Bool,
        Int64,
        Date,
        Time,
        Datetime,
    }

    /// An error returned from Sqlite
    pub struct SqliteError {
        pub primary: PrimaryErrCode,
        pub extended: ExtendedErrCode,
        pub message: String,
    }

    impl SqliteError {}

    impl Debug for SqliteError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl Display for SqliteError {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {}
    }

    impl Error for SqliteError {}

    impl From<SqliteError> for Error {
        fn from(error: SqliteError) -> Self {}
    }

    /// An explicitly prepared statement.
    ///
    /// Statements are prepared and cached by default, per connection. This type allows you to
    /// look at that cache in-between the statement being prepared and it being executed. This contains
    /// the expected columns to be returned and the expected parameter types (if available).
    ///
    /// Statements can be re-used with any connection and on first-use it will be re-prepared and
    /// cached within the connection.
    pub struct Statement {
        pub columns: std::sync::Arc<Vec<crate::Column>>,
    }

    impl Statement {
        pub fn sql(&self) -> &str {}

        pub fn columns(&self) -> &[Column] {}

        pub fn query(&self) -> query::Query<Arguments> {}

        pub fn query_with<A>(&self, arguments: A) -> query::Query<A>
        where
            A: IntoArguments, {
        }

        pub fn query_as<O>(&self) -> query_as::QueryAs<O, Arguments>
        where
            O: for<'r> from_row::FromRow<'r>, {
        }

        pub fn query_as_with<'s, O, A>(&self, arguments: A) -> query_as::QueryAs<O, A>
        where
            O: for<'r> from_row::FromRow<'r>,
            A: IntoArguments, {
        }

        pub fn query_scalar<O>(&self) -> query_scalar::QueryScalar<O, Arguments>
        where
            (O): for<'r> from_row::FromRow<'r>, {
        }

        pub fn query_scalar_with<'s, O, A>(&self, arguments: A) -> query_scalar::QueryScalar<O, A>
        where
            (O): for<'r> from_row::FromRow<'r>,
            A: IntoArguments, {
        }
    }

    impl Debug for Statement {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {}
    }

    impl Clone for Statement {
        fn clone(&self) -> Statement {}
    }

    pub struct Value {}

    impl Value {
        pub fn int(&self) -> i32 {}

        pub fn int64(&self) -> i64 {}

        pub fn double(&self) -> f64 {}

        pub fn blob(&self) -> &[u8] {}

        pub fn text(&self) -> Result<&str, DecodeError> {}

        pub fn type_info(&self) -> SqliteDataType {}

        pub fn is_null(&self) -> bool {}
    }

    impl Clone for Value {
        fn clone(&self) -> Value {}
    }

    /// An in-progress database transaction or savepoint.
    ///
    /// A transaction starts with a call to [`Pool::begin`] or [`Connection::begin`].
    ///
    /// A transaction should end with a call to [`commit`] or [`rollback`]. If neither are called before the transaction
    /// goes out-of-scope, [`rollback`] is called. In other words, [`rollback`] is called on `drop` if the transaction is
    /// still in-progress.
    ///
    /// A savepoint is a special mark inside a transaction that allows all commands that are executed after it was
    /// established to be rolled back, restoring the transaction state to what it was at the time of the savepoint.
    ///
    /// [`Connection::begin`]: crate::connection::Connection::begin()
    /// [`Pool::begin`]: crate::pool::Pool::begin()
    /// [`commit`]: Self::commit()
    /// [`rollback`]: Self::rollback()
    pub struct Transaction<'c> {}

    impl<'c> Transaction<'c> {
        /// Begin a nested transaction
        pub fn begin(conn: impl Into<MaybePoolConnection<'c>>) -> BoxFuture<'c, Result<Self>> {}

        /// Commits this transaction or savepoint.
        pub async fn commit(self) -> Result<()> {}

        /// Aborts this transaction or savepoint.
        pub async fn rollback(self) -> Result<()> {}
    }

    impl<'c> Debug for Transaction<'c> {
        fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {}
    }

    impl<'c> Deref for Transaction<'c> {
        type Target = Connection;
        fn deref(&self) -> &Self::Target {}
    }

    impl<'c> DerefMut for Transaction<'c> {
        fn deref_mut(&mut self) -> &mut Self::Target {}
    }

    impl<'c> Drop for Transaction<'c> {
        fn drop(&mut self) {}
    }

    #[macro_export]
    macro_rules! try_stream {
    ($($block:tt)*) => { ... };
}
    #[macro_export]
    macro_rules! compatible {
    ($x:expr, $($y:path)|+) => { ... };
}
    pub use musq_macros::*;
}

