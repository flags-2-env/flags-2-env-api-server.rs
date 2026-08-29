#![forbid(unsafe_code)]

use flags_2_env_api_server::{config::ApiConfig, flags, server};

fn main() {
    let environment = flags::resolve().unwrap_or_else(|error| panic!("{error}"));
    let cfg = ApiConfig::from_map(&environment);
    server::run(&cfg);
}
