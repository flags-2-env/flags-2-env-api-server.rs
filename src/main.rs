#![forbid(unsafe_code)]

use flags_2_env_api_server::{config::ApiConfig, flags, observability, server};
use next_loggers::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const ROUTINE_ID: &str = "ores-routine-r9MiiQp8bkLeWcU8tT-AQ";
    let logger = observability::logger();
    let environment = match flags::resolve() {
        Ok(environment) => environment,
        Err(error) => {
            // Resolver messages can quote rejected input, so only the outcome
            // is recorded; the message itself still reaches the exit path.
            let _ = logger
                .error(vec![json!("flags-2-env configuration resolution failed")])
                .add_trace("ores-trace-mTht4qsAYBFWjUzSq-nXB", false)
                .add_routine_id(ROUTINE_ID)
                .send();
            return Err(std::io::Error::other(error).into());
        }
    };
    let _ = logger
        .info(vec![json!("flags-2-env configuration resolved")])
        .add_fields(observability::count_field(
            "config.resolved_entry_count",
            environment.len(),
        ))
        .add_trace("ores-trace-uWier3y6Y1nWxXIrqqWfT", false)
        .add_routine_id(ROUTINE_ID)
        .send();
    let config = ApiConfig::from_map(&environment);
    server::run(&config, &logger).await
}
