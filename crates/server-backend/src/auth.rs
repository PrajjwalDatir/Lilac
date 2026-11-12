use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{HeaderMap, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{app::AppState, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub username: String,
    pub exp: usize,
    pub iat: usize,
}

/// Authenticated user extracted from `Authorization: Bearer <jwt>`.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub username: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let token = extract_bearer_token(&parts.headers)
            .ok_or_else(|| AppError::Unauthorized("missing authentication token".into()))?;

        let claims = verify_jwt(&token, &app_state.config.jwt_secret)?;
        Ok(Self {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    let stripped = raw
        .strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))?;
    Some(stripped.trim().to_string())
}

/// Synchronous password hashing (used by CLI seed script).
pub fn hash_password(password: &str) -> Result<String, AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("password hash failure: {e}")))?
        .to_string();
    Ok(hash)
}

/// Async-safe Argon2 password hashing offloaded to Tokio's blocking pool.
pub async fn hash_password_async(password: String) -> Result<String, AppError> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking hash join failed: {e}")))?
}

/// Async-safe Argon2 password verification offloaded to Tokio's blocking pool.
pub async fn verify_password_async(password: String, password_hash: String) -> Result<bool, AppError> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&password_hash)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("stored hash invalid: {e}")))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("blocking verify join failed: {e}")))?
}

pub fn issue_jwt(
    user_id: Uuid,
    username: &str,
    secret: &str,
    ttl_hours: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = now + Duration::hours(ttl_hours);
    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt encode error: {e}")))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<Claims, AppError> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| AppError::Unauthorized("invalid or expired token".into()))?;
    Ok(data.claims)
}
