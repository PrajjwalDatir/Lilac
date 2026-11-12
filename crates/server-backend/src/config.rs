use std::env;

use rand_core::{OsRng, RngCore};

/// Runtime configuration for `server-backend`.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub bind_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_ttl_hours: i64,
    pub gold_qualification_target: i32,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();
        let env_mode = env::var("LILAC_ENV").unwrap_or_else(|_| "development".into());
        let bind_addr = env::var("LILAC_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://monarch@localhost:5432/lilac".into());

        let jwt_secret = match env::var("LILAC_JWT_SECRET") {
            Ok(secret) if secret.len() >= 16 => secret,
            Ok(_) if env_mode == "production" => {
                anyhow::bail!("LILAC_JWT_SECRET must be at least 16 characters in production");
            }
            Err(_) if env_mode == "production" => {
                anyhow::bail!("LILAC_JWT_SECRET must be set in production");
            }
            _ => {
                // Generate an ephemeral per-boot secret in dev so no hardcoded secret can be forged.
                let mut bytes = [0u8; 32];
                OsRng.fill_bytes(&mut bytes);
                let ephemeral: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                tracing::warn!(
                    "LILAC_JWT_SECRET not set; generated ephemeral dev JWT secret for this process"
                );
                ephemeral
            }
        };

        let jwt_ttl_hours = env::var("LILAC_JWT_TTL_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(168);
        let gold_qualification_target = env::var("LILAC_GOLD_QUALIFICATION_GAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(lilac_proto::RankTier::DEFAULT_GOLD_QUALIFICATION_GAMES);

        Ok(Self {
            bind_addr,
            database_url,
            jwt_secret,
            jwt_ttl_hours,
            gold_qualification_target,
        })
    }
}
