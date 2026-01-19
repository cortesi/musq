use std::{
    fmt::Debug,
    time::{Duration, Instant},
};

use log::LevelFilter;
use tracing::Level;

#[derive(Clone, Debug)]
#[non_exhaustive]
/// Logging configuration for queries.
pub struct LogSettings {
    /// Log level for statements.
    pub statements_level: LevelFilter,
    /// Log level for slow statements.
    pub slow_statements_level: LevelFilter,
    /// Threshold for slow statements.
    pub slow_statements_duration: Duration,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            statements_level: LevelFilter::Debug,
            slow_statements_level: LevelFilter::Warn,
            slow_statements_duration: Duration::from_secs(1),
        }
    }
}

impl LogSettings {
    /// Configure statement logging level.
    pub fn log_statements(&mut self, level: LevelFilter) {
        self.statements_level = level;
    }
    /// Configure slow statement logging level and threshold.
    pub fn log_slow_statements(&mut self, level: LevelFilter, duration: Duration) {
        self.slow_statements_level = level;
        self.slow_statements_duration = duration;
    }

    /// Returns `true` if any logging level is enabled.
    pub fn is_enabled(&self) -> bool {
        self.statements_level != LevelFilter::Off || self.slow_statements_level != LevelFilter::Off
    }
}

#[doc(hidden)]
pub fn private_level_filter_to_levels(filter: log::LevelFilter) -> Option<(Level, log::Level)> {
    let tracing_level = match filter {
        log::LevelFilter::Error => Some(Level::ERROR),
        log::LevelFilter::Warn => Some(Level::WARN),
        log::LevelFilter::Info => Some(Level::INFO),
        log::LevelFilter::Debug => Some(Level::DEBUG),
        log::LevelFilter::Trace => Some(Level::TRACE),
        log::LevelFilter::Off => None,
    };

    tracing_level.zip(filter.to_level())
}

/// Check whether tracing is enabled for the query target at the provided level.
fn tracing_enabled_for(level: Level) -> bool {
    match level {
        Level::ERROR => tracing::enabled!(target: "query", Level::ERROR),
        Level::WARN => tracing::enabled!(target: "query", Level::WARN),
        Level::INFO => tracing::enabled!(target: "query", Level::INFO),
        Level::DEBUG => tracing::enabled!(target: "query", Level::DEBUG),
        Level::TRACE => tracing::enabled!(target: "query", Level::TRACE),
    }
}

/// Emit a tracing event with a dynamically chosen log level.
fn emit_query_event(
    tracing_level: Level,
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    match tracing_level {
        Level::ERROR => emit_query_event_error(summary, sql, rows_affected, rows_returned, elapsed),
        Level::WARN => emit_query_event_warn(summary, sql, rows_affected, rows_returned, elapsed),
        Level::INFO => emit_query_event_info(summary, sql, rows_affected, rows_returned, elapsed),
        Level::DEBUG => emit_query_event_debug(summary, sql, rows_affected, rows_returned, elapsed),
        Level::TRACE => emit_query_event_trace(summary, sql, rows_affected, rows_returned, elapsed),
    }
}

/// Emit an error-level query event.
fn emit_query_event_error(
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    tracing::event!(
        target: "query",
        Level::ERROR,
        summary,
        db.statement = sql,
        rows_affected,
        rows_returned,
        ?elapsed,
    );
}

/// Emit a warn-level query event.
fn emit_query_event_warn(
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    tracing::event!(
        target: "query",
        Level::WARN,
        summary,
        db.statement = sql,
        rows_affected,
        rows_returned,
        ?elapsed,
    );
}

/// Emit an info-level query event.
fn emit_query_event_info(
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    tracing::event!(
        target: "query",
        Level::INFO,
        summary,
        db.statement = sql,
        rows_affected,
        rows_returned,
        ?elapsed,
    );
}

