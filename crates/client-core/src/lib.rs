//! Shared, Wasm-safe client state machine used by both `tui-client` and `wasm-client`.

use chess_core::{BoardState, GameStatus, Side};
use lilac_proto::{
    ClientMsg, GameTermination, MatchCardPlayer, PlayerProfileDto, PlaystyleTag, RrBreakdown,
    ServerMsg, TimeControlKind,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Clone)]
pub struct ActiveMatchView {
    pub game_id: Uuid,
    pub time_control: TimeControlKind,
    pub your_side: Side,
    pub white: MatchCardPlayer,
    pub black: MatchCardPlayer,
    pub board: BoardState,
    pub white_clock_ms: u64,
    pub black_clock_ms: u64,
    pub opening_eco: Option<String>,
    pub opening_name: Option<String>,
    pub last_move_uci: Option<String>,
    pub draw_offered_by: Option<Side>,
    pub rematch_offered_by: Option<Side>,
    pub status: GameStatus,
    pub termination: Option<GameTermination>,
    pub rr_breakdown: Option<RrBreakdown>,
    pub earned_tags: Vec<PlaystyleTag>,
}

/// Serializable UI snapshot for Wasm/JS bridges.
#[derive(Debug, Clone, Serialize)]
pub struct ClientUiSnapshot {
    pub connection: ConnectionState,
    pub username: Option<String>,
    pub in_queue: bool,
    pub queue_stage: Option<String>,
    pub active_game_id: Option<Uuid>,
    pub active_fen: Option<String>,
    pub opening_name: Option<String>,
    pub white_clock_ms: Option<u64>,
    pub black_clock_ms: Option<u64>,
    pub last_rtt_ms: Option<u64>,
    pub last_bounty_alert: Option<String>,
    pub last_error: Option<String>,
}

/// Transport-agnostic client state updated whenever a [`ServerMsg`] arrives.
#[derive(Debug, Clone)]
pub struct ClientAppState {
    pub connection: ConnectionState,
    pub authenticated_user: Option<(Uuid, String)>,
    pub profile: Option<PlayerProfileDto>,
    pub in_queue: bool,
    pub queue_stage: Option<String>,
    pub active_match: Option<ActiveMatchView>,
    pub last_bounty_alert: Option<String>,
    pub last_error: Option<String>,
    pub last_rtt_ms: Option<u64>,
}

impl Default for ClientAppState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientAppState {
    pub fn new() -> Self {
        Self {
            connection: ConnectionState::Disconnected,
            authenticated_user: None,
            profile: None,
            in_queue: false,
            queue_stage: None,
            active_match: None,
            last_bounty_alert: None,
            last_error: None,
            last_rtt_ms: None,
        }
    }

    pub fn ui_snapshot(&self) -> ClientUiSnapshot {
        ClientUiSnapshot {
            connection: self.connection.clone(),
            username: self.authenticated_user.as_ref().map(|(_, u)| u.clone()),
            in_queue: self.in_queue,
            queue_stage: self.queue_stage.clone(),
            active_game_id: self.active_match.as_ref().map(|m| m.game_id),
            active_fen: self.active_match.as_ref().map(|m| m.board.to_fen()),
            opening_name: self
                .active_match
                .as_ref()
                .and_then(|m| m.opening_name.clone()),
            white_clock_ms: self.active_match.as_ref().map(|m| m.white_clock_ms),
            black_clock_ms: self.active_match.as_ref().map(|m| m.black_clock_ms),
            last_rtt_ms: self.last_rtt_ms,
            last_bounty_alert: self.last_bounty_alert.clone(),
            last_error: self.last_error.clone(),
        }
    }

