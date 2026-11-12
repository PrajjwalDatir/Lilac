use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserRow {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn find_by_username(pool: &PgPool, username: &str) -> AppResult<Option<Self>> {
        let row =
            sqlx::query_as::<_, Self>("SELECT * FROM users WHERE LOWER(username) = LOWER($1)")
                .bind(username)
                .fetch_optional(pool)
                .await?;
        Ok(row)
    }

    pub async fn find_by_login(pool: &PgPool, username_or_email: &str) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>(
            "SELECT * FROM users WHERE LOWER(username) = LOWER($1) OR LOWER(email) = LOWER($1)",
        )
        .bind(username_or_email)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn create(
        pool: &PgPool,
        username: &str,
        email: &str,
        password_hash: &str,
    ) -> AppResult<Self> {
        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO users (username, email, password_hash)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(username)
        .bind(email)
        .bind(password_hash)
        .fetch_one(pool)
        .await
        .map_err(|err| {
            if let sqlx::Error::Database(db_err) = &err
                && db_err.code().as_deref() == Some("23505")
            {
                return AppError::Conflict("username or email already exists".into());
            }
            AppError::Database(err)
        })?;
        Ok(row)
    }
}
