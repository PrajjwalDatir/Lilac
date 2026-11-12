use axum::{
    Json,
    extract::{Path, Query, State},
};
use lilac_proto::{PlayerProfileDto, TimeControlKind};
use serde::Deserialize;

use crate::{
    app::AppState,
    auth::AuthUser,
    db::{PlayerProfileRepo, UserRow},
    error::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct ProfileQuery {
    pub time_control: Option<TimeControlKind>,
}

pub async fn get_my_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<ProfileQuery>,
) -> AppResult<Json<PlayerProfileDto>> {
    let user = UserRow::find_by_id(&state.db, auth.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    let tc = query.time_control.unwrap_or(TimeControlKind::Blitz);
    let profile = PlayerProfileRepo::fetch_profile(
        &state.db,
        &user,
        tc,
        state.config.gold_qualification_target,
    )
    .await?;

    Ok(Json(profile))
}

pub async fn get_profile_by_username(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(query): Query<ProfileQuery>,
) -> AppResult<Json<PlayerProfileDto>> {
    let user = UserRow::find_by_username(&state.db, &username)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("player '{username}' not found")))?;

    let tc = query.time_control.unwrap_or(TimeControlKind::Blitz);
    let profile = PlayerProfileRepo::fetch_profile(
        &state.db,
        &user,
        tc,
        state.config.gold_qualification_target,
    )
    .await?;

    Ok(Json(profile))
}
