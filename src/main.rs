#![forbid(unsafe_code)]

use flags_2_env_api_server::{config::ApiConfig, flags, server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let environment = flags::resolve().map_err(std::io::Error::other)?;
    let config = ApiConfig::from_map(&environment);
    server::run(&config).await
}
