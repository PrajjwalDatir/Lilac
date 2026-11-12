use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, FromRow)]
pub struct TournamentRow {
    pub id: Uuid,
    pub cohort_id: Uuid,
    pub name: String,
    pub time_control: String,
    pub format: String,
    pub phase: String,
    pub total_rounds: i32,
    pub current_round: i32,
    pub starts_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TournamentEntryRow {
    pub tournament_id: Uuid,
    pub user_id: Uuid,
    pub faction_tag: Option<String>,
    pub score: f64,
    pub tiebreak: f64,
    pub checked_in: bool,
    pub eliminated: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct TournamentPairingRow {
    pub id: Uuid,
    pub tournament_id: Uuid,
    pub round_number: i32,
    pub white_id: Uuid,
    pub black_id: Uuid,
    pub game_id: Option<Uuid>,
    pub is_nemesis_matchup: bool,
    pub result: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl TournamentRow {
    pub async fn create(
        pool: &PgPool,
        cohort_id: Uuid,
        name: &str,
        time_control: &str,
        format: &str,
        total_rounds: i32,
        starts_at: DateTime<Utc>,
    ) -> AppResult<Self> {
        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO tournaments (cohort_id, name, time_control, format, total_rounds, starts_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(cohort_id)
        .bind(name)
        .bind(time_control)
        .bind(format)
        .bind(total_rounds)
        .bind(starts_at)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>("SELECT * FROM tournaments WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }
}
