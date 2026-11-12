use std::sync::Arc;

use axum::{
    Router,
    routing::{get, post},
};
use sqlx::PgPool;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};

use crate::{
    config::AppConfig,
    events::EventBus,
    routes::{auth, health, leaderboard, profile, sandbox, ws},
    ws_hub::ConnectionRegistry,
};

/// Shared application state injected into every Axum route and WebSocket session.
#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub db: PgPool,
    pub event_bus: Arc<dyn EventBus>,
    pub ws_hub: Arc<ConnectionRegistry>,
}

pub fn build_router(state: AppState) -> Router {
    let web_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web");
    let assets_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

    Router::new()
        .route("/health", get(health::health_check))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/profile/me", get(profile::get_my_profile))
        .route("/api/profile/{username}", get(profile::get_profile_by_username))
        .route("/api/leaderboard/global", get(leaderboard::get_global_leaderboard))
        .route(
            "/api/leaderboard/cohort/{cohort_id}",
            get(leaderboard::get_cohort_leaderboard),
        )
        .route("/api/sandbox/inspect", post(sandbox::inspect_sandbox))
        .route("/api/cohorts", get(sandbox::list_cohorts))
        .route("/ws", get(ws::ws_handler))
        .nest_service("/assets", ServeDir::new(assets_dir))
        .fallback_service(ServeDir::new(web_dir))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