    /// Reduces an incoming [`ServerMsg`] into client state.
    pub fn apply_server_msg(&mut self, msg: ServerMsg, now_ms: u64) {
        match msg {
            ServerMsg::Authenticated { user_id, username } => {
                self.connection = ConnectionState::Connected;
                self.authenticated_user = Some((user_id, username));
                self.last_error = None;
            }
            ServerMsg::QueueStatus {
                in_queue, stage, ..
            } => {
                self.in_queue = in_queue;
                self.queue_stage = if in_queue { Some(stage) } else { None };
            }
            ServerMsg::MatchFound {
                game_id,
                time_control,
                your_side,
                white,
                black,
                initial_fen,
                white_clock_ms,
                black_clock_ms,
            } => {
                self.in_queue = false;
                self.queue_stage = None;
                let board = BoardState::from_fen(&initial_fen).unwrap_or_default();
                self.active_match = Some(ActiveMatchView {
                    game_id,
                    time_control,
                    your_side,
                    white,
                    black,
                    board,
                    white_clock_ms,
                    black_clock_ms,
                    opening_eco: None,
                    opening_name: None,
                    last_move_uci: None,
                    draw_offered_by: None,
                    rematch_offered_by: None,
                    status: GameStatus::Ongoing,
                    termination: None,
                    rr_breakdown: None,
                    earned_tags: Vec::new(),
                });
            }
            ServerMsg::GameSnapshot {
                game_id,
                time_control,
                your_side,
                white,
                black,
                uci_moves,
                fen,
                white_clock_ms,
                black_clock_ms,
                draw_offered_by,
                status,
            } => {
                self.in_queue = false;
                self.queue_stage = None;
                let mut board = BoardState::new();
                for uci in &uci_moves {
                    if board.make_uci_move(uci).is_err() {
                        board = BoardState::from_fen(&fen).unwrap_or_default();
                        break;
                    }
                }
                let opening = board.opening();
                self.active_match = Some(ActiveMatchView {
                    game_id,
                    time_control,
                    your_side,
                    white,
                    black,
                    board,
                    white_clock_ms,
                    black_clock_ms,
                    opening_eco: opening.as_ref().map(|o| o.eco.clone()),
                    opening_name: opening.map(|o| o.name),
                    last_move_uci: uci_moves.last().cloned(),
                    draw_offered_by,
                    rematch_offered_by: None,
                    status,
                    termination: None,
                    rr_breakdown: None,
                    earned_tags: Vec::new(),
                });
            }
            ServerMsg::BoardUpdate {
                game_id,
                uci,
                fen,
                white_clock_ms,
                black_clock_ms,
                opening_eco,
                opening_name,
                status,
                ..
            } => {
                if let Some(m) = self.active_match.as_mut().filter(|m| m.game_id == game_id) {
                    if m.board.make_uci_move(&uci).is_err()
                        && let Ok(synced) = BoardState::from_fen(&fen)
                    {
                        m.board = synced;
                    }
                    m.white_clock_ms = white_clock_ms;
                    m.black_clock_ms = black_clock_ms;
                    m.opening_eco = opening_eco;
                    m.opening_name = opening_name;
                    m.last_move_uci = Some(uci);
                    m.draw_offered_by = None;
                    m.status = status;
                }
            }
            ServerMsg::ClockSync {
                game_id,
                white_clock_ms,
                black_clock_ms,
            } => {
                if let Some(m) = self.active_match.as_mut().filter(|m| m.game_id == game_id) {
                    m.white_clock_ms = white_clock_ms;
                    m.black_clock_ms = black_clock_ms;
                }
            }
            ServerMsg::GameOver {
                game_id,
                status,
                termination,
                rr_breakdown,
                earned_tags,
                ..
            } => {
                if let Some(m) = self.active_match.as_mut().filter(|m| m.game_id == game_id) {
                    m.status = status;
                    m.termination = Some(termination);
                    m.rr_breakdown = rr_breakdown;
                    m.earned_tags = earned_tags;
                }
            }
            ServerMsg::RematchOffered { game_id, by_side } => {
                if let Some(m) = self.active_match.as_mut().filter(|m| m.game_id == game_id) {
                    m.rematch_offered_by = Some(by_side);
                }
            }
            ServerMsg::BountyAlert { message, .. } => {
                self.last_bounty_alert = Some(message);
            }
            ServerMsg::Pong { client_time_ms, .. } => {
                self.last_rtt_ms = Some(now_ms.saturating_sub(client_time_ms));
            }
            ServerMsg::Echo { .. } => {}
            ServerMsg::Error { code, message } => {
                self.last_error = Some(format!("{code}: {message}"));
            }
        }
    }
}

pub fn encode_client_msg(msg: &ClientMsg) -> Result<String, serde_json::Error> {
    serde_json::to_string(msg)
}

pub fn encode_join_queue(time_control: TimeControlKind) -> String {
    encode_client_msg(&ClientMsg::JoinQueue { time_control }).unwrap_or_default()
}

pub fn encode_make_move(game_id: Uuid, uci: String, client_timestamp_ms: u64) -> String {
    encode_client_msg(&ClientMsg::MakeMove {
        game_id,
        uci,
        client_timestamp_ms,
    })
    .unwrap_or_default()
}

pub fn encode_resign(game_id: Uuid) -> String {
    encode_client_msg(&ClientMsg::Resign { game_id }).unwrap_or_default()
}

pub fn encode_ping(client_time_ms: u64) -> String {
    encode_client_msg(&ClientMsg::Ping { client_time_ms }).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pong_updates_rtt() {
        let mut state = ClientAppState::new();
        state.apply_server_msg(
            ServerMsg::Pong {
                client_time_ms: 1000,
                server_time_ms: 1015,
            },
            1042,
        );
        assert_eq!(state.last_rtt_ms, Some(42));
    }
}
