#![allow(dead_code)]
//! Integration tests for musq.

#[path = "../src/logger.rs"]
mod logger;

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        fmt,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use log::LevelFilter;
    use logger::{LogSettings, QueryLogger};
    use tracing::{
        Event, Level, Metadata, Subscriber, dispatcher,
        field::{Field, Visit},
        span::{Attributes, Id, Record},
    };

    use super::logger;

    #[derive(Clone, Default)]
    struct CapturingSubscriber {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    #[derive(Clone, Debug)]
    struct CapturedEvent {
        level: Level,
        fields: HashMap<String, String>,
    }

    impl CapturingSubscriber {
        fn events(&self) -> Vec<CapturedEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    struct FieldVisitor<'a> {
        fields: &'a mut HashMap<String, String>,
    }

    impl<'a> Visit for FieldVisitor<'a> {
        fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    impl Subscriber for CapturingSubscriber {
        fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _attrs: &Attributes<'_>) -> Id {
            Id::from_u64(1)
        }

        fn record(&self, _span: &Id, _values: &Record<'_>) {}

        fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

        fn event(&self, event: &Event<'_>) {
            let mut fields = HashMap::new();
            let mut visitor = FieldVisitor {
                fields: &mut fields,
            };
            event.record(&mut visitor);
            self.events.lock().unwrap().push(CapturedEvent {
                level: *event.metadata().level(),
                fields,
            });
        }

        fn enter(&self, _span: &Id) {}

        fn exit(&self, _span: &Id) {}
    }

    #[test]
    fn logs_at_statements_level() {
        let subscriber = CapturingSubscriber::default();
        let dispatch = dispatcher::Dispatch::new(subscriber.clone());
        let _guard = dispatcher::set_default(&dispatch);

        let mut settings = LogSettings::default();
        settings.log_statements(LevelFilter::Info);
        settings.log_slow_statements(LevelFilter::Warn, Duration::from_secs(60));

        let mut logger = QueryLogger::new("SELECT 1", settings);
        logger.increment_rows_returned();
        logger.increase_rows_affected(2);
        drop(logger);
        drop(_guard);

        let events = subscriber.events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.level, Level::INFO);
        assert_eq!(event.fields.get("rows_returned").unwrap(), "1");
        assert_eq!(event.fields.get("rows_affected").unwrap(), "2");
    }

    #[test]
    fn logs_at_slow_level() {
        let subscriber = CapturingSubscriber::default();
        let dispatch = dispatcher::Dispatch::new(subscriber.clone());
        let _guard = dispatcher::set_default(&dispatch);

        let mut settings = LogSettings::default();
        settings.log_statements(LevelFilter::Info);
        settings.log_slow_statements(LevelFilter::Warn, Duration::from_millis(0));

        let mut logger = QueryLogger::new("UPDATE foo", settings);
        logger.increase_rows_affected(5);
        drop(logger);
        drop(_guard);

        let events = subscriber.events();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.level, Level::WARN);
        assert_eq!(event.fields.get("rows_affected").unwrap(), "5");
    }
}
