#![forbid(unsafe_code)]

use flags_2_env_api_server::{config::ApiConfig, server};

fn main() {
    let cfg = ApiConfig::from_env();
    server::run(&cfg);
}

