use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ranks::{RankBadge, RankDivision, RankTier, RrBreakdown, TimeControlKind};

/// Behavioral playstyle tag derived by telemetry workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaystyleTag {
    FastOpener,
    GambitPlayer,
    AggressiveSac,
    DefensiveFortress,
    EndgameSurgeon,
    TimeScrambler,
    NemesisSlayer,
}

/// Opening mastery card displayed on player profiles and pre-game match cards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpeningMasteryCard {
    pub eco_family: String,
    pub mastery_level: i32,
    pub mastery_xp: i64,
    pub games_played: i32,
    pub wins: i32,
    pub win_rate_pct: f32,
}

/// Unlocked or in-progress profile achievement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AchievementBadge {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub progress: i32,
    pub target: i32,
    pub unlocked_at: Option<DateTime<Utc>>,
}

/// Rivalry / Nemesis summary between the viewer and an opponent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NemesisSummary {
    pub opponent_id: Uuid,
    pub opponent_username: String,
    pub wins_against: i32,
    pub losses_against: i32,
    pub draws_against: i32,
    pub rivalry_weight: f64,
    pub active_bounty_rr: i32,
}

/// Valorant-style match-history entry with itemized RR changes and opening info.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchHistoryEntry {
    pub game_id: Uuid,
    pub time_control: TimeControlKind,
    pub opponent_username: String,
    pub opponent_tier_label: String,
    pub outcome: String,
    pub eco_code: Option<String>,
    pub opening_name: Option<String>,
    pub rr_breakdown: Option<RrBreakdown>,
    pub played_at: DateTime<Utc>,
}

/// Pre-game face-off card shown before move 1 (Valorant match-loading style).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchCardPlayer {
    pub user_id: Uuid,
    pub username: String,
    pub rank: RankBadge,
    pub playstyle_tags: Vec<PlaystyleTag>,
    pub signature_openings: Vec<OpeningMasteryCard>,
    pub active_bounty_rr: i32,
    pub head_to_head: Option<(i32, i32, i32)>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoginRequest {
    pub username_or_email: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: Uuid,
    pub username: String,
}

/// Full player profile returned by `GET /api/profile/me` and `/api/profile/{username}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerProfileDto {
    pub user_id: Uuid,
    pub username: String,
    pub cohort_id: Option<Uuid>,
    pub cohort_name: Option<String>,
    pub rank: RankBadge,
    pub playstyle_tags: Vec<PlaystyleTag>,
    pub top_openings: Vec<OpeningMasteryCard>,
    pub achievements: Vec<AchievementBadge>,
    pub recent_matches: Vec<MatchHistoryEntry>,
    pub nemeses: Vec<NemesisSummary>,
    pub season_peaks: Vec<SeasonActRankDto>,
    pub created_at: DateTime<Utc>,
}

/// Valorant-style seasonal peak badge stored per Competitive Act / Season.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonActRankDto {
    pub season: i32,
    pub time_control: TimeControlKind,
    pub peak_tier: RankTier,
    pub peak_division: RankDivision,
    pub wins_in_season: i32,
    pub act_global_rating: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeaderboardEntryDto {
    pub rank_position: i64,
    pub user_id: Uuid,
    pub username: String,
    pub tier: RankTier,
    pub division: RankDivision,
    pub rr_points: i32,
    pub global_rating: Option<i32>,
    pub cohort_points: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlobalLeaderboardDto {
    pub time_control: TimeControlKind,
    pub entries: Vec<LeaderboardEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortLeaderboardDto {
    pub cohort_id: Uuid,
    pub cohort_name: String,
    pub entries: Vec<LeaderboardEntryDto>,
}

