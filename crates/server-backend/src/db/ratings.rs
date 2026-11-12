use chrono::{DateTime, Utc};
use lilac_proto::{RankBadge, RankDivision, RankTier, TimeControlKind};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, FromRow)]
pub struct PlayerRatingRow {
    pub user_id: Uuid,
    pub time_control: String,
    pub hidden_mmr: f64,
    pub mmr_rd: f64,
    pub mmr_volatility: f64,
    pub tier: String,
    pub division: i16,
    pub rr_points: i32,
    pub demotion_shield: bool,
    pub peak_tier: String,
    pub peak_division: i16,
    pub global_rating_qualified: bool,
    pub global_rating: Option<i32>,
    pub gold_qualification_games: i32,
    pub updated_at: DateTime<Utc>,
}

impl PlayerRatingRow {
    /// Non-mutating read used by `GET /api/profile/*` endpoints.
    pub async fn find_readonly(
        pool: &PgPool,
        user_id: Uuid,
        time_control: TimeControlKind,
    ) -> AppResult<Option<Self>> {
        let row = sqlx::query_as::<_, Self>(
            "SELECT * FROM player_ratings WHERE user_id = $1 AND time_control = $2",
        )
        .bind(user_id)
        .bind(time_control.as_str())
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Initializes a Bronze I rating row for write paths (registration / first match).
    pub async fn get_or_init(
        pool: &PgPool,
        user_id: Uuid,
        time_control: TimeControlKind,
    ) -> AppResult<Self> {
        let tc = time_control.as_str();
        sqlx::query(
            r#"
            INSERT INTO player_ratings (user_id, time_control)
            VALUES ($1, $2)
            ON CONFLICT (user_id, time_control) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(tc)
        .execute(pool)
        .await?;
        let fetched = sqlx::query_as::<_, Self>(
            "SELECT * FROM player_ratings WHERE user_id = $1 AND time_control = $2",
        )
        .bind(user_id)
        .bind(tc)
        .fetch_one(pool)
        .await?;
        Ok(fetched)
    }

    pub fn default_badge(gold_qualification_target: i32) -> RankBadge {
        RankBadge {
            tier: RankTier::Bronze,
            division: RankDivision::I,
            rr_points: 50,
            demotion_shield: false,
            peak_tier: RankTier::Bronze,
            peak_division: RankDivision::I,
            global_rating_qualified: false,
            global_rating: None,
            gold_qualification_games: 0,
            gold_qualification_target,
        }
    }

    pub fn to_badge(&self, gold_qualification_target: i32) -> RankBadge {
        let tier = RankTier::parse(&self.tier).unwrap_or(RankTier::Bronze);
        let peak_tier = RankTier::parse(&self.peak_tier).unwrap_or(tier);
        RankBadge {
            tier,
            division: RankDivision::from_i16(self.division),
            rr_points: self.rr_points,
            demotion_shield: self.demotion_shield,
            peak_tier,
            peak_division: RankDivision::from_i16(self.peak_division),
            global_rating_qualified: self.global_rating_qualified,
            global_rating: self.global_rating.filter(|_| self.global_rating_qualified),
            gold_qualification_games: self.gold_qualification_games,
            gold_qualification_target,
        }
    }
}
