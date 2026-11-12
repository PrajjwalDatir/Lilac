//! Pure, Wasm-compatible chess rules engine and ECO opening classifier.
//!
//! Wraps [`shakmaty`] behind Lilac domain types so all crates share exact
//! FIDE move validation, FEN/PGN serialization, Zobrist repetition tracking,
//! incremental phase division, exact capture tracking, and opening classification.

pub mod openings;

use std::collections::HashMap;

use openings::{OpeningInfo, classify_opening};
use serde::{Deserialize, Serialize};
use shakmaty::{
    CastlingMode, Chess, Color, EnPassantMode, Move, Outcome, Position, Role,
    fen::Fen,
    san::{San, SanPlus},
    uci::UciMove,
    zobrist::{Zobrist64, ZobristHash},
};
use thiserror::Error;

/// Errors returned by chess rule validation or notation parsing.
#[derive(Debug, Error)]
pub enum ChessError {
    #[error("invalid FEN string: {0}")]
    InvalidFen(String),
    #[error("invalid UCI move notation: {0}")]
    InvalidUci(String),
    #[error("invalid SAN move notation: {0}")]
    InvalidSan(String),
    #[error("illegal move: {0}")]
    IllegalMove(String),
    #[error("game is already finished")]
    GameFinished,
}

/// Side to move or winner color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Black => "Black",
        }
    }
}

impl From<Color> for Side {
    fn from(value: Color) -> Self {
        match value {
            Color::White => Self::White,
            Color::Black => Self::Black,
        }
    }
}

/// Terminal or ongoing state of a chess game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GameStatus {
    Ongoing,
    Checkmate { winner: Side },
    Stalemate,
    InsufficientMaterial,
    FiftyMoveRule,
    ThreefoldRepetition,
}

impl GameStatus {
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Ongoing)
    }

    pub fn display_label(self) -> String {
        match self {
            Self::Ongoing => "Ongoing".into(),
            Self::Checkmate { winner } => format!("Checkmate — {} wins", winner.label()),
            Self::Stalemate => "Draw by stalemate".into(),
            Self::InsufficientMaterial => "Draw by insufficient material".into(),
            Self::FiftyMoveRule => "Draw by 50-move rule".into(),
            Self::ThreefoldRepetition => "Draw by threefold repetition".into(),
        }
    }
}

/// Recorded half-move (ply) metadata produced after applying a legal move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedMove {
    pub ply: u16,
    pub side: Side,
    pub uci: String,
    pub san: String,
    pub fen_after: String,
    pub is_check: bool,
    pub is_capture: bool,
    pub status_after: GameStatus,
}

/// Stateful chess board wrapping [`Chess`] with move history, O(1) Zobrist
/// repetition counts, incremental phase division, and exact capture history.
#[derive(Debug, Clone)]
pub struct BoardState {
    position: Chess,
    uci_history: Vec<String>,
    san_history: Vec<String>,
    repetition_counts: HashMap<Zobrist64, u8>,
    captured_by_white: Vec<PieceKind>,
    captured_by_black: Vec<PieceKind>,
    middlegame_start_ply: Option<u16>,
    endgame_start_ply: Option<u16>,
}

impl Default for BoardState {
    fn default() -> Self {
        Self::new()
    }
}

impl BoardState {
    /// Creates a standard starting chess position.
    pub fn new() -> Self {
        let position = Chess::default();
        let mut state = Self {
            position,
            uci_history: Vec::new(),
            san_history: Vec::new(),
            repetition_counts: HashMap::new(),
            captured_by_white: Vec::new(),
            captured_by_black: Vec::new(),
            middlegame_start_ply: None,
            endgame_start_ply: None,
        };
        state.record_repetition();
        state.update_phase_markers();
        state
    }

