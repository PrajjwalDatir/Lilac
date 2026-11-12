-- Lilac Initial PostgreSQL Schema
-- Covers Base Auth/Profiles + Tasks 1-4 (State Matrix, Nemesis Graph,
-- Telemetry/Valorant RR + Gold+ Global Rating, Autonomous Tournaments).

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 1. Core Users
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(32) NOT NULL UNIQUE,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. Competitive Ratings (Hidden MMR + Valorant 0-100 RR + Gold+ Global Rating Gate)
CREATE TABLE IF NOT EXISTS player_ratings (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    time_control VARCHAR(16) NOT NULL, -- bullet | blitz | rapid | classical
    hidden_mmr DOUBLE PRECISION NOT NULL DEFAULT 1200.0,
    mmr_rd DOUBLE PRECISION NOT NULL DEFAULT 350.0,
    tier VARCHAR(16) NOT NULL DEFAULT 'bronze',
    division SMALLINT NOT NULL DEFAULT 1, -- 1 | 2 | 3
    rr_points INTEGER NOT NULL DEFAULT 50,
    demotion_shield BOOLEAN NOT NULL DEFAULT FALSE,
    peak_tier VARCHAR(16) NOT NULL DEFAULT 'bronze',
    peak_division SMALLINT NOT NULL DEFAULT 1,
    -- Gold+ qualification gate for unlocking actual public global rating:
    global_rating_qualified BOOLEAN NOT NULL DEFAULT FALSE,
    global_rating INTEGER,
    gold_qualification_games INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, time_control)
);

