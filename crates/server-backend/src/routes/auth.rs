use axum::{Json, extract::State, http::StatusCode};
use lilac_proto::{AuthResponse, LoginRequest, RegisterRequest, TimeControlKind};

use crate::{
    app::AppState,
    auth::{hash_password_async, issue_jwt, verify_password_async},
    db::{PlayerRatingRow, UserRow},
    error::{AppError, AppResult},
};

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<(StatusCode, Json<AuthResponse>)> {
    let username = req.username.trim();
    let email = req.email.trim().to_ascii_lowercase();

    if username.len() < 3 || username.len() > 32 {
        return Err(AppError::BadRequest(
            "username must be between 3 and 32 characters".into(),
        ));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(AppError::BadRequest(
            "username may only contain ASCII letters, numbers, '_' or '-'".into(),
        ));
    }
    if !email.contains('@') || email.contains(char::is_whitespace) {
        return Err(AppError::BadRequest("invalid email address".into()));
    }

    let password_hash = hash_password_async(req.password).await?;
    let user = UserRow::create(&state.db, username, &email, &password_hash).await?;

    // Initialize default Bronze I (50/100 RR) Blitz rating row.
    let _ = PlayerRatingRow::get_or_init(&state.db, user.id, TimeControlKind::Blitz).await?;

    let token = issue_jwt(
        user.id,
        &user.username,
        &state.config.jwt_secret,
        state.config.jwt_ttl_hours,
    )?;

    Ok((
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
            user_id: user.id,
            username: user.username,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let user = UserRow::find_by_login(&state.db, req.username_or_email.trim())
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid credentials".into()))?;

    let valid = verify_password_async(req.password, user.password_hash.clone()).await?;
    if !valid {
        return Err(AppError::Unauthorized("invalid credentials".into()));
    }

    let token = issue_jwt(
        user.id,
        &user.username,
        &state.config.jwt_secret,
        state.config.jwt_ttl_hours,
    )?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.id,
        username: user.username,
    }))
}