    /// Loads a board from an arbitrary FEN string.
    pub fn from_fen(fen_str: &str) -> Result<Self, ChessError> {
        let fen: Fen = fen_str
            .parse()
            .map_err(|e| ChessError::InvalidFen(format!("{e}")))?;
        let position: Chess = fen
            .into_position(CastlingMode::Standard)
            .map_err(|e| ChessError::InvalidFen(format!("{e}")))?;
        let mut state = Self {
            position,
            uci_history: Vec::new(),
            san_history: Vec::new(),
            repetition_counts: HashMap::new(),
            captured_by_white: Vec::new(),
            captured_by_black: Vec::new(),
            middlegame_start_ply: None,
            endgame_start_ply: None,
        };
        state.record_repetition();
        state.update_phase_markers();
        Ok(state)
    }

    /// Returns the full FEN string of the current position.
    pub fn to_fen(&self) -> String {
        Fen::from_position(self.position.clone(), EnPassantMode::Legal).to_string()
    }

    /// Side currently to move.
    pub fn turn(&self) -> Side {
        self.position.turn().into()
    }

    /// Number of half-moves played since game start.
    pub fn ply_count(&self) -> u16 {
        self.uci_history.len() as u16
    }

    /// Returns all legal moves in UCI notation (e.g. `e2e4`, `e7e8q`).
    pub fn legal_uci_moves(&self) -> Vec<String> {
        self.position
            .legal_moves()
            .into_iter()
            .map(|m| m.to_uci(CastlingMode::Standard).to_string())
            .collect()
    }

    /// Checks whether the side to move is currently in check.
    pub fn is_check(&self) -> bool {
        self.position.is_check()
    }

    /// Computes material balance from White's perspective in centipawns (P=100, N/B=300, R=500, Q=900).
    pub fn material_balance_cp(&self) -> i32 {
        let board = self.position.board();
        let piece_score = |color: Color| -> i32 {
            let p = board.by_color(color);
            (p & board.pawns()).count() as i32 * 100
                + (p & board.knights()).count() as i32 * 300
                + (p & board.bishops()).count() as i32 * 300
                + (p & board.rooks()).count() as i32 * 500
                + (p & board.queens()).count() as i32 * 900
        };
        piece_score(Color::White) - piece_score(Color::Black)
    }

    /// Exact pieces captured by White and Black, sorted by piece value descending.
    pub fn captured_pieces(&self) -> (&[PieceKind], &[PieceKind]) {
        (&self.captured_by_white, &self.captured_by_black)
    }

    /// Returns the current [`GameStatus`].
    pub fn status(&self) -> GameStatus {
        if let Some(outcome) = self.position.outcome() {
            return match outcome {
                Outcome::Decisive { winner } => GameStatus::Checkmate {
                    winner: winner.into(),
                },
                Outcome::Draw => {
                    if self.position.is_stalemate() {
                        GameStatus::Stalemate
                    } else {
                        GameStatus::InsufficientMaterial
                    }
                }
            };
        }

        if self.position.halfmoves() >= 100 {
            return GameStatus::FiftyMoveRule;
        }

        let key = self.repetition_key();
        if self.repetition_counts.get(&key).copied().unwrap_or(0) >= 3 {
            return GameStatus::ThreefoldRepetition;
        }

        GameStatus::Ongoing
    }

    /// Applies a move in UCI notation (e.g. `e2e4`, `e1g1`, `a7a8q`).
    pub fn make_uci_move(&mut self, uci_str: &str) -> Result<AppliedMove, ChessError> {
        if self.status().is_terminal() {
            return Err(ChessError::GameFinished);
        }
        let uci: UciMove = uci_str
            .trim()
            .parse()
            .map_err(|_| ChessError::InvalidUci(uci_str.to_string()))?;
        let mv = uci
            .to_move(&self.position)
            .map_err(|_| ChessError::IllegalMove(uci_str.to_string()))?;
        self.apply_shakmaty_move(mv)
    }

