use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::app::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
}

pub async fn health_check(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
        .is_ok();

    if db_ok {
        (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ok",
                database: "connected",
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                status: "degraded",
                database: "unreachable",
            }),
        )
    }
}
