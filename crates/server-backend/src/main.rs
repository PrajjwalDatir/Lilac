use server_backend::{
    app::{AppState, build_router},
    config::AppConfig,
    db,
    events::InMemoryEventBus,
    ws_hub::ConnectionRegistry,
};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,server_backend=debug,tower_http=debug")),
        )
        .init();

    let config = AppConfig::from_env()?;
    let pool = db::connect_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    let state = AppState {
        config: config.clone(),
        db: pool,
        event_bus: InMemoryEventBus::new(1024),
        ws_hub: ConnectionRegistry::new(),
    };

    let router = build_router(state);
    let listener = TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "lilac server listening");

    axum::serve(listener, router).await?;
    Ok(())
}