    /// Applies a move in Standard Algebraic Notation (e.g. `Nf3`, `O-O`, `exd5`).
    pub fn make_san_move(&mut self, san_str: &str) -> Result<AppliedMove, ChessError> {
        if self.status().is_terminal() {
            return Err(ChessError::GameFinished);
        }
        let san: San = san_str
            .trim()
            .parse()
            .map_err(|_| ChessError::InvalidSan(san_str.to_string()))?;
        let mv = san
            .to_move(&self.position)
            .map_err(|_| ChessError::IllegalMove(san_str.to_string()))?;
        self.apply_shakmaty_move(mv)
    }

    /// Classifies the current move sequence into an ECO opening, if matched.
    pub fn opening(&self) -> Option<OpeningInfo> {
        classify_opening(&self.uci_history)
    }

    pub fn uci_history(&self) -> &[String] {
        &self.uci_history
    }

    pub fn san_history(&self) -> &[String] {
        &self.san_history
    }

    /// Renders a clean PGN movetext string (e.g. `1. e4 e5 2. Nf3 Nc6`).
    pub fn to_pgn_movetext(&self) -> String {
        let mut out = String::new();
        for (idx, san) in self.san_history.iter().enumerate() {
            if idx % 2 == 0 {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&format!("{}. {}", (idx / 2) + 1, san));
            } else {
                out.push(' ');
                out.push_str(san);
            }
        }
        out
    }

    /// Exports a full PGN document including standard Seven Tag Roster headers.
    pub fn to_full_pgn(&self, event: &str, white: &str, black: &str, result: &str) -> String {
        let eco = self.opening();
        let mut pgn = format!(
            "[Event \"{event}\"]\n[Site \"Lilac\"]\n[White \"{white}\"]\n[Black \"{black}\"]\n[Result \"{result}\"]\n"
        );
        if let Some(info) = eco {
            pgn.push_str(&format!(
                "[ECO \"{}\"]\n[Opening \"{}\"]\n",
                info.eco, info.name
            ));
        }
        pgn.push('\n');
        let moves = self.to_pgn_movetext();
        if moves.is_empty() {
            pgn.push_str(result);
        } else {
            pgn.push_str(&format!("{moves} {result}"));
        }
        pgn.push('\n');
        pgn
    }

    /// Parses standard PGN movetext (ignoring `[Tag "..."]` headers, `{comments}`,
    /// nested `(...)` RAV variations, `$N` NAGs, and move numbers) into a [`BoardState`].
    pub fn from_pgn_movetext(pgn: &str) -> Result<Self, ChessError> {
        let mut board = Self::new();
        let mut in_brace_comment = false;
        let mut paren_depth = 0usize;

        for line in pgn.lines() {
            let trimmed = line.trim();
            if !in_brace_comment
                && paren_depth == 0
                && trimmed.starts_with('[')
                && trimmed.ends_with(']')
            {
                continue;
            }

            let mut chars = trimmed.chars().peekable();
            let mut token = String::new();
            let flush = |tok: &mut String, b: &mut Self| -> Result<(), ChessError> {
                let cleaned = tok.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
                if !cleaned.is_empty()
                    && !cleaned.starts_with('$')
                    && !matches!(cleaned, "1-0" | "0-1" | "1/2-1/2" | "*")
                {
                    b.make_san_move(cleaned)?;
                }
                tok.clear();
                Ok(())
            };

            while let Some(ch) = chars.next() {
                if in_brace_comment {
                    if ch == '}' {
                        in_brace_comment = false;
                    }
                    continue;
                }
                if paren_depth > 0 {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => paren_depth -= 1,
                        _ => {}
                    }
                    continue;
                }
                match ch {
                    '{' => {
                        flush(&mut token, &mut board)?;
                        in_brace_comment = true;
                    }
                    '(' => {
                        flush(&mut token, &mut board)?;
                        paren_depth += 1;
                    }
                    ';' => {
                        flush(&mut token, &mut board)?;
                        break;
                    }
                    c if c.is_whitespace() => {
                        flush(&mut token, &mut board)?;
                    }
                    c => token.push(c),
                }
            }
            flush(&mut token, &mut board)?;
        }
        Ok(board)
    }

    /// Returns O(1) incrementally tracked Opening / Middlegame / Endgame boundaries.
    pub fn divide_phases(&self) -> GameDivision {
        GameDivision {
            middlegame_start_ply: self.middlegame_start_ply,
            endgame_start_ply: self.endgame_start_ply,
        }
    }

    /// Returns a 64-element piece placement map (`a8..h1` in FEN rank-8-to-rank-1 order).
    pub fn board_squares_top_to_bottom(&self) -> [Option<(Side, PieceKind)>; 64] {
        let mut squares = [None; 64];
        let board = self.position.board();
        for rank_rev in 0..8usize {
            let rank = 7 - rank_rev;
            for file in 0..8usize {
                let sq = shakmaty::Square::from_coords(
                    shakmaty::File::new(file as u32),
                    shakmaty::Rank::new(rank as u32),
                );
                if let Some(piece) = board.piece_at(sq) {
                    squares[rank_rev * 8 + file] = Some((piece.color.into(), piece.role.into()));
                }
            }
        }
        squares
    }

    fn apply_shakmaty_move(&mut self, mv: Move) -> Result<AppliedMove, ChessError> {
        let side = self.turn();
        let captured_role = mv.capture();
        let is_capture = captured_role.is_some();
        let uci = mv.to_uci(CastlingMode::Standard).to_string();
        let san_plus = SanPlus::from_move_and_play_unchecked(&mut self.position, &mv);
        let san = san_plus.to_string();

        if let Some(role) = captured_role {
            let kind: PieceKind = role.into();
            let tray = match side {
                Side::White => &mut self.captured_by_white,
                Side::Black => &mut self.captured_by_black,
            };
            tray.push(kind);
            tray.sort_by_key(|k| std::cmp::Reverse(k.sort_value()));
        }

        self.uci_history.push(uci.clone());
        self.san_history.push(san.clone());
        self.record_repetition();
        self.update_phase_markers();

        let is_check = self.position.is_check();
        let status_after = self.status();

        Ok(AppliedMove {
            ply: self.ply_count(),
            side,
            uci,
            san,
            fen_after: self.to_fen(),
            is_check,
            is_capture,
            status_after,
        })
    }

    fn update_phase_markers(&mut self) {
        let ply = self.ply_count();
        let board = self.position.board();
        let non_pawn_king_cp = |color: Color| -> i32 {
            let p = board.by_color(color);
            (p & board.knights()).count() as i32 * 300
                + (p & board.bishops()).count() as i32 * 300
                + (p & board.rooks()).count() as i32 * 500
                + (p & board.queens()).count() as i32 * 900
        };
        let w_mat = non_pawn_king_cp(Color::White);
        let b_mat = non_pawn_king_cp(Color::Black);

        if self.middlegame_start_ply.is_none() && (ply >= 16 || (w_mat + b_mat) < 6000) {
            self.middlegame_start_ply = Some(ply);
        }
        if self.endgame_start_ply.is_none() && w_mat <= 1300 && b_mat <= 1300 {
            self.endgame_start_ply = Some(ply);
        }
    }

    fn repetition_key(&self) -> Zobrist64 {
        self.position.zobrist_hash::<Zobrist64>(EnPassantMode::Legal)
    }

    fn record_repetition(&mut self) {
        let key = self.repetition_key();
        *self.repetition_counts.entry(key).or_insert(0) += 1;
    }
}

