//! Idempotent development seed script for Lilac.
//!
//! Usage: `cargo run -p server-backend --bin seed`

use server_backend::{
    auth::hash_password,
    config::AppConfig,
    db::{self, CohortMembershipRow, CohortRow, UserRelationshipRow, UserRow},
};
use uuid::Uuid;

struct SeedPlayer {
    username: &'static str,
    tier: &'static str,
    division: i16,
    rr: i32,
    mmr: f64,
    qualified: bool,
    global_rating: Option<i32>,
    qual_games: i32,
    opening: &'static str,
}

const SEED_PLAYERS: &[SeedPlayer] = &[
    SeedPlayer { username: "iron_rookie", tier: "iron", division: 2, rr: 40, mmr: 780.0, qualified: false, global_rating: None, qual_games: 0, opening: "Italian" },
    SeedPlayer { username: "bronze_brawler", tier: "bronze", division: 3, rr: 82, mmr: 960.0, qualified: false, global_rating: None, qual_games: 0, opening: "London" },
    SeedPlayer { username: "silver_nemesis", tier: "silver", division: 3, rr: 91, mmr: 1210.0, qualified: false, global_rating: None, qual_games: 0, opening: "Sicilian" },
    SeedPlayer { username: "gold_aspirant", tier: "gold", division: 1, rr: 34, mmr: 1410.0, qualified: false, global_rating: None, qual_games: 4, opening: "Caro-Kann" },
    SeedPlayer { username: "gold_qualifier", tier: "gold", division: 2, rr: 65, mmr: 1520.0, qualified: true, global_rating: Some(1525), qual_games: 12, opening: "Queen's Gambit" },
    SeedPlayer { username: "plat_tactician", tier: "platinum", division: 1, rr: 48, mmr: 1690.0, qualified: true, global_rating: Some(1695), qual_games: 18, opening: "Ruy Lopez" },
    SeedPlayer { username: "diamond_sac", tier: "diamond", division: 2, rr: 77, mmr: 1940.0, qualified: true, global_rating: Some(1945), qual_games: 25, opening: "King's Indian" },
    SeedPlayer { username: "radiant_gm", tier: "radiant", division: 3, rr: 312, mmr: 2420.0, qualified: true, global_rating: Some(2420), qual_games: 60, opening: "Sicilian" },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::from_env()?;
    let pool = db::connect_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let cohort = CohortRow::create(&pool, "Alpha Cohort #1", 1, 10_000).await?;
    let pw_hash = hash_password("lilacseed123")
        .map_err(|e| anyhow::anyhow!("seed password hash error: {e}"))?;

    let mut user_ids: Vec<(Uuid, &SeedPlayer)> = Vec::new();
    for sp in SEED_PLAYERS {
        let user = match UserRow::find_by_username(&pool, sp.username).await? {
            Some(existing) => existing,
            None => {
                UserRow::create(
                    &pool,
                    sp.username,
                    &format!("{}@seed.lilac.gg", sp.username),
                    &pw_hash,
                )
                .await?
            }
        };

        sqlx::query(
            r#"
            INSERT INTO player_ratings (
                user_id, time_control, hidden_mmr, tier, division, rr_points,
                peak_tier, peak_division, global_rating_qualified, global_rating, gold_qualification_games
            )
            VALUES ($1, 'blitz', $2, $3, $4, $5, $3, $4, $6, $7, $8)
            ON CONFLICT (user_id, time_control) DO UPDATE SET
                hidden_mmr = EXCLUDED.hidden_mmr,
                tier = EXCLUDED.tier,
                division = EXCLUDED.division,
                rr_points = EXCLUDED.rr_points,
                global_rating_qualified = EXCLUDED.global_rating_qualified,
                global_rating = EXCLUDED.global_rating,
                gold_qualification_games = EXCLUDED.gold_qualification_games
            "#,
        )
        .bind(user.id)
        .bind(sp.mmr)
        .bind(sp.tier)
        .bind(sp.division)
        .bind(sp.rr)
        .bind(sp.qualified)
        .bind(sp.global_rating)
        .bind(sp.qual_games)
        .execute(&pool)
        .await?;

        let _ = CohortMembershipRow::add_member(&pool, cohort.id, user.id, sp.tier).await?;

        sqlx::query(
            r#"
            INSERT INTO player_opening_stats (user_id, eco_family, games_as_white, wins_as_white, mastery_xp, mastery_level)
            VALUES ($1, $2, 12, 8, 450, 3)
            ON CONFLICT (user_id, eco_family) DO NOTHING
            "#,
        )
        .bind(user.id)
        .bind(sp.opening)
        .execute(&pool)
        .await?;

        user_ids.push((user.id, sp));
    }

    // Seed rivalry edges between adjacent players
    for pair in user_ids.windows(2) {
        let (a, b) = UserRelationshipRow::canonical_pair(pair[0].0, pair[1].0);
        sqlx::query(
            r#"
            INSERT INTO user_relationships (player_a, player_b, wins_a, wins_b, draws, rivalry_weight)
            VALUES ($1, $2, 2, 2, 1, 5.0)
            ON CONFLICT (player_a, player_b) DO NOTHING
            "#,
        )
        .bind(a)
        .bind(b)
        .execute(&pool)
        .await?;
    }

    println!(
        "Seeded {} players into cohort '{}' ({})",
        user_ids.len(),
        cohort.name,
        cohort.id
    );
    Ok(())
}
