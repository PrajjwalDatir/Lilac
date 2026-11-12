pub mod cohorts;
pub mod games;
pub mod profile;
pub mod progression;
pub mod ratings;
pub mod tournaments;
pub mod users;

pub use cohorts::{CohortMembershipRow, CohortRow, UserRelationshipRow};
pub use games::{GameMoveRow, GameRow, NewGameParams};
pub use profile::PlayerProfileRepo;
pub use progression::{
    BountyRow, MatchRrLedgerRow, PlayerOpeningStatsRow, PlayerTelemetryTagRow, ProgressionRepo,
    SeasonActRankRow,
};
pub use ratings::PlayerRatingRow;
pub use tournaments::{TournamentEntryRow, TournamentPairingRow, TournamentRow};
pub use users::UserRow;

use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
