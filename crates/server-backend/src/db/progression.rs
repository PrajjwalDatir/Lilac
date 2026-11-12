use chrono::{DateTime, Utc};
use lilac_proto::{LeaderboardEntryDto, RankDivision, RankTier, TimeControlKind};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::AppResult;

#[derive(Debug, Clone, FromRow)]
pub struct MatchRrLedgerRow {
    pub id: Uuid,
    pub game_id: Uuid,
    pub user_id: Uuid,
    pub time_control: String,
    pub base_delta: i32,
    pub nemesis_bonus: i32,
    pub performance_bonus: i32,
    pub net_delta: i32,
    pub tier_before: String,
    pub division_before: i16,
    pub rr_before: i32,
    pub tier_after: String,
    pub division_after: i16,
    pub rr_after: i32,
    pub promoted: bool,
    pub demoted: bool,
    pub unlocked_global_rating: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct BountyRow {
    pub id: Uuid,
    pub target_user_id: Uuid,
    pub cohort_id: Option<Uuid>,
    pub streak_length: i32,
    pub reward_rr: i32,
    pub active: bool,
    pub claimed_by_user_id: Option<Uuid>,
    pub claimed_game_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PlayerOpeningStatsRow {
    pub user_id: Uuid,
    pub eco_family: String,
    pub games_as_white: i32,
    pub wins_as_white: i32,
    pub games_as_black: i32,
    pub wins_as_black: i32,
    pub mastery_xp: i64,
    pub mastery_level: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct PlayerTelemetryTagRow {
    pub user_id: Uuid,
    pub tag: String,
    pub confidence: f64,
    pub sample_games: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct SeasonActRankRow {
    pub user_id: Uuid,
    pub season: i32,
    pub time_control: String,
    pub peak_tier: String,
    pub peak_division: i16,
    pub wins_in_season: i32,
    pub act_global_rating: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct GlobalLbQueryRow {
    user_id: Uuid,
    username: String,
    tier: String,
    division: i16,
    rr_points: i32,
    global_rating: i32,
}

#[derive(Debug, FromRow)]
struct CohortLbQueryRow {
    user_id: Uuid,
    username: String,
    tier: Option<String>,
    division: Option<i16>,
    rr_points: Option<i32>,
    global_rating: Option<i32>,
    global_rating_qualified: Option<bool>,
    cohort_points: i32,
}

pub struct ProgressionRepo;

impl ProgressionRepo {
    /// Returns the public Global Leaderboard containing **only** players who have
    /// reached Gold+ and passed the global rating qualification gate.
    pub async fn fetch_global_leaderboard(
        pool: &PgPool,
        time_control: TimeControlKind,
        limit: i64,
    ) -> AppResult<Vec<LeaderboardEntryDto>> {
        let rows = sqlx::query_as::<_, GlobalLbQueryRow>(
            r#"
            SELECT u.id AS user_id, u.username, pr.tier, pr.division, pr.rr_points, pr.global_rating
            FROM player_ratings pr
            JOIN users u ON u.id = pr.user_id
            WHERE pr.time_control = $1
              AND pr.global_rating_qualified = TRUE
              AND pr.global_rating IS NOT NULL
            ORDER BY pr.global_rating DESC, pr.rr_points DESC
            LIMIT $2
            "#,
        )
        .bind(time_control.as_str())
        .bind(limit.clamp(1, 200))
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .enumerate()
            .map(|(idx, r)| LeaderboardEntryDto {
                rank_position: (idx + 1) as i64,
                user_id: r.user_id,
                username: r.username,
                tier: RankTier::parse(&r.tier).unwrap_or(RankTier::Gold),
                division: RankDivision::from_i16(r.division),
                rr_points: r.rr_points,
                global_rating: Some(r.global_rating),
                cohort_points: None,
            })
            .collect())
    }

    pub async fn fetch_cohort_leaderboard(
        pool: &PgPool,
        cohort_id: Uuid,
        time_control: TimeControlKind,
        limit: i64,
    ) -> AppResult<Vec<LeaderboardEntryDto>> {
        let rows = sqlx::query_as::<_, CohortLbQueryRow>(
            r#"
            SELECT
                u.id AS user_id,
                u.username,
                pr.tier,
                pr.division,
                pr.rr_points,
                pr.global_rating,
                pr.global_rating_qualified,
                cm.cohort_points
            FROM cohort_memberships cm
            JOIN users u ON u.id = cm.user_id
            LEFT JOIN player_ratings pr
                ON pr.user_id = cm.user_id AND pr.time_control = $2
            WHERE cm.cohort_id = $1
            ORDER BY cm.cohort_points DESC, cm.joined_at ASC
            LIMIT $3
            "#,
        )
        .bind(cohort_id)
        .bind(time_control.as_str())
        .bind(limit.clamp(1, 200))
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .enumerate()
            .map(|(idx, r)| {
                let qualified = r.global_rating_qualified.unwrap_or(false);
                LeaderboardEntryDto {
                    rank_position: (idx + 1) as i64,
                    user_id: r.user_id,
                    username: r.username,
                    tier: r
                        .tier
                        .as_deref()
                        .and_then(RankTier::parse)
                        .unwrap_or(RankTier::Bronze),
                    division: RankDivision::from_i16(r.division.unwrap_or(1)),
                    rr_points: r.rr_points.unwrap_or(50),
                    global_rating: r.global_rating.filter(|_| qualified),
                    cohort_points: Some(r.cohort_points),
                }
            })
            .collect())
    }
}