/// Emit a debug-level query event.
fn emit_query_event_debug(
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    tracing::event!(
        target: "query",
        Level::DEBUG,
        summary,
        db.statement = sql,
        rows_affected,
        rows_returned,
        ?elapsed,
    );
}

/// Emit a trace-level query event.
fn emit_query_event_trace(
    summary: &str,
    sql: &str,
    rows_affected: u64,
    rows_returned: u64,
    elapsed: Duration,
) {
    tracing::event!(
        target: "query",
        Level::TRACE,
        summary,
        db.statement = sql,
        rows_affected,
        rows_returned,
        ?elapsed,
    );
}

/// Logger that tracks query execution statistics.
pub struct QueryLogger<'q> {
    /// SQL being executed.
    sql: &'q str,
    /// Count of rows returned.
    rows_returned: u64,
    /// Count of rows affected.
    rows_affected: u64,
    /// Start time for the query.
    start: Instant,
    /// Logging settings in effect.
    settings: LogSettings,
}

/// Trait implemented by types that can log query execution statistics.
pub trait QueryLog {
    /// Increment the rows-returned counter.
    fn inc_rows_returned(&mut self);
    /// Increment the rows-affected counter.
    fn inc_rows_affected(&mut self, n: u64);
}

impl<'q> QueryLogger<'q> {
    /// Create a new query logger.
    pub fn new(sql: &'q str, settings: LogSettings) -> Self {
        Self {
            sql,
            rows_returned: 0,
            rows_affected: 0,
            start: Instant::now(),
            settings,
        }
    }

    /// Increment the rows-returned counter.
    pub fn increment_rows_returned(&mut self) {
        self.rows_returned += 1;
    }

    /// Increment the rows-affected counter.
    pub fn increase_rows_affected(&mut self, n: u64) {
        self.rows_affected += n;
    }

    /// Emit a log event for the completed query.
    pub fn finish(&self) {
        let elapsed = self.start.elapsed();
        let lvl = self.level_for_elapsed(elapsed);

        let Some((tracing_level, log_level)) = private_level_filter_to_levels(lvl) else {
            return;
        };

        if !self.log_is_enabled(tracing_level, log_level) {
            return;
        }

        let (summary, sql) = self.build_log_payload();

        emit_query_event(
            tracing_level,
            summary.as_str(),
            sql.as_str(),
            self.rows_affected,
            self.rows_returned,
            elapsed,
        );
    }

    /// Choose the logging level based on elapsed execution time.
    fn level_for_elapsed(&self, elapsed: Duration) -> LevelFilter {
        if elapsed >= self.settings.slow_statements_duration {
            self.settings.slow_statements_level
        } else {
            self.settings.statements_level
        }
    }

    /// Check if either the log or tracing subscriber is enabled at the given level.
    fn log_is_enabled(&self, tracing_level: Level, log_level: log::Level) -> bool {
        // The enabled level could be set from either tracing world or log world, so check both
        // to see if logging should be enabled for our level.
        log::log_enabled!(target: "query", log_level) || tracing_enabled_for(tracing_level)
    }

    /// Build the summary line and optional formatted SQL payload.
    fn build_log_payload(&self) -> (String, String) {
        let mut summary = parse_query_summary(self.sql);
        if summary != self.sql {
            summary.push_str(" …");
            let formatted = sqlformat::format(
                self.sql,
                &sqlformat::QueryParams::None,
                &sqlformat::FormatOptions::default(),
            );
            (summary, format!("\n\n{}\n", formatted))
        } else {
            (summary, String::new())
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
pub struct NopQueryLogger;

impl QueryLog for NopQueryLogger {
    fn inc_rows_returned(&mut self) {}
    fn inc_rows_affected(&mut self, _n: u64) {}
}

/// Produce a short summary of a SQL statement for logging.
fn parse_query_summary(sql: &str) -> String {
    // For now, just take the first 4 words
    sql.split_whitespace()
        .take(4)
        .collect::<Vec<&str>>()
        .join(" ")
}
