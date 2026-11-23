//! Declarative read-only SQL query surface contributed by modules and hosts.

use std::{
    fmt::{self, Debug, Formatter},
    sync::Arc,
};

use futures_core::future::BoxFuture;

use crate::{Connection, FunctionFlags, Result, Value};

/// Async pre-authorizer connection hook executed on a fresh read-only sandbox
/// connection before `TEMP` views, limits, and the SQLite authorizer are
/// installed.
///
/// The hook receives the mutable [`Connection`] (opened read-only with full
/// access to base tables before the authorizer is attached) and the SQL text
/// of the incoming query.
pub type SandboxPrepareHook = Arc<
    dyn for<'a> Fn(&'a mut Connection, &'a str) -> BoxFuture<'a, Result<()>> + Send + Sync,
>;

/// Declarative read-only SQL surface exposed by a [`crate::Module`] or host to
/// [`crate::ReadOnlySandbox`].
#[derive(Clone, Default)]
pub struct QuerySurface {
    /// `CREATE TEMP VIEW ...` DDL scripts executed before authorizer
    /// installation.
    pub temp_views: Vec<String>,
    /// Allowlisted `TEMP` view names exposed to sandboxed queries.
    pub allowed_views: Vec<String>,
    /// Base table names in `main` permitted solely for SQLite's `count(*)`
    /// single-table flattener probe (`arg2 == ""`).
    pub count_only_tables: Vec<String>,
    /// Extra SQL scalar or aggregate function names allowlisted alongside the
    /// built-in safe read-only function set.
    pub allowed_functions: Vec<String>,
    /// Pre-authorizer connection preparation hooks (for example, registering
    /// custom scalar functions or loading lookup state).
    pub prepare_hooks: Vec<SandboxPrepareHook>,
}

impl Debug for QuerySurface {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuerySurface")
            .field("temp_views", &self.temp_views.len())
            .field("allowed_views", &self.allowed_views)
            .field("count_only_tables", &self.count_only_tables)
            .field("allowed_functions", &self.allowed_functions)
            .field("prepare_hooks", &self.prepare_hooks.len())
            .finish()
    }
}

impl QuerySurface {
    /// Construct an empty [`QuerySurface`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return `true` when this surface declares no views, tables, functions, or
    /// hooks.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.temp_views.is_empty()
            && self.allowed_views.is_empty()
            && self.count_only_tables.is_empty()
            && self.allowed_functions.is_empty()
            && self.prepare_hooks.is_empty()
    }

    /// Add a `CREATE TEMP VIEW ...` DDL script.
    #[must_use]
    pub fn with_temp_view(mut self, ddl: impl Into<String>) -> Self {
        self.temp_views.push(ddl.into());
        self
    }

    /// Allow sandboxed queries to read `TEMP` view `name`.
    #[must_use]
    pub fn with_allowed_view(mut self, name: impl Into<String>) -> Self {
        push_unique_ascii_ci(&mut self.allowed_views, name.into());
        self
    }

    /// Allow sandboxed queries to read every `TEMP` view in `names`.
    #[must_use]
    pub fn with_allowed_views<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            push_unique_ascii_ci(&mut self.allowed_views, name.into());
        }
        self
    }

    /// Allow SQLite's `count(*)` flattener probe on base table `name` in
    /// `main`.
    #[must_use]
    pub fn with_count_only_table(mut self, name: impl Into<String>) -> Self {
        push_unique_ascii_ci(&mut self.count_only_tables, name.into());
        self
    }

    /// Allow SQLite's `count(*)` flattener probe on every base table in
    /// `names`.
    #[must_use]
    pub fn with_count_only_tables<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            push_unique_ascii_ci(&mut self.count_only_tables, name.into());
        }
        self
    }

    /// Allow sandboxed queries to invoke SQL function `name`.
    #[must_use]
    pub fn with_allowed_function(mut self, name: impl Into<String>) -> Self {
        push_unique_ascii_ci(&mut self.allowed_functions, name.into());
        self
    }

    /// Allow sandboxed queries to invoke every SQL function in `names`.
    #[must_use]
    pub fn with_allowed_functions<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            push_unique_ascii_ci(&mut self.allowed_functions, name.into());
        }
        self
    }

    /// Add an async pre-authorizer connection preparation hook.
    #[must_use]
    pub fn with_prepare_hook<F>(mut self, hook: F) -> Self
    where
        F: for<'a> Fn(&'a mut Connection, &'a str) -> BoxFuture<'a, Result<()>>
            + Send
            + Sync
            + 'static,
    {
        self.prepare_hooks.push(Arc::new(hook));
        self
    }

    /// Register a stateless scalar SQL function on every sandbox connection
    /// and add its `name` to `allowed_functions`.
    #[must_use]
    pub fn with_scalar_function<F>(
        mut self,
        name: impl Into<String>,
        n_args: i32,
        flags: FunctionFlags,
        func: F,
    ) -> Self
    where
        F: Fn(&[Value]) -> Result<Value> + Send + Sync + 'static,
    {
        let fn_name = name.into();
        push_unique_ascii_ci(&mut self.allowed_functions, fn_name.clone());
        let shared = Arc::new(func);
        self.prepare_hooks.push(Arc::new(move |conn, _sql| {
            let fn_name = fn_name.clone();
            let shared = Arc::clone(&shared);
            Box::pin(async move {
                conn.register_function(&fn_name, n_args, flags, move |args| shared(args))
                    .await
            })
        }));
        self
    }

    /// Merge `other` into `self`, deduplicating allowlist entries.
    pub fn merge(&mut self, other: &Self) {
        self.temp_views.extend(other.temp_views.iter().cloned());
        for view in &other.allowed_views {
            push_unique_ascii_ci(&mut self.allowed_views, view.clone());
        }
        for table in &other.count_only_tables {
            push_unique_ascii_ci(&mut self.count_only_tables, table.clone());
        }
        for func in &other.allowed_functions {
            push_unique_ascii_ci(&mut self.allowed_functions, func.clone());
        }
        self.prepare_hooks
            .extend(other.prepare_hooks.iter().cloned());
    }

    /// Return the union of all surfaces in `surfaces`.
    #[must_use]
    pub fn union<'a>(surfaces: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut combined = Self::new();
        for surface in surfaces {
            combined.merge(surface);
        }
        combined
    }
}

/// Insert `value` into `list` if no ASCII case-insensitive match already
/// exists.
fn push_unique_ascii_ci(list: &mut Vec<String>, value: String) {
    if !list.iter().any(|item| item.eq_ignore_ascii_case(&value)) {
        list.push(value);
    }
}
