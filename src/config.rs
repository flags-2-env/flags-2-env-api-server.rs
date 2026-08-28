#![forbid(unsafe_code)]

#[derive(Clone, Debug)]
pub struct ApiConfig {
    pub bind: String,
    pub tcp_bind: Option<String>,
    pub nats_url: Option<String>,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        let env = crate::env::load().unwrap_or_else(|err| panic!("{err}"));
        Self {
            bind: crate::env::get(&env, crate::env::BIND)
                .unwrap_or("127.0.0.1:8080")
                .to_owned(),
            tcp_bind: crate::env::get(&env, crate::env::TCP_BIND).map(str::to_owned),
            nats_url: crate::env::get(&env, crate::env::NATS_URL).map(str::to_owned),
        }
    }
}
