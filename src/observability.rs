#![forbid(unsafe_code)]

//! Structured next-loggers events for the flags-2-env API server.
//!
//! Configuration is described by counts only. Resolved flag or environment
//! values, bind addresses, and connection strings never enter an event.

use next_loggers::{JsonObject, Logger, Options, Value};

/// Builds the service logger, which writes JSON lines to stdout for the
/// container log pipeline.
#[must_use]
pub fn logger() -> Logger {
    Logger::new(Options {
        app_name: env!("CARGO_PKG_NAME").to_owned(),
        ..Options::default()
    })
}

/// A single numeric field, used for counts that never reveal values.
#[must_use]
pub fn count_field(name: &'static str, count: usize) -> JsonObject {
    JsonObject::from_iter([(name.to_owned(), Value::from(count))])
}

#[cfg(test)]
mod tests {
    use super::count_field;

    #[test]
    fn count_fields_carry_only_the_number() {
        let fields = count_field("config.resolved_entry_count", 3);
        assert_eq!(fields.len(), 1);
        assert_eq!(fields["config.resolved_entry_count"], 3);
    }
}
