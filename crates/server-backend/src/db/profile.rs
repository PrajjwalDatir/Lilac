use chrono::{DateTime, Utc};
use lilac_proto::{
    AchievementBadge, MatchHistoryEntry, NemesisSummary, OpeningMasteryCard, PlayerProfileDto,
    PlaystyleTag, RankDivision, RankTier, RrBreakdown, SeasonActRankDto, TimeControlKind,
};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{
    db::{PlayerRatingRow, UserRow},
    error::AppResult,
};

#[derive(Debug, FromRow)]
struct CohortSummaryRow {
    cohort_id: Uuid,
    cohort_name: String,
}

#[derive(Debug, FromRow)]
struct TagRow {
    tag: String,
}

#[derive(Debug, FromRow)]
struct OpeningRow {
    eco_family: String,
    games_as_white: i32,
    wins_as_white: i32,
    games_as_black: i32,
    wins_as_black: i32,
    mastery_xp: i64,
    mastery_level: i32,
}

#[derive(Debug, FromRow)]
struct AchievementRow {
    slug: String,
    name: String,
    description: String,
    category: String,
    target_value: i32,
    progress: Option<i32>,
    unlocked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct RecentMatchRow {
    game_id: Uuid,
    time_control: String,
    opponent_username: String,
    opponent_tier_label: String,
    outcome: String,
    eco_code: Option<String>,
    opening_name: Option<String>,
    played_at: DateTime<Utc>,
    base_delta: Option<i32>,
    nemesis_bonus: Option<i32>,
    performance_bonus: Option<i32>,
    net_delta: Option<i32>,
    tier_before: Option<String>,
    division_before: Option<i16>,
    rr_before: Option<i32>,
    tier_after: Option<String>,
    division_after: Option<i16>,
    rr_after: Option<i32>,
    promoted: Option<bool>,
    demoted: Option<bool>,
    unlocked_global_rating: Option<i32>,
}

#[derive(Debug, FromRow)]
struct NemesisRow {
    opponent_id: Uuid,
    opponent_username: String,
    wins_against: i32,
    losses_against: i32,
    draws_against: i32,
    rivalry_weight: f64,
    active_bounty_rr: i32,
}

pub struct PlayerProfileRepo;

impl PlayerProfileRepo {
    pub async fn fetch_profile(
        pool: &PgPool,
        user: &UserRow,
        time_control: TimeControlKind,
        gold_qualification_target: i32,
    ) -> AppResult<PlayerProfileDto> {
        let rating_fut = PlayerRatingRow::find_readonly(pool, user.id, time_control);

        let cohort_fut = sqlx::query_as::<_, CohortSummaryRow>(
            r#"
            SELECT c.id AS cohort_id, c.name AS cohort_name
            FROM cohort_memberships cm
            JOIN cohorts c ON c.id = cm.cohort_id
            WHERE cm.user_id = $1
            ORDER BY cm.joined_at DESC
            LIMIT 1
            "#,
        )
        .bind(user.id)
        .fetch_optional(pool);

        let tags_fut = sqlx::query_as::<_, TagRow>(
            r#"
            SELECT tag
            FROM player_telemetry_tags
            WHERE user_id = $1
            ORDER BY confidence DESC, sample_games DESC
            LIMIT 5
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let openings_fut = sqlx::query_as::<_, OpeningRow>(
            r#"
            SELECT eco_family, games_as_white, wins_as_white, games_as_black, wins_as_black,
                   mastery_xp, mastery_level
            FROM player_opening_stats
            WHERE user_id = $1
            ORDER BY mastery_xp DESC, (games_as_white + games_as_black) DESC
            LIMIT 8
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let ach_fut = sqlx::query_as::<_, AchievementRow>(
            r#"
            SELECT a.slug, a.name, a.description, a.category, a.target_value,
                   pa.progress, pa.unlocked_at
            FROM achievements a
            LEFT JOIN player_achievements pa
                ON pa.achievement_id = a.id AND pa.user_id = $1
            ORDER BY pa.unlocked_at DESC NULLS LAST, a.id ASC
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let recent_fut = sqlx::query_as::<_, RecentMatchRow>(
            r#"
            SELECT
                g.id AS game_id,
                g.time_control,
                CASE WHEN g.white_id = $1 THEN ub.username ELSE uw.username END AS opponent_username,
                CASE WHEN g.white_id = $1 THEN g.black_tier_label ELSE g.white_tier_label END AS opponent_tier_label,
                COALESCE(g.termination_reason, g.status) AS outcome,
                g.eco_code,
                g.opening_name,
                g.started_at AS played_at,
                l.base_delta,
                l.nemesis_bonus,
                l.performance_bonus,
                l.net_delta,
                l.tier_before,
                l.division_before,
                l.rr_before,
                l.tier_after,
                l.division_after,
                l.rr_after,
                l.promoted,
                l.demoted,
                l.unlocked_global_rating
            FROM games g
            JOIN users uw ON uw.id = g.white_id
            JOIN users ub ON ub.id = g.black_id
            LEFT JOIN match_rr_ledger l ON l.game_id = g.id AND l.user_id = $1
            WHERE (g.white_id = $1 OR g.black_id = $1)
              AND g.status <> 'ongoing'
            ORDER BY g.started_at DESC
            LIMIT 15
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let nemesis_fut = sqlx::query_as::<_, NemesisRow>(
            r#"
            SELECT
                CASE WHEN ur.player_a = $1 THEN ur.player_b ELSE ur.player_a END AS opponent_id,
                u.username AS opponent_username,
                CASE WHEN ur.player_a = $1 THEN ur.wins_a ELSE ur.wins_b END AS wins_against,
                CASE WHEN ur.player_a = $1 THEN ur.wins_b ELSE ur.wins_a END AS losses_against,
                ur.draws AS draws_against,
                ur.rivalry_weight,
                COALESCE((
                    SELECT MAX(b.reward_rr)
                    FROM bounties b
                    WHERE b.target_user_id = CASE WHEN ur.player_a = $1 THEN ur.player_b ELSE ur.player_a END
                      AND b.active = TRUE
                ), 0) AS active_bounty_rr
            FROM user_relationships ur
            JOIN users u ON u.id = CASE WHEN ur.player_a = $1 THEN ur.player_b ELSE ur.player_a END
            WHERE ur.player_a = $1 OR ur.player_b = $1
            ORDER BY ur.rivalry_weight DESC
            LIMIT 10
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let season_fut = sqlx::query_as::<_, crate::db::SeasonActRankRow>(
            r#"
            SELECT * FROM season_act_ranks
            WHERE user_id = $1
            ORDER BY season DESC, time_control ASC
            "#,
        )
        .bind(user.id)
        .fetch_all(pool);

        let (
            rating_row,
            (
                cohort,
                tag_rows,
                opening_rows,
                ach_rows,
                recent_rows,
                nemesis_rows,
                season_rows,
            ),
        ) = tokio::try_join!(
            rating_fut,
            async {
                tokio::try_join!(
                    cohort_fut,
                    tags_fut,
                    openings_fut,
                    ach_fut,
                    recent_fut,
                    nemesis_fut,
                    season_fut
                )
                .map_err(crate::error::AppError::Database)
            }
        )?;

        let rank = rating_row
            .map(|r| r.to_badge(gold_qualification_target))
            .unwrap_or_else(|| PlayerRatingRow::default_badge(gold_qualification_target));

        let playstyle_tags = tag_rows
            .into_iter()
            .filter_map(|r| parse_playstyle_tag(&r.tag))
            .collect();

        let top_openings = opening_rows
            .into_iter()
            .map(|r| {
                let games = r.games_as_white + r.games_as_black;
                let wins = r.wins_as_white + r.wins_as_black;
                let win_rate_pct = if games > 0 {
                    (wins as f32 / games as f32) * 100.0
                } else {
                    0.0
                };
                OpeningMasteryCard {
                    eco_family: r.eco_family,
                    mastery_level: r.mastery_level,
                    mastery_xp: r.mastery_xp,
                    games_played: games,
                    wins,
                    win_rate_pct,
                }
            })
            .collect();

        let achievements = ach_rows
            .into_iter()
            .map(|r| AchievementBadge {
                slug: r.slug,
                name: r.name,
                description: r.description,
                category: r.category,
                progress: r.progress.unwrap_or(0),
                target: r.target_value,
                unlocked_at: r.unlocked_at,
            })
            .collect();

        let recent_matches = recent_rows
            .into_iter()
            .map(|r| {
                let tc = parse_time_control(&r.time_control);
                let rr_breakdown = match (
                    r.base_delta,
                    r.nemesis_bonus,
                    r.performance_bonus,
                    r.net_delta,
                    r.tier_before.as_deref(),
                    r.division_before,
                    r.rr_before,
                    r.tier_after.as_deref(),
                    r.division_after,
                    r.rr_after,
                    r.promoted,
                    r.demoted,
                ) {
                    (
                        Some(base_delta),
                        Some(nemesis_bonus),
                        Some(performance_bonus),
                        Some(net_delta),
                        Some(tb),
                        Some(db),
                        Some(rb),
                        Some(ta),
                        Some(da),
                        Some(ra),
                        Some(promoted),
                        Some(demoted),
                    ) => Some(RrBreakdown {
                        base_delta,
                        nemesis_bonus,
                        performance_bonus,
                        net_delta,
                        tier_before: RankTier::parse(tb).unwrap_or(RankTier::Bronze),
                        division_before: RankDivision::from_i16(db),
                        rr_before: rb,
                        tier_after: RankTier::parse(ta).unwrap_or(RankTier::Bronze),
                        division_after: RankDivision::from_i16(da),
                        rr_after: ra,
                        promoted,
                        demoted,
                        unlocked_global_rating: r.unlocked_global_rating,
                    }),
                    _ => None,
                };
                MatchHistoryEntry {
                    game_id: r.game_id,
                    time_control: tc,
                    opponent_username: r.opponent_username,
                    opponent_tier_label: r.opponent_tier_label,
                    outcome: r.outcome,
                    eco_code: r.eco_code,
                    opening_name: r.opening_name,
                    rr_breakdown,
                    played_at: r.played_at,
                }
            })
            .collect();

        let nemeses = nemesis_rows
            .into_iter()
            .map(|r| NemesisSummary {
                opponent_id: r.opponent_id,
                opponent_username: r.opponent_username,
                wins_against: r.wins_against,
                losses_against: r.losses_against,
                draws_against: r.draws_against,
                rivalry_weight: r.rivalry_weight,
                active_bounty_rr: r.active_bounty_rr,
            })
            .collect();

        let season_peaks = season_rows
            .into_iter()
            .map(|r| SeasonActRankDto {
                season: r.season,
                time_control: parse_time_control(&r.time_control),
                peak_tier: RankTier::parse(&r.peak_tier).unwrap_or(RankTier::Bronze),
                peak_division: RankDivision::from_i16(r.peak_division),
                wins_in_season: r.wins_in_season,
                act_global_rating: r.act_global_rating,
            })
            .collect();

        Ok(PlayerProfileDto {
            user_id: user.id,
            username: user.username.clone(),
            cohort_id: cohort.as_ref().map(|c| c.cohort_id),
            cohort_name: cohort.map(|c| c.cohort_name),
            rank,
            playstyle_tags,
            top_openings,
            achievements,
            recent_matches,
            nemeses,
            season_peaks,
            created_at: user.created_at,
        })
    }
}

fn parse_time_control(raw: &str) -> TimeControlKind {
    match raw {
        "bullet" => TimeControlKind::Bullet,
        "rapid" => TimeControlKind::Rapid,
        "classical" => TimeControlKind::Classical,
        _ => TimeControlKind::Blitz,
    }
}

fn parse_playstyle_tag(raw: &str) -> Option<PlaystyleTag> {
    match raw {
        "fast_opener" => Some(PlaystyleTag::FastOpener),
        "gambit_player" => Some(PlaystyleTag::GambitPlayer),
        "aggressive_sac" => Some(PlaystyleTag::AggressiveSac),
        "defensive_fortress" => Some(PlaystyleTag::DefensiveFortress),
        "endgame_surgeon" => Some(PlaystyleTag::EndgameSurgeon),
        "time_scrambler" => Some(PlaystyleTag::TimeScrambler),
        "nemesis_slayer" => Some(PlaystyleTag::NemesisSlayer),
        _ => None,
    }
}
