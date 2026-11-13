//! WebAssembly client bindings wrapping `client-core` and `chess-core`.
//!
//! Compiles cleanly for `wasm32-unknown-unknown` to drive web UIs
//! from the exact same WebSocket protocol and rules engine as `tui-client`.

use client_core::{
    ClientAppState, encode_join_queue, encode_make_move, encode_ping, encode_resign,
};
use lilac_proto::{ServerMsg, TimeControlKind};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct LilacWasmClient {
    state: ClientAppState,
}

impl Default for LilacWasmClient {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl LilacWasmClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            state: ClientAppState::new(),
        }
    }

    /// Consumes a JSON-encoded [`ServerMsg`] frame from the browser WebSocket.
    pub fn on_server_message(&mut self, json_payload: &str, now_ms: f64) -> Result<(), JsValue> {
        let msg: ServerMsg = serde_json::from_str(json_payload)
            .map_err(|e| JsValue::from_str(&format!("invalid ServerMsg JSON: {e}")))?;
        self.state.apply_server_msg(msg, now_ms.max(0.0) as u64);
        Ok(())
    }

    /// Serializes the current UI state snapshot for JS/React/Svelte/Leptos rendering.
    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.state.ui_snapshot()).unwrap_or_else(|_| "{}".into())
    }

    /// Returns the active match FEN (if in a game), or standard starting FEN.
    pub fn active_fen(&self) -> String {
        self.state
            .active_match
            .as_ref()
            .map(|m| m.board.to_fen())
            .unwrap_or_else(|| chess_core::BoardState::new().to_fen())
    }

    /// Returns all legal UCI moves for the current active match (or starting position).
    pub fn legal_moves_json(&self) -> String {
        let moves = self
            .state
            .active_match
            .as_ref()
            .map(|m| m.board.legal_uci_moves())
            .unwrap_or_else(|| chess_core::BoardState::new().legal_uci_moves());
        serde_json::to_string(&moves).unwrap_or_else(|_| "[]".into())
    }

    pub fn build_join_queue_msg(&self, time_control: &str) -> String {
        let tc = match time_control {
            "bullet" => TimeControlKind::Bullet,
            "rapid" => TimeControlKind::Rapid,
            "classical" => TimeControlKind::Classical,
            _ => TimeControlKind::Blitz,
        };
        encode_join_queue(tc)
    }

    pub fn build_make_move_msg(&self, uci: &str, client_timestamp_ms: f64) -> Result<String, JsValue> {
        let game_id = self
            .state
            .active_match
            .as_ref()
            .map(|m| m.game_id)
            .ok_or_else(|| JsValue::from_str("no active match"))?;
        Ok(encode_make_move(
            game_id,
            uci.to_string(),
            client_timestamp_ms.max(0.0) as u64,
        ))
    }

    pub fn build_resign_msg(&self) -> Result<String, JsValue> {
        let game_id = self
            .state
            .active_match
            .as_ref()
            .map(|m| m.game_id)
            .ok_or_else(|| JsValue::from_str("no active match"))?;
        Ok(encode_resign(game_id))
    }

    pub fn build_ping_msg(&self, client_time_ms: f64) -> String {
        encode_ping(client_time_ms.max(0.0) as u64)
    }

    /// Returns the most recent WebSocket RTT in milliseconds, if measured.
    pub fn last_rtt_ms(&self) -> Option<f64> {
        self.state.last_rtt_ms.map(|ms| ms as f64)
    }
}
