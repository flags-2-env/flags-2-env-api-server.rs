#![forbid(unsafe_code)]

use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub bind: String,
    pub tcp_bind: Option<String>,
    pub nats_url: Option<String>,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        let env = crate::env::load().unwrap_or_else(|err| panic!("{err}"));
        Self::from_map(&env)
    }

    pub fn from_map(environment: &BTreeMap<String, String>) -> Self {
        Self {
            bind: environment
                .get(crate::env::BIND)
                .cloned()
                .unwrap_or_else(|| "127.0.0.1:8080".to_owned()),
            tcp_bind: environment.get(crate::env::TCP_BIND).cloned(),
            nats_url: environment.get(crate::env::NATS_URL).cloned(),
        }
    }
}
