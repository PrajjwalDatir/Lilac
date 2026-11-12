use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, FromRow)]
pub struct GameRow {
    pub id: Uuid,
    pub white_id: Uuid,
    pub black_id: Uuid,
    pub cohort_id: Option<Uuid>,
    pub tournament_id: Option<Uuid>,
    pub time_control: String,
    pub initial_ms: i64,
    pub increment_ms: i64,
    pub status: String,
    pub winner_side: Option<String>,
    pub termination_reason: Option<String>,
    pub initial_fen: String,
    pub final_fen: Option<String>,
    pub pgn_movetext: String,
    pub eco_code: Option<String>,
    pub eco_family: Option<String>,
    pub opening_name: Option<String>,
    pub white_tier_label: String,
    pub black_tier_label: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GameMoveRow {
    pub game_id: Uuid,
    pub ply: i32,
    pub uci: String,
    pub san: String,
    pub fen_after: String,
    pub think_time_ms: i64,
    pub clock_remaining_ms: i64,
    pub played_at: DateTime<Utc>,
}

pub struct NewGameParams<'a> {
    pub id: Uuid,
    pub white_id: Uuid,
    pub black_id: Uuid,
    pub cohort_id: Option<Uuid>,
    pub tournament_id: Option<Uuid>,
    pub time_control: &'a str,
    pub initial_ms: i64,
    pub increment_ms: i64,
    pub initial_fen: &'a str,
    pub white_tier_label: &'a str,
    pub black_tier_label: &'a str,
}

impl GameRow {
    pub async fn create(pool: &PgPool, p: NewGameParams<'_>) -> AppResult<Self> {
        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO games (
                id, white_id, black_id, cohort_id, tournament_id,
                time_control, initial_ms, increment_ms, initial_fen,
                white_tier_label, black_tier_label
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#,
        )
        .bind(p.id)
        .bind(p.white_id)
        .bind(p.black_id)
        .bind(p.cohort_id)
        .bind(p.tournament_id)
        .bind(p.time_control)
        .bind(p.initial_ms)
        .bind(p.increment_ms)
        .bind(p.initial_fen)
        .bind(p.white_tier_label)
        .bind(p.black_tier_label)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>("SELECT * FROM games WHERE id = $1")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn mark_finished(
        pool: &PgPool,
        game_id: Uuid,
        status: &str,
        winner_side: Option<&str>,
        termination_reason: &str,
        final_fen: &str,
        pgn_movetext: &str,
        eco_code: Option<&str>,
        eco_family: Option<&str>,
        opening_name: Option<&str>,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            UPDATE games
            SET status = $2,
                winner_side = $3,
                termination_reason = $4,
                final_fen = $5,
                pgn_movetext = $6,
                eco_code = $7,
                eco_family = $8,
                opening_name = $9,
                ended_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(game_id)
        .bind(status)
        .bind(winner_side)
        .bind(termination_reason)
        .bind(final_fen)
        .bind(pgn_movetext)
        .bind(eco_code)
        .bind(eco_family)
        .bind(opening_name)
        .execute(pool)
        .await?;
        Ok(())
    }
}

impl GameMoveRow {
    pub async fn append(
        pool: &PgPool,
        game_id: Uuid,
        ply: i32,
        uci: &str,
        san: &str,
        fen_after: &str,
        think_time_ms: i64,
        clock_remaining_ms: i64,
    ) -> AppResult<Self> {
        let row = sqlx::query_as::<_, Self>(
            r#"
            INSERT INTO game_moves (game_id, ply, uci, san, fen_after, think_time_ms, clock_remaining_ms)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (game_id, ply) DO NOTHING
            RETURNING *
            "#,
        )
        .bind(game_id)
        .bind(ply)
        .bind(uci)
        .bind(san)
        .bind(fen_after)
        .bind(think_time_ms)
        .bind(clock_remaining_ms)
        .fetch_one(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_for_game(pool: &PgPool, game_id: Uuid) -> AppResult<Vec<Self>> {
        let rows = sqlx::query_as::<_, Self>(
            "SELECT * FROM game_moves WHERE game_id = $1 ORDER BY ply ASC",
        )
        .bind(game_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
