use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, FromRow)]
pub struct CohortRow {
    pub id: Uuid,
    pub name: String,
    pub season: i32,
    pub max_size: i32,
    pub member_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CohortMembershipRow {
    pub cohort_id: Uuid,
    pub user_id: Uuid,
    pub bracket_tier: String,
    pub faction_tag: Option<String>,
    pub cohort_points: i32,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct UserRelationshipRow {
    pub player_a: Uuid,
    pub player_b: Uuid,
    pub wins_a: i32,
    pub wins_b: i32,
    pub draws: i32,
    pub rivalry_weight: f64,
    pub matches_today: i32,
    pub bounty_cooldown_until: Option<DateTime<Utc>>,
    pub last_played_at: DateTime<Utc>,
}

impl CohortRow {
    pub async fn find_by_id(pool: &PgPool, cohort_id: Uuid) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>("SELECT * FROM cohorts WHERE id = $1")
            .bind(cohort_id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn create(
        pool: &PgPool,
        name: &str,
        season: i32,
        max_size: i32,
    ) -> AppResult<Self> {
        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO cohorts (name, season, max_size)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(name)
        .bind(season)
        .bind(max_size)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}

impl CohortMembershipRow {
    pub async fn add_member(
        pool: &PgPool,
        cohort_id: Uuid,
        user_id: Uuid,
        bracket_tier: &str,
    ) -> AppResult<Self> {
        let mut tx = pool.begin().await?;
        let existing = sqlx::query_as::<_, Self>(
            "SELECT * FROM cohort_memberships WHERE cohort_id = $1 AND user_id = $2",
        )
        .bind(cohort_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = existing {
            tx.commit().await?;
            return Ok(row);
        }

        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO cohort_memberships (cohort_id, user_id, bracket_tier)
            VALUES ($1, $2, $3)
            RETURNING *
            "#,
        )
        .bind(cohort_id)
        .bind(user_id)
        .bind(bracket_tier)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query("UPDATE cohorts SET member_count = member_count + 1 WHERE id = $1")
            .bind(cohort_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(row)
    }
}

impl UserRelationshipRow {
    /// Normalizes `(u1, u2)` into canonical `(player_a < player_b)` ordering.
    pub fn canonical_pair(u1: Uuid, u2: Uuid) -> (Uuid, Uuid) {
        if u1 < u2 { (u1, u2) } else { (u2, u1) }
    }

    pub async fn list_for_user(pool: &PgPool, user_id: Uuid, limit: i64) -> AppResult<Vec<Self>> {
        let rows = sqlx::query_as::<_, Self>(
            r#"
            SELECT * FROM user_relationships
            WHERE player_a = $1 OR player_b = $1
            ORDER BY rivalry_weight DESC
            LIMIT $2
            "#,
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
