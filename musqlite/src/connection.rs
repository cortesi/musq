use log::LevelFilter;
use std::fmt::Debug;
use std::time::Duration;

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
}
