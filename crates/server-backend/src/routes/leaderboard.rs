use axum::{
    Json,
    extract::{Path, Query, State},
};
use lilac_proto::{CohortLeaderboardDto, GlobalLeaderboardDto, TimeControlKind};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    app::AppState,
    db::{CohortRow, ProgressionRepo},
    error::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    pub time_control: Option<TimeControlKind>,
    pub limit: Option<i64>,
}

/// Public Global Leaderboard — strictly shows players who reached Gold+
/// and completed qualification (`global_rating_qualified = TRUE`).
pub async fn get_global_leaderboard(
    State(state): State<AppState>,
    Query(query): Query<LeaderboardQuery>,
) -> AppResult<Json<GlobalLeaderboardDto>> {
    let tc = query.time_control.unwrap_or(TimeControlKind::Blitz);
    let limit = query.limit.unwrap_or(50);
    let entries = ProgressionRepo::fetch_global_leaderboard(&state.db, tc, limit).await?;
    Ok(Json(GlobalLeaderboardDto {
        time_control: tc,
        entries,
    }))
}

pub async fn get_cohort_leaderboard(
    State(state): State<AppState>,
    Path(cohort_id): Path<Uuid>,
    Query(query): Query<LeaderboardQuery>,
) -> AppResult<Json<CohortLeaderboardDto>> {
    let cohort = CohortRow::find_by_id(&state.db, cohort_id)
        .await?
        .ok_or_else(|| AppError::NotFound("cohort not found".into()))?;
    let tc = query.time_control.unwrap_or(TimeControlKind::Blitz);
    let limit = query.limit.unwrap_or(50);
    let entries =
        ProgressionRepo::fetch_cohort_leaderboard(&state.db, cohort_id, tc, limit).await?;

    Ok(Json(CohortLeaderboardDto {
        cohort_id,
        cohort_name: cohort.name,
        entries,
    }))
}