-- 3. Localized 10k Cohorts & Memberships (Task 2)
CREATE TABLE IF NOT EXISTS cohorts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(64) NOT NULL,
    season INTEGER NOT NULL DEFAULT 1,
    max_size INTEGER NOT NULL DEFAULT 10000,
    member_count INTEGER NOT NULL DEFAULT 0,
    status VARCHAR(24) NOT NULL DEFAULT 'forming', -- forming | active | tournament | archived
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cohort_memberships (
    cohort_id UUID NOT NULL REFERENCES cohorts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bracket_tier VARCHAR(16) NOT NULL DEFAULT 'bronze',
    faction_tag VARCHAR(32),
    cohort_points INTEGER NOT NULL DEFAULT 0,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (cohort_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_cohort_memberships_user ON cohort_memberships(user_id);

-- 4. Graph-Relational Player Relationships / Nemesis Edges (Task 2)
CREATE TABLE IF NOT EXISTS user_relationships (
    player_a UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    player_b UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    wins_a INTEGER NOT NULL DEFAULT 0,
    wins_b INTEGER NOT NULL DEFAULT 0,
    draws INTEGER NOT NULL DEFAULT 0,
    rivalry_weight DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    last_played_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_a, player_b),
    CHECK (player_a < player_b)
);

CREATE INDEX IF NOT EXISTS idx_user_relationships_b ON user_relationships(player_b);

-- 5. Games & Move Telemetry (Task 1)
CREATE TABLE IF NOT EXISTS games (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    white_id UUID NOT NULL REFERENCES users(id),
    black_id UUID NOT NULL REFERENCES users(id),
    cohort_id UUID REFERENCES cohorts(id),
    tournament_id UUID,
    time_control VARCHAR(16) NOT NULL,
    initial_ms BIGINT NOT NULL,
    increment_ms BIGINT NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'ongoing',
    winner_side VARCHAR(8),
    termination_reason VARCHAR(64),
    initial_fen TEXT NOT NULL,
    final_fen TEXT,
    pgn_movetext TEXT NOT NULL DEFAULT '',
    eco_code VARCHAR(8),
    eco_family VARCHAR(64),
    opening_name VARCHAR(128),
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_games_white ON games(white_id, started_at DESC);
CREATE INDEX IF NOT EXISTS idx_games_black ON games(black_id, started_at DESC);

CREATE TABLE IF NOT EXISTS game_moves (
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    ply INTEGER NOT NULL,
    uci VARCHAR(8) NOT NULL,
    san VARCHAR(16) NOT NULL,
    fen_after TEXT NOT NULL,
    think_time_ms BIGINT NOT NULL,
    clock_remaining_ms BIGINT NOT NULL,
    played_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (game_id, ply)
);

-- 6. Valorant-Style Itemized RR History (Task 3)
CREATE TABLE IF NOT EXISTS match_rr_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    game_id UUID NOT NULL REFERENCES games(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    time_control VARCHAR(16) NOT NULL,
    base_delta INTEGER NOT NULL,
    nemesis_bonus INTEGER NOT NULL DEFAULT 0,
    performance_bonus INTEGER NOT NULL DEFAULT 0,
    net_delta INTEGER NOT NULL,
    tier_before VARCHAR(16) NOT NULL,
    division_before SMALLINT NOT NULL,
    rr_before INTEGER NOT NULL,
    tier_after VARCHAR(16) NOT NULL,
    division_after SMALLINT NOT NULL,
    rr_after INTEGER NOT NULL,
    promoted BOOLEAN NOT NULL DEFAULT FALSE,
    demoted BOOLEAN NOT NULL DEFAULT FALSE,
    unlocked_global_rating INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_match_rr_ledger_user ON match_rr_ledger(user_id, created_at DESC);

-- 7. Behavioral Playstyle Tags & Bounties (Task 3)
CREATE TABLE IF NOT EXISTS player_telemetry_tags (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    tag VARCHAR(32) NOT NULL,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    sample_games INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, tag)
);

CREATE TABLE IF NOT EXISTS bounties (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cohort_id UUID REFERENCES cohorts(id) ON DELETE SET NULL,
    streak_length INTEGER NOT NULL DEFAULT 3,
    reward_rr INTEGER NOT NULL DEFAULT 10,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    claimed_by_user_id UUID REFERENCES users(id),
    claimed_game_id UUID REFERENCES games(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    claimed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_bounties_active_target ON bounties(target_user_id) WHERE active = TRUE;

-- 8. Opening Mastery & Achievements (Profile Progression)
CREATE TABLE IF NOT EXISTS player_opening_stats (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    eco_family VARCHAR(64) NOT NULL,
    games_as_white INTEGER NOT NULL DEFAULT 0,
    wins_as_white INTEGER NOT NULL DEFAULT 0,
    games_as_black INTEGER NOT NULL DEFAULT 0,
    wins_as_black INTEGER NOT NULL DEFAULT 0,
    mastery_xp BIGINT NOT NULL DEFAULT 0,
    mastery_level INTEGER NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, eco_family)
);

CREATE TABLE IF NOT EXISTS achievements (
    id SERIAL PRIMARY KEY,
    slug VARCHAR(64) NOT NULL UNIQUE,
    name VARCHAR(128) NOT NULL,
    description TEXT NOT NULL,
    category VARCHAR(32) NOT NULL,
    target_value INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS player_achievements (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    achievement_id INTEGER NOT NULL REFERENCES achievements(id) ON DELETE CASCADE,
    progress INTEGER NOT NULL DEFAULT 0,
    unlocked_at TIMESTAMPTZ,
    PRIMARY KEY (user_id, achievement_id)
);

-- Seed canonical gamification achievements (idempotent)
INSERT INTO achievements (slug, name, description, category, target_value) VALUES
    ('first_blood', 'First Blood', 'Win your first ranked match in a Lilac Cohort.', 'milestone', 1),
    ('nemesis_slayer', 'Nemesis Slayer', 'Defeat a registered Nemesis opponent 5 times.', 'rivalry', 5),
    ('bounty_hunter', 'Bounty Hunter', 'Claim 3 active Nemesis win-streak bounties.', 'rivalry', 3),
    ('opening_specialist', 'Opening Specialist', 'Win 25 games within a single ECO opening family.', 'opening', 25),
    ('gambit_addict', 'Gambit Addict', 'Win 10 games after sacrificing material in the opening.', 'playstyle', 10),
    ('gold_qualified', 'Global Qualifier', 'Reach Gold I and complete 10 qualification games to unlock Global Rating.', 'rank', 10)
ON CONFLICT (slug) DO NOTHING;

-- 9. Autonomous Cohort Tournaments (Task 4)
CREATE TABLE IF NOT EXISTS tournaments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cohort_id UUID NOT NULL REFERENCES cohorts(id) ON DELETE CASCADE,
    name VARCHAR(128) NOT NULL,
    time_control VARCHAR(16) NOT NULL,
    format VARCHAR(24) NOT NULL DEFAULT 'swiss', -- swiss | arena | elimination
    phase VARCHAR(24) NOT NULL DEFAULT 'scheduled', -- scheduled | check_in | active | settling | completed
    total_rounds INTEGER NOT NULL DEFAULT 5,
    current_round INTEGER NOT NULL DEFAULT 0,
    starts_at TIMESTAMPTZ NOT NULL,
    ended_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS tournament_entries (
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    faction_tag VARCHAR(32),
    score DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    tiebreak DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    checked_in BOOLEAN NOT NULL DEFAULT FALSE,
    eliminated BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (tournament_id, user_id)
);

CREATE TABLE IF NOT EXISTS tournament_pairings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    round_number INTEGER NOT NULL,
    white_id UUID NOT NULL REFERENCES users(id),
    black_id UUID NOT NULL REFERENCES users(id),
    game_id UUID REFERENCES games(id),
    is_nemesis_matchup BOOLEAN NOT NULL DEFAULT FALSE,
    result VARCHAR(16), -- 1-0 | 0-1 | 1/2-1/2 | bye
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
