use log::LevelFilter;
use std::fmt::Debug;
use std::time::Duration;
use std::time::Instant;

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct LogSettings {
    pub statements_level: LevelFilter,
    pub slow_statements_level: LevelFilter,
    pub slow_statements_duration: Duration,
}

impl Default for LogSettings {
    fn default() -> Self {
        LogSettings {
            statements_level: LevelFilter::Debug,
            slow_statements_level: LevelFilter::Warn,
            slow_statements_duration: Duration::from_secs(1),
        }
    }
}

impl LogSettings {
    pub fn log_statements(&mut self, level: LevelFilter) {
        self.statements_level = level;
    }
    pub fn log_slow_statements(&mut self, level: LevelFilter, duration: Duration) {
        self.slow_statements_level = level;
        self.slow_statements_duration = duration;
    }

    /// Returns `true` if any logging level is enabled.
    pub fn is_enabled(&self) -> bool {
        self.statements_level != LevelFilter::Off || self.slow_statements_level != LevelFilter::Off
    }
}

// Yes these look silly. `tracing` doesn't currently support dynamic levels
// https://github.com/tokio-rs/tracing/issues/372
#[doc(hidden)]
macro_rules! private_tracing_dynamic_enabled {
    (target: $target:expr, $level:expr) => {{
        use ::tracing::Level;

        match $level {
            Level::ERROR => ::tracing::enabled!(target: $target, Level::ERROR),
            Level::WARN => ::tracing::enabled!(target: $target, Level::WARN),
            Level::INFO => ::tracing::enabled!(target: $target, Level::INFO),
            Level::DEBUG => ::tracing::enabled!(target: $target, Level::DEBUG),
            Level::TRACE => ::tracing::enabled!(target: $target, Level::TRACE),
        }
    }};
    ($level:expr) => {{
        $crate::private_tracing_dynamic_enabled!(target: module_path!(), $level)
    }};
}

#[doc(hidden)]
macro_rules! private_tracing_dynamic_event {
    (target: $target:expr, $level:expr, $($args:tt)*) => {{
        use ::tracing::Level;

        match $level {
            Level::ERROR => ::tracing::event!(target: $target, Level::ERROR, $($args)*),
            Level::WARN => ::tracing::event!(target: $target, Level::WARN, $($args)*),
            Level::INFO => ::tracing::event!(target: $target, Level::INFO, $($args)*),
            Level::DEBUG => ::tracing::event!(target: $target, Level::DEBUG, $($args)*),
            Level::TRACE => ::tracing::event!(target: $target, Level::TRACE, $($args)*),
        }
    }};
}

#[doc(hidden)]
pub(crate) fn private_level_filter_to_levels(
    filter: log::LevelFilter,
) -> Option<(tracing::Level, log::Level)> {
    let tracing_level = match filter {
        log::LevelFilter::Error => Some(tracing::Level::ERROR),
        log::LevelFilter::Warn => Some(tracing::Level::WARN),
        log::LevelFilter::Info => Some(tracing::Level::INFO),
        log::LevelFilter::Debug => Some(tracing::Level::DEBUG),
        log::LevelFilter::Trace => Some(tracing::Level::TRACE),
        log::LevelFilter::Off => None,
    };

    tracing_level.zip(filter.to_level())
}

pub(crate) struct QueryLogger<'q> {
    sql: &'q str,
    rows_returned: u64,
    rows_affected: u64,
    start: Instant,
    settings: LogSettings,
}

/// Trait implemented by types that can log query execution statistics.
pub(crate) trait QueryLog {
    fn inc_rows_returned(&mut self);
    fn inc_rows_affected(&mut self, n: u64);
}

impl<'q> QueryLogger<'q> {
    pub fn new(sql: &'q str, settings: LogSettings) -> Self {
        Self {
            sql,
            rows_returned: 0,
            rows_affected: 0,
            start: Instant::now(),
            settings,
        }
    }

    pub fn increment_rows_returned(&mut self) {
        self.rows_returned += 1;
    }

    pub fn increase_rows_affected(&mut self, n: u64) {
        self.rows_affected += n;
    }

    pub fn finish(&self) {
        let elapsed = self.start.elapsed();

        let lvl = if elapsed >= self.settings.slow_statements_duration {
            self.settings.slow_statements_level
        } else {
            self.settings.statements_level
        };

        if let Some((tracing_level, log_level)) = private_level_filter_to_levels(lvl) {
            // The enabled level could be set from either tracing world or log world, so check both
            // to see if logging should be enabled for our level
            let log_is_enabled = log::log_enabled!(target: "query", log_level)
                || private_tracing_dynamic_enabled!(target: "query", tracing_level);
            if log_is_enabled {
                let mut summary = parse_query_summary(self.sql);

                let sql = if summary != self.sql {
                    summary.push_str(" …");
                    format!(
                        "\n\n{}\n",
                        sqlformat::format(
                            self.sql,
                            &sqlformat::QueryParams::None,
                            &sqlformat::FormatOptions::default()
                        )
                    )
                } else {
                    String::new()
                };

                private_tracing_dynamic_event!(
                    target: "query",
                    tracing_level,
                    summary,
                    db.statement = sql,
                    rows_affected = self.rows_affected,
                    rows_returned= self.rows_returned,
                    ?elapsed,
                );
            }
        }
    }
}

impl<'q> Drop for QueryLogger<'q> {
    fn drop(&mut self) {
        self.finish();
    }
}

impl<'q> QueryLog for QueryLogger<'q> {
    fn inc_rows_returned(&mut self) {
        self.increment_rows_returned();
    }

    fn inc_rows_affected(&mut self, n: u64) {
        self.increase_rows_affected(n);
    }
}

/// A no-op logger used when query logging is disabled.
#[derive(Default)]
pub(crate) struct NopQueryLogger;

impl QueryLog for NopQueryLogger {
    fn inc_rows_returned(&mut self) {}
    fn inc_rows_affected(&mut self, _n: u64) {}
}

fn parse_query_summary(sql: &str) -> String {
    // For now, just take the first 4 words
    sql.split_whitespace()
        .take(4)
        .collect::<Vec<&str>>()
        .join(" ")
}
