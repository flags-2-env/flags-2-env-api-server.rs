#![forbid(unsafe_code)]

use std::{error::Error, future::IntoFuture, net::SocketAddr, time::Duration};

use axum::{
    extract::State,
    http::{
        header::{CACHE_CONTROL, CONTENT_TYPE},
        HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use next_loggers::{json, Logger};
use serde::Serialize;
use tokio::{
    net::TcpListener,
    signal,
    sync::oneshot,
    time::{sleep, timeout},
};

use crate::{config::ApiConfig, lifecycle::LifecycleState, routes};

const SERVICE_NAME: &str = env!("CARGO_PKG_NAME");
const SERVICE_SURFACE: &str = "product-api";
const CONTRACT_VERSION: &str = "ores.service-lifecycle/v1";
const DRAIN_TIMEOUT: Duration = Duration::from_secs(45);
const READINESS_PROPAGATION_DELAY: Duration = Duration::from_millis(250);
type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct AppState {
    lifecycle: LifecycleState,
}

#[derive(Serialize)]
struct ProbeBody {
    schema_version: &'static str,
    status: &'static str,
    service: &'static str,
    surface: &'static str,
    revision: String,
}

#[derive(Serialize)]
struct VersionBody {
    schema_version: &'static str,
    service: &'static str,
    surface: &'static str,
    package_version: &'static str,
    revision: String,
    git_sha: Option<&'static str>,
    contract_version: &'static str,
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/v1/catalog", get(catalog))
        .route("/healthz", get(healthz))
        .route("/livez", get(healthz))
        .route("/readyz", get(readyz))
        .route("/startupz", get(startupz))
        .route("/version", get(version))
        .route("/metrics", get(metrics))
        .with_state(state)
}

pub async fn run(config: &ApiConfig, logger: &Logger) -> Result<(), BoxError> {
    const ROUTINE_ID: &str = "ores-routine-YJpeIzXOSzjKcaxWiVs3p";
    // Bind addresses and other configuration values are never recorded.
    let address = match config.bind.parse::<SocketAddr>() {
        Ok(address) => address,
        Err(error) => {
            let _ = logger
                .error(vec![json!("API bind address is invalid")])
                .add_trace("ores-trace-GqXMMRSXYYPpCf9JTbRBZ", false)
                .add_routine_id(ROUTINE_ID)
                .send();
            return Err(error.into());
        }
    };
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            let _ = logger
                .error(vec![json!("API listener bind failed")])
                .add_trace("ores-trace-wMAKrrAZqoE9jzG_8Qp5B", false)
                .add_routine_id(ROUTINE_ID)
                .send();
            return Err(error.into());
        }
    };
    let lifecycle = LifecycleState::from_environment();
    let state = AppState {
        lifecycle: lifecycle.clone(),
    };
    lifecycle.mark_started();
    tracing::info!(service = SERVICE_NAME, %address, "API listener ready");
    let _ = logger
        .info(vec![json!("API listener ready")])
        .add_trace("ores-trace-JWKlCySFMO7dsaacLLxJh", false)
        .add_routine_id(ROUTINE_ID)
        .send();

    let app = match ores_middleware::frameworks::axum_audit::install_from_env(
        router(state),
        env!("CARGO_PKG_NAME"),
    ) {
        Ok(app) => app,
        Err(error) => {
            let _ = logger
                .error(vec![json!("API audit middleware installation failed")])
                .add_trace("ores-trace-FciJqyrWXwKJvjTz1nx38", false)
                .add_routine_id(ROUTINE_ID)
                .send();
            return Err(error.into());
        }
    };

    let (drain_started_tx, drain_started_rx) = oneshot::channel();
    let shutdown_lifecycle = lifecycle.clone();
    let shutdown_logger = logger.clone();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal(&shutdown_logger).await;
            shutdown_lifecycle.begin_drain();
            let _ = shutdown_logger
                .info(vec![json!("API graceful drain started")])
                .add_trace("ores-trace-BFdDfjyGxkwBwUNrUxoLQ", false)
                .add_routine_id(ROUTINE_ID)
                .send();
            sleep(READINESS_PROPAGATION_DELAY).await;
            let _ = drain_started_tx.send(());
        })
        .into_future();
    tokio::pin!(server);

    let result = tokio::select! {
        result = &mut server => result,
        started = drain_started_rx => {
            if started.is_err() {
                let _ = logger
                    .error(vec![json!("API shutdown coordinator stopped before drain began")])
                    .add_trace("ores-trace-aOmrB5XwX4l6R1dcySNoo", false)
                    .add_routine_id(ROUTINE_ID)
                    .send();
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "shutdown coordinator stopped before drain began",
                ))
            } else {
                match timeout(DRAIN_TIMEOUT, &mut server).await {
                    Ok(result) => result,
                    Err(_) => {
                        let _ = logger
                            .error(vec![json!("API graceful shutdown exceeded the drain timeout")])
                            .add_trace("ores-trace-kT-o7VZYe97ln4JNrScQp", false)
                            .add_routine_id(ROUTINE_ID)
                            .send();
                        Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            format!("graceful shutdown exceeded {} seconds", DRAIN_TIMEOUT.as_secs()),
                        ))
                    }
                }
            }
        }
    };
    if let Err(error) = result {
        let _ = logger
            .error(vec![json!("API server stopped with an I/O error")])
            .add_trace("ores-trace-KeZUdPyd91jiSs0l89Re1", false)
            .add_routine_id(ROUTINE_ID)
            .send();
        return Err(error.into());
    }
    let _ = logger
        .info(vec![json!("API server stopped")])
        .add_trace("ores-trace-oWB8XmzVPaM-8KTrzCkKE", false)
        .add_routine_id(ROUTINE_ID)
        .send();
    Ok(())
}

