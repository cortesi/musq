use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;
use std::time::Instant;

use crate::{connection::LogSettings, logger};

// Yes these look silly. `tracing` doesn't currently support dynamic levels
// https://github.com/tokio-rs/tracing/issues/372
#[doc(hidden)]
#[macro_export]
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
#[macro_export]
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
pub fn private_level_filter_to_levels(
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

pub use sqlformat;

pub struct QueryLogger<'q> {
    sql: &'q str,
    rows_returned: u64,
    rows_affected: u64,
    start: Instant,
    settings: LogSettings,
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
                let mut summary = parse_query_summary(&self.sql);

                let sql = if summary != self.sql {
                    summary.push_str(" …");
                    format!(
                        "\n\n{}\n",
                        sqlformat::format(
                            &self.sql,
                            &sqlformat::QueryParams::None,
                            sqlformat::FormatOptions::default()
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

pub fn parse_query_summary(sql: &str) -> String {
    // For now, just take the first 4 words
    sql.split_whitespace()
        .take(4)
        .collect::<Vec<&str>>()
        .join(" ")
}

pub struct QueryPlanLogger<'q, O: Debug + Hash + Eq, R: Debug, P: Debug> {
    sql: &'q str,
    unknown_operations: HashSet<O>,
    results: Vec<R>,
    program: &'q [P],
    settings: LogSettings,
}

impl<'q, O: Debug + Hash + Eq, R: Debug, P: Debug> QueryPlanLogger<'q, O, R, P> {
    pub fn new(sql: &'q str, program: &'q [P], settings: LogSettings) -> Self {
        Self {
            sql,
            unknown_operations: HashSet::new(),
            results: Vec::new(),
            program,
            settings,
        }
    }

    pub fn log_enabled(&self) -> bool {
        if let Some((tracing_level, log_level)) =
            logger::private_level_filter_to_levels(self.settings.statements_level)
        {
            log::log_enabled!(log_level) || crate::private_tracing_dynamic_enabled!(tracing_level)
        } else {
            false
        }
    }

    pub fn add_result(&mut self, result: R) {
        self.results.push(result);
    }

    pub fn add_unknown_operation(&mut self, operation: O) {
        self.unknown_operations.insert(operation);
    }

    pub fn finish(&self) {
        let lvl = self.settings.statements_level;

        if let Some((tracing_level, log_level)) = logger::private_level_filter_to_levels(lvl) {
            let log_is_enabled = log::log_enabled!(target: "explain", log_level)
                || private_tracing_dynamic_enabled!(target: "explain", tracing_level);
            if log_is_enabled {
                let mut summary = parse_query_summary(&self.sql);

                let sql = if summary != self.sql {
                    summary.push_str(" …");
                    format!(
                        "\n\n{}\n",
                        sqlformat::format(
                            &self.sql,
                            &sqlformat::QueryParams::None,
                            sqlformat::FormatOptions::default()
                        )
                    )
                } else {
                    String::new()
                };

                let message = format!(
                    "{}; program:{:?}, unknown_operations:{:?}, results: {:?}{}",
                    summary, self.program, self.unknown_operations, self.results, sql
                );

                crate::private_tracing_dynamic_event!(
                    target: "explain",
                    tracing_level,
                    message,
                );
            }
        }
    }
}

impl<'q, O: Debug + Hash + Eq, R: Debug, P: Debug> Drop for QueryPlanLogger<'q, O, R, P> {
    fn drop(&mut self) {
        self.finish();
    }
}
