use axum::{Json, extract::State};
use chess_core::{BoardState, GameStatus, PieceKind, Side, openings::OpeningInfo};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    app::AppState,
    error::{AppError, AppResult},
};

#[derive(Debug, Deserialize)]
pub struct SandboxInspectRequest {
    pub uci_moves: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SandboxInspectResponse {
    pub fen: String,
    pub turn: Side,
    pub is_check: bool,
    pub status: GameStatus,
    pub legal_moves: Vec<String>,
    pub uci_history: Vec<String>,
    pub san_history: Vec<String>,
    pub pgn_movetext: String,
    pub opening: Option<OpeningInfo>,
    pub phase: &'static str,
    pub material_balance_cp: i32,
    pub captured_by_white: Vec<String>,
    pub captured_by_black: Vec<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CohortListItem {
    pub id: Uuid,
    pub name: String,
    pub season: i32,
    pub member_count: i32,
    pub status: String,
}

fn piece_code(side_prefix: char, kind: PieceKind) -> String {
    let ch = match kind {
        PieceKind::Queen => 'Q',
        PieceKind::Rook => 'R',
        PieceKind::Bishop => 'B',
        PieceKind::Knight => 'N',
        PieceKind::Pawn => 'P',
        PieceKind::King => 'K',
    };
    format!("{side_prefix}{ch}")
}

/// Stateless rules inspection powered directly by `chess-core` for the local
/// Web UI pass-and-play sandbox (keeps 100% of FIDE chess logic in Rust).
pub async fn inspect_sandbox(
    Json(req): Json<SandboxInspectRequest>,
) -> AppResult<Json<SandboxInspectResponse>> {
    let mut board = BoardState::new();
    for uci in &req.uci_moves {
        board
            .make_uci_move(uci)
            .map_err(|e| AppError::BadRequest(format!("invalid move '{uci}': {e}")))?;
    }

    let div = board.divide_phases();
    let phase = if div.endgame_start_ply.is_some() {
        "Endgame"
    } else if div.middlegame_start_ply.is_some() {
        "Middlegame"
    } else {
        "Opening"
    };

    let (cap_w, cap_b) = board.captured_pieces();
    let captured_by_white = cap_w.iter().map(|&k| piece_code('b', k)).collect();
    let captured_by_black = cap_b.iter().map(|&k| piece_code('w', k)).collect();

    Ok(Json(SandboxInspectResponse {
        fen: board.to_fen(),
        turn: board.turn(),
        is_check: board.is_check(),
        status: board.status(),
        legal_moves: board.legal_uci_moves(),
        uci_history: board.uci_history().to_vec(),
        san_history: board.san_history().to_vec(),
        pgn_movetext: board.to_pgn_movetext(),
        opening: board.opening(),
        phase,
        material_balance_cp: board.material_balance_cp(),
        captured_by_white,
        captured_by_black,
    }))
}

pub async fn list_cohorts(State(state): State<AppState>) -> AppResult<Json<Vec<CohortListItem>>> {
    let rows = sqlx::query_as::<_, CohortListItem>(
        r#"
        SELECT id, name, season, member_count, status
        FROM cohorts
        ORDER BY created_at DESC
        LIMIT 20
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}
