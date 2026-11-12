use chess_core::{GameStatus, Side};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    profile::PlaystyleTag,
    ranks::{RankTier, TimeControlKind},
};

/// Internal domain events published on the backend `EventBus` to decouple
/// Task 1 (State Matrix), Task 2 (Matchmaker), Task 3 (Telemetry/RR), and Task 4 (Tournaments).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    MoveTelemetry {
        game_id: Uuid,
        player_id: Uuid,
        ply: u16,
        uci: String,
        think_time_ms: u64,
        material_balance_cp: i32,
        timestamp: DateTime<Utc>,
    },
    GameCompleted {
        game_id: Uuid,
        white_id: Uuid,
        black_id: Uuid,
        time_control: TimeControlKind,
        status: GameStatus,
        winner: Option<Side>,
        pgn_movetext: String,
        uci_moves: Vec<String>,
        move_times_ms: Vec<u64>,
        eco_code: Option<String>,
        eco_family: Option<String>,
        opening_name: Option<String>,
        tournament_id: Option<Uuid>,
        completed_at: DateTime<Utc>,
    },
    BountyClaimed {
        game_id: Uuid,
        hunter_id: Uuid,
        target_id: Uuid,
        bounty_rr: i32,
    },
    RankUpdated {
        user_id: Uuid,
        time_control: TimeControlKind,
        new_tier: RankTier,
        new_rr: i32,
        unlocked_global_rating: Option<i32>,
    },
    PlaystyleTagAssigned {
        user_id: Uuid,
        tag: PlaystyleTag,
    },
    CohortFilled {
        cohort_id: Uuid,
        member_count: i32,
    },
    TournamentPhaseChanged {
        tournament_id: Uuid,
        cohort_id: Uuid,
        new_phase: String,
    },
}
