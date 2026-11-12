-- 0002_foundation_polish.sql
-- Adds Glicko-2 volatility, Valorant Season/Act rank history, anti-boost cooldowns,
-- anti-cheat flags, opponent tier snapshots on games, FK constraints, and query indexes.

-- 1. Glicko-2 volatility on player_ratings
ALTER TABLE player_ratings
    ADD COLUMN IF NOT EXISTS mmr_volatility DOUBLE PRECISION NOT NULL DEFAULT 0.09;

-- 2. Valorant-style Seasonal / Act Rank History
CREATE TABLE IF NOT EXISTS season_act_ranks (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    season INTEGER NOT NULL,
    time_control VARCHAR(16) NOT NULL,
    peak_tier VARCHAR(16) NOT NULL,
    peak_division SMALLINT NOT NULL DEFAULT 1,
    wins_in_season INTEGER NOT NULL DEFAULT 0,
    act_global_rating INTEGER,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, season, time_control)
);

-- 3. Snapshot opponent tier labels on games + FK to tournaments
ALTER TABLE games
    ADD COLUMN IF NOT EXISTS white_tier_label VARCHAR(32) NOT NULL DEFAULT 'Bronze I',
    ADD COLUMN IF NOT EXISTS black_tier_label VARCHAR(32) NOT NULL DEFAULT 'Bronze I';

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_games_tournament'
    ) THEN
        ALTER TABLE games
            ADD CONSTRAINT fk_games_tournament
            FOREIGN KEY (tournament_id) REFERENCES tournaments(id) ON DELETE SET NULL;
    END IF;
END $$;

-- 4. Anti-boost / Win-trading cooldowns on Nemesis relationships
ALTER TABLE user_relationships
    ADD COLUMN IF NOT EXISTS matches_today INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS bounty_cooldown_until TIMESTAMPTZ;

-- 5. Anti-cheat & anomaly flags (Task 3 / post-game inspection)
CREATE TABLE IF NOT EXISTS cheat_flags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_id UUID REFERENCES games(id) ON DELETE SET NULL,
    flag_type VARCHAR(32) NOT NULL, -- engine_correlation | boost_farming | abort_abuse
    severity DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    resolved BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_cheat_flags_user ON cheat_flags(user_id, resolved);

-- 6. Performance Indexes for Global Leaderboard (Gold+), Matchmaking, & Tournaments
CREATE INDEX IF NOT EXISTS idx_player_ratings_global_lb
    ON player_ratings (time_control, global_rating DESC)
    WHERE global_rating_qualified = TRUE AND global_rating IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_player_ratings_mmr
    ON player_ratings (time_control, hidden_mmr);

CREATE INDEX IF NOT EXISTS idx_games_status_ended
    ON games (status, ended_at DESC);

CREATE INDEX IF NOT EXISTS idx_tournaments_cohort_phase
    ON tournaments (cohort_id, phase);
