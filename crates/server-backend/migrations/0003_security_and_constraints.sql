-- 0003_security_and_constraints.sql
-- Enforces case-insensitive uniqueness on users and single active bounty per target.

-- Normalize any existing duplicate-free rows cleanly before indexing
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_username_lower
    ON users (LOWER(username));

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_email_lower
    ON users (LOWER(email));

CREATE UNIQUE INDEX IF NOT EXISTS idx_bounties_one_active_per_target
    ON bounties (target_user_id)
    WHERE active = TRUE;