/// Phase boundary plies computed by [`BoardState::divide_phases`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDivision {
    pub middlegame_start_ply: Option<u16>,
    pub endgame_start_ply: Option<u16>,
}

/// Parses a UCI move string (e.g. `e2e4` or `a7a8q`) into zero-indexed
/// `(from_file, from_rank)`, `(to_file, to_rank)`, and optional promotion piece char.
pub fn uci_to_coords(uci: &str) -> Option<((u8, u8), (u8, u8), Option<char>)> {
    let b = uci.trim().as_bytes();
    if b.len() < 4 || b.len() > 5 {
        return None;
    }
    let parse_sq = |f: u8, r: u8| -> Option<(u8, u8)> {
        if (b'a'..=b'h').contains(&f) && (b'1'..=b'8').contains(&r) {
            Some((f - b'a', r - b'1'))
        } else {
            None
        }
    };
    let from = parse_sq(b[0], b[1])?;
    let to = parse_sq(b[2], b[3])?;
    let promo = b.get(4).map(|&c| c as char);
    Some((from, to, promo))
}

/// Piece role independent of [`shakmaty`] internals for client rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceKind {
    Pawn,
    Knight,
    Bishop,
    Rook,
    Queen,
    King,
}

impl PieceKind {
    fn sort_value(self) -> u8 {
        match self {
            Self::Queen => 5,
            Self::Rook => 4,
            Self::Bishop => 3,
            Self::Knight => 2,
            Self::Pawn => 1,
            Self::King => 0,
        }
    }
}

