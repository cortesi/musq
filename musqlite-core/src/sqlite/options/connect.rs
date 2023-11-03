use crate::error::Error;
use crate::executor::Executor;
use crate::sqlite::{ConnectOptions, Connection};
use futures_core::future::BoxFuture;
use log::LevelFilter;
use std::fmt::Write;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

impl ConnectOptions {
    pub fn from_url(url: &Url) -> Result<Self, Error> {
        // SQLite URL parsing is handled specially;
        // we want to treat the following URLs as equivalent:
        //
        // * sqlite:foo.db
        // * sqlite://foo.db
        //
        // If we used `Url::path()`, the latter would return an empty string
        // because `foo.db` gets parsed as the hostname.
        Self::from_str(url.as_str())
    }

    pub fn connect(&self) -> BoxFuture<'_, Result<Connection, Error>> {
        Box::pin(async move {
            let mut conn = Connection::establish(self).await?;

            // Execute PRAGMAs
            conn.execute(&*self.pragma_string()).await?;

            if !self.collations.is_empty() {
                let mut locked = conn.lock_handle().await?;

                for collation in &self.collations {
                    collation.create(&mut locked.guard.handle)?;
                }
            }

            Ok(conn)
        })
    }

    pub fn log_statements(mut self, level: LevelFilter) -> Self {
        self.log_settings.log_statements(level);
        self
    }

    pub fn log_slow_statements(mut self, level: LevelFilter, duration: Duration) -> Self {
        self.log_settings.log_slow_statements(level, duration);
        self
    }

    /// Collect all `PRAMGA` commands into a single string
    pub(crate) fn pragma_string(&self) -> String {
        let mut string = String::new();

        for (key, opt_value) in &self.pragmas {
            if let Some(value) = opt_value {
                write!(string, "PRAGMA {} = {}; ", key, value).ok();
            }
        }

        string
    }
}