async fn root() -> Response {
    let mut response = Json(serde_json::json!({
        "service": SERVICE_NAME,
        "surface": SERVICE_SURFACE,
        "status": "online"
    }))
    .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn catalog() -> Json<routes::v1::Catalog> {
    Json(routes::v1::catalog())
}

async fn healthz(State(state): State<AppState>) -> Response {
    probe_response(
        StatusCode::OK,
        "ores.service-health/v1",
        "alive",
        &state.lifecycle,
    )
}

async fn readyz(State(state): State<AppState>) -> Response {
    if state.lifecycle.is_ready() {
        probe_response(
            StatusCode::OK,
            "ores.service-readiness/v1",
            "ready",
            &state.lifecycle,
        )
    } else {
        probe_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "ores.service-readiness/v1",
            "not_ready",
            &state.lifecycle,
        )
    }
}

async fn startupz(State(state): State<AppState>) -> Response {
    if state.lifecycle.is_started() {
        probe_response(
            StatusCode::OK,
            "ores.service-startup/v1",
            "started",
            &state.lifecycle,
        )
    } else {
        probe_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "ores.service-startup/v1",
            "starting",
            &state.lifecycle,
        )
    }
}

async fn version(State(state): State<AppState>) -> Response {
    let mut response = Json(VersionBody {
        schema_version: "ores.service-version/v1",
        service: SERVICE_NAME,
        surface: SERVICE_SURFACE,
        package_version: env!("CARGO_PKG_VERSION"),
        revision: state.lifecycle.revision().to_owned(),
        git_sha: option_env!("GIT_SHA").or(option_env!("GITHUB_SHA")),
        contract_version: CONTRACT_VERSION,
    })
    .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn metrics(State(state): State<AppState>) -> Response {
    let body = format!(
        concat!(
            "# HELP ores_service_ready Whether the service is accepting traffic.\n",
            "# TYPE ores_service_ready gauge\n",
            "ores_service_ready{{service=\"{}\",surface=\"{}\"}} {}\n",
            "# HELP ores_service_started Whether startup completed.\n",
            "# TYPE ores_service_started gauge\n",
            "ores_service_started{{service=\"{}\",surface=\"{}\"}} {}\n"
        ),
        SERVICE_NAME,
        SERVICE_SURFACE,
        u8::from(state.lifecycle.is_ready()),
        SERVICE_NAME,
        SERVICE_SURFACE,
        u8::from(state.lifecycle.is_started()),
    );
    let mut response = (StatusCode::OK, body).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn probe_response(
    status_code: StatusCode,
    schema_version: &'static str,
    status: &'static str,
    lifecycle: &LifecycleState,
) -> Response {
    let mut response = (
        status_code,
        Json(ProbeBody {
            schema_version,
            status,
            service: SERVICE_NAME,
            surface: SERVICE_SURFACE,
            revision: lifecycle.revision().to_owned(),
        }),
    )
        .into_response();
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn shutdown_signal(logger: &Logger) {
    const ROUTINE_ID: &str = "ores-routine-uw6dCTQijrv2lCHdDrdUR";
    #[cfg(unix)]
    {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut terminate) => {
                tokio::select! { _ = signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            }
            Err(error) => {
                tracing::error!(%error, "failed to install SIGTERM handler");
                let _ = logger
                    .error(vec![json!("API SIGTERM handler installation failed")])
                    .add_trace("ores-trace-hTUFZmOiHcJ1aSo7P_BXS", false)
                    .add_routine_id(ROUTINE_ID)
                    .send();
                let _ = signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (logger, ROUTINE_ID);
        let _ = signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::{router, AppState, Response};
    use crate::lifecycle::LifecycleState;
    use axum::{
        body::Body,
        http::{header::CACHE_CONTROL, Request, StatusCode},
    };
    use tower::ServiceExt;

    async fn response(state: AppState, path: &str) -> Response {
        router(state)
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn lifecycle_routes_are_separate_and_fail_closed() {
        let lifecycle = LifecycleState::new("test");
        let state = AppState {
            lifecycle: lifecycle.clone(),
        };
        assert_eq!(
            response(state.clone(), "/healthz").await.status(),
            StatusCode::OK
        );
        assert_eq!(
            response(state.clone(), "/readyz").await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            response(state.clone(), "/startupz").await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        lifecycle.mark_started();
        assert_eq!(
            response(state.clone(), "/readyz").await.status(),
            StatusCode::OK
        );
        assert_eq!(
            response(state.clone(), "/startupz").await.status(),
            StatusCode::OK
        );
        lifecycle.begin_drain();
        assert_eq!(
            response(state, "/readyz").await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn operational_responses_are_not_cacheable() {
        let state = AppState {
            lifecycle: LifecycleState::new("test"),
        };
        let response = response(state, "/healthz").await;
        assert_eq!(
            response.headers().get(CACHE_CONTROL),
            Some(&"no-store".parse().unwrap())
        );
    }

    #[tokio::test]
    async fn catalog_route_preserves_the_existing_contract() {
        let state = AppState {
            lifecycle: LifecycleState::new("test"),
        };
        assert_eq!(
            response(state, "/v1/catalog").await.status(),
            StatusCode::OK
        );
    }
}