impl From<Role> for PieceKind {
    fn from(role: Role) -> Self {
        match role {
            Role::Pawn => Self::Pawn,
            Role::Knight => Self::Knight,
            Role::Bishop => Self::Bishop,
            Role::Rook => Self::Rook,
            Role::Queen => Self::Queen,
            Role::King => Self::King,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fools_mate_is_checkmate() {
        let mut board = BoardState::new();
        board.make_uci_move("f2f3").unwrap();
        board.make_uci_move("e7e5").unwrap();
        board.make_uci_move("g2g4").unwrap();
        let last = board.make_uci_move("d8h4").unwrap();

        assert!(last.is_check);
        assert_eq!(
            last.status_after,
            GameStatus::Checkmate { winner: Side::Black }
        );
        assert_eq!(board.to_pgn_movetext(), "1. f3 e5 2. g4 Qh4#");
    }

    #[test]
    fn threefold_repetition_detected() {
        let mut board = BoardState::new();
        for _ in 0..2 {
            board.make_uci_move("g1f3").unwrap();
            board.make_uci_move("g8f6").unwrap();
            board.make_uci_move("f3g1").unwrap();
            board.make_uci_move("f6g8").unwrap();
        }
        assert_eq!(board.status(), GameStatus::ThreefoldRepetition);
    }

    #[test]
    fn en_passant_and_promotion_capture_tracked_accurately() {
        let mut board = BoardState::from_fen("4k3/1P6/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        let ep = board.make_uci_move("e5d6").unwrap();
        assert!(ep.is_capture);
        assert_eq!(board.captured_pieces().0, &[PieceKind::Pawn]);
        board.make_uci_move("e8d7").unwrap();
        let promo = board.make_uci_move("b7b8q").unwrap();
        assert!(promo.san.contains("=Q"));
        assert_eq!(board.material_balance_cp(), 1000);
    }

    #[test]
    fn pgn_with_comments_variations_and_nags_roundtrips() {
        let pgn = "[Event \"Test\"]\n\n1. e4 $1 {Best} c5 (1... e5 2. Nf3) 2. Nf3 d6 3. d4 cxd4 4. Nxd4 *";
        let board = BoardState::from_pgn_movetext(pgn).unwrap();
        assert_eq!(board.ply_count(), 7);
        assert_eq!(board.captured_pieces(), (&[PieceKind::Pawn][..], &[PieceKind::Pawn][..]));
        assert_eq!(board.opening().unwrap().eco, "B50");
        assert_eq!(uci_to_coords("e2e4"), Some(((4, 1), (4, 3), None)));
    }
}
