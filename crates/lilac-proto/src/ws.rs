use chess_core::{GameStatus, Side};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    profile::{MatchCardPlayer, PlaystyleTag},
    ranks::{GameTermination, RrBreakdown, TimeControlKind},
};

/// Client -> Server WebSocket messages consumed by `server-backend` (Task 1 & 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Optional post-connect JWT authentication if not passed via header/query.
    Authenticate { token: String },
    /// Join localized Cohort/Nemesis matchmaking queue.
    JoinQueue { time_control: TimeControlKind },
    /// Leave matchmaking queue.
    LeaveQueue,
    /// Submit a chess move in UCI notation (`e2e4`, `e7e8q`).
    MakeMove {
        game_id: Uuid,
        uci: String,
        client_timestamp_ms: u64,
    },
    Resign { game_id: Uuid },
    OfferDraw { game_id: Uuid },
    AcceptDraw { game_id: Uuid },
    OfferRematch { game_id: Uuid },
    AcceptRematch { game_id: Uuid },
    RequestSnapshot { game_id: Uuid },
    Ping { client_time_ms: u64 },
}

/// Server -> Client WebSocket messages consumed identically by `tui-client` and `wasm-client`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Authenticated {
        user_id: Uuid,
        username: String,
    },
    QueueStatus {
        in_queue: bool,
        time_control: Option<TimeControlKind>,
        stage: String,
        wait_ms: u64,
    },
    MatchFound {
        game_id: Uuid,
        time_control: TimeControlKind,
        your_side: Side,
        white: MatchCardPlayer,
        black: MatchCardPlayer,
        initial_fen: String,
        white_clock_ms: u64,
        black_clock_ms: u64,
    },
    BoardUpdate {
        game_id: Uuid,
        ply: u16,
        uci: String,
        san: String,
        fen: String,
        white_clock_ms: u64,
        black_clock_ms: u64,
        opening_eco: Option<String>,
        opening_name: Option<String>,
        status: GameStatus,
    },
    ClockSync {
        game_id: Uuid,
        white_clock_ms: u64,
        black_clock_ms: u64,
    },
    GameOver {
        game_id: Uuid,
        status: GameStatus,
        termination: GameTermination,
        reason: String,
        rr_breakdown: Option<RrBreakdown>,
        earned_tags: Vec<PlaystyleTag>,
    },
    /// Full state hydration payload for mid-game disconnect/reconnect recovery (Task 1).
    GameSnapshot {
        game_id: Uuid,
        time_control: TimeControlKind,
        your_side: Side,
        white: MatchCardPlayer,
        black: MatchCardPlayer,
        uci_moves: Vec<String>,
        fen: String,
        white_clock_ms: u64,
        black_clock_ms: u64,
        draw_offered_by: Option<Side>,
        status: GameStatus,
    },
    RematchOffered {
        game_id: Uuid,
        by_side: Side,
    },
    BountyAlert {
        target_username: String,
        bounty_rr: i32,
        message: String,
    },
    Pong {
        client_time_ms: u64,
        server_time_ms: u64,
    },
    Echo {
        message: String,
    },
    Error {
        code: String,
        message: String,
    },
}
