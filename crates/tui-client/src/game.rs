use std::collections::HashSet;

use chess_core::{BoardState, GameStatus, PieceKind, Side, uci_to_coords};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    BoardCursor,
    TextMove,
    Menu,
    PromotionChoice {
        from: (u8, u8),
        to: (u8, u8),
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Resume,
    NewGame,
    FlipBoard,
    ToggleAutoFlip,
    UndoMove,
    TextMove,
    Quit,
}

pub const MENU_ACTIONS: [MenuAction; 7] = [
    MenuAction::Resume,
    MenuAction::NewGame,
    MenuAction::FlipBoard,
    MenuAction::ToggleAutoFlip,
    MenuAction::UndoMove,
    MenuAction::TextMove,
    MenuAction::Quit,
];

/// Interactive two-player local (pass-and-play) terminal chess controller.
pub struct LocalGameApp {
    pub board: BoardState,
    history: Vec<(BoardState, Option<((u8, u8), (u8, u8))>)>,
    pub cursor: (u8, u8),
    pub hovered: Option<(u8, u8)>,
    pub selected: Option<(u8, u8)>,
    pub dragging_from: Option<(u8, u8)>,
    pub last_move: Option<((u8, u8), (u8, u8))>,
    pub flipped: bool,
    pub auto_flip: bool,
    pub mode: InputMode,
    pub menu_index: usize,
    pub should_quit: bool,
    pub text_buffer: String,
    pub status_message: String,
}

impl Default for LocalGameApp {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalGameApp {
    pub fn new() -> Self {
        Self {
            board: BoardState::new(),
            history: Vec::new(),
            cursor: (4, 1), // e2
            hovered: None,
            selected: None,
            dragging_from: None,
            last_move: None,
            flipped: false,
            auto_flip: false,
            mode: InputMode::BoardCursor,
            menu_index: 0,
            should_quit: false,
            text_buffer: String::new(),
            status_message: "Two-player local pass-and-play. Use Arrows + Enter or drag with mouse.".into(),
        }
    }

    pub fn open_menu(&mut self) {
        self.dragging_from = None;
        self.menu_index = 0;
        self.mode = InputMode::Menu;
        self.status_message = "Options menu opened.".into();
    }

    pub fn close_menu(&mut self) {
        self.mode = InputMode::BoardCursor;
        self.status_message = "Back to match.".into();
    }

    pub fn move_menu_selection(&mut self, delta: isize) {
        let len = MENU_ACTIONS.len() as isize;
        let next = (self.menu_index as isize + delta).rem_euclid(len);
        self.menu_index = next as usize;
    }

    pub fn execute_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::Resume => self.close_menu(),
            MenuAction::NewGame => self.reset(),
            MenuAction::FlipBoard => {
                self.flipped = !self.flipped;
                self.close_menu();
            }
            MenuAction::ToggleAutoFlip => {
                self.auto_flip = !self.auto_flip;
                self.status_message = if self.auto_flip {
                    "Auto-flip enabled (rotates each turn).".into()
                } else {
                    "Auto-flip disabled.".into()
                };
            }
            MenuAction::UndoMove => self.undo(),
            MenuAction::TextMove => {
                self.text_buffer.clear();
                self.mode = InputMode::TextMove;
            }
            MenuAction::Quit => {
                self.should_quit = true;
            }
        }
    }

    pub fn reset(&mut self) {
        let auto_flip = self.auto_flip;
        *self = Self::new();
        self.auto_flip = auto_flip;
        self.status_message = "Started a fresh local game.".into();
    }

    pub fn undo(&mut self) {
        if let Some((prev_board, prev_last_move)) = self.history.pop() {
            self.board = prev_board;
            self.last_move = prev_last_move;
            self.selected = None;
            self.dragging_from = None;
            self.mode = InputMode::BoardCursor;
            if self.auto_flip {
                self.flipped = self.board.turn() == Side::Black;
            }
            self.status_message = "Undid last move.".into();
        } else {
            self.status_message = "Nothing to undo.".into();
        }
    }

    pub fn on_mouse_move(&mut self, sq: Option<(u8, u8)>) {
        self.hovered = sq;
    }

    /// Returns exact `(captured_by_white, captured_by_black)` slices tracked by `chess-core`.
    pub fn captured_pieces(&self) -> (&[PieceKind], &[PieceKind]) {
        self.board.captured_pieces()
    }

    pub fn on_mouse_down(&mut self, sq: (u8, u8)) {
        if self.mode != InputMode::BoardCursor {
            return;
        }
        self.cursor = sq;

        if self.board.status().is_terminal() {
            self.status_message = "Game over — press 'r' for a new game or 'u' to undo.".into();
            return;
        }

        if let Some(from) = self.selected {
            if from == sq {
                self.selected = None;
                self.dragging_from = None;
                self.status_message = "Selection cleared.".into();
                return;
            }
            if self.try_move_or_promote(from, sq) {
                self.dragging_from = None;
                return;
            }
        }

        if self.square_has_legal_moves(sq) {
            self.selected = Some(sq);
            self.dragging_from = Some(sq);
            self.status_message = format!(
                "Selected {} — drag or click a highlighted square.",
                coord_to_alg(sq)
            );
        } else {
            self.selected = None;
            self.dragging_from = None;
            self.status_message = format!("No legal moves from {}.", coord_to_alg(sq));
        }
    }

    pub fn on_mouse_drag(&mut self, sq: (u8, u8)) {
        if self.mode != InputMode::BoardCursor || self.dragging_from.is_none() {
            return;
        }
        self.cursor = sq;
        if let Some(from) = self.dragging_from {
            self.status_message = if from == sq {
                format!("Dragging from {}...", coord_to_alg(from))
            } else {
                format!("Dragging {} -> {}", coord_to_alg(from), coord_to_alg(sq))
            };
        }
    }

    pub fn on_mouse_up(&mut self, sq: Option<(u8, u8)>) {
        let Some(from) = self.dragging_from.take() else {
            return;
        };
        let Some(target) = sq else {
            return;
        };
        self.cursor = target;
        if target != from {
            let _ = self.try_move_or_promote(from, target);
        }
    }

    fn square_has_legal_moves(&self, sq: (u8, u8)) -> bool {
        self.board
            .legal_uci_moves()
            .into_iter()
            .filter_map(|uci| uci_to_coords(&uci))
            .any(|(from, _, _)| from == sq)
    }

    fn try_move_or_promote(&mut self, from: (u8, u8), to: (u8, u8)) -> bool {
        let matching_ucis: Vec<String> = self
            .board
            .legal_uci_moves()
            .into_iter()
            .filter(|uci| {
                uci_to_coords(uci)
                    .is_some_and(|(f, t, _)| f == from && t == to)
            })
            .collect();

        if matching_ucis.is_empty() {
            return false;
        }

        if matching_ucis.iter().any(|u| u.len() == 5) {
            self.mode = InputMode::PromotionChoice { from, to };
            self.status_message =
                "Promote pawn: [q] Queen  [r] Rook  [b] Bishop  [n] Knight  [Esc] Cancel".into();
            return true;
        }

        self.try_apply_uci(&matching_ucis[0]);
        true
    }

    pub fn move_cursor(&mut self, dx: i8, dy: i8) {
        self.hovered = None;
        let (fx, fy) = if self.flipped { (-dx, -dy) } else { (dx, dy) };
        let nx = (self.cursor.0 as i8 + fx).clamp(0, 7) as u8;
        let ny = (self.cursor.1 as i8 + fy).clamp(0, 7) as u8;
        self.cursor = (nx, ny);
    }

    /// Returns all legal destination squares `(file, rank)` for the currently selected piece.
    pub fn legal_targets_for_selected(&self) -> HashSet<(u8, u8)> {
        let Some(sel) = self.selected else {
            return HashSet::new();
        };
        self.board
            .legal_uci_moves()
            .into_iter()
            .filter_map(|uci| uci_to_coords(&uci))
            .filter(|(from, _, _)| *from == sel)
            .map(|(_, to, _)| to)
            .collect()
    }

    /// Handles `Enter` / `Space` at the current cursor square.
    pub fn activate_cursor(&mut self) {
        if self.board.status().is_terminal() {
            self.status_message = "Game over — press 'r' for a new game or 'u' to undo.".into();
            return;
        }

        let sq = self.cursor;
        if let Some(from) = self.selected {
            if from == sq {
                self.selected = None;
                self.status_message = "Selection cleared.".into();
                return;
            }

            if self.try_move_or_promote(from, sq) {
                return;
            }
        }

        // Otherwise, select a piece belonging to the side to move if it has legal moves.
        if self.square_has_legal_moves(sq) {
            self.selected = Some(sq);
            self.status_message = format!(
                "Selected {} — choose a highlighted target square.",
                coord_to_alg(sq)
            );
        } else {
            self.selected = None;
            self.status_message = format!("No legal moves from {}.", coord_to_alg(sq));
        }
    }

    pub fn complete_promotion(&mut self, piece_char: char) {
        let InputMode::PromotionChoice { from, to } = self.mode else {
            return;
        };
        let uci = format!("{}{}{}", coord_to_alg(from), coord_to_alg(to), piece_char);
        self.mode = InputMode::BoardCursor;
        self.try_apply_uci(&uci);
    }

    pub fn submit_text_move(&mut self) {
        let raw = self.text_buffer.trim().to_string();
        self.text_buffer.clear();
        self.mode = InputMode::BoardCursor;
        if raw.is_empty() {
            return;
        }

        if self.board.status().is_terminal() {
            self.status_message = "Game over — press 'r' to start a new game.".into();
            return;
        }

        // Try UCI first, then SAN (e.g. Nf3, O-O, exd5).
        let prev = self.board.clone();
        let prev_last = self.last_move;
        let res = self
            .board
            .make_uci_move(&raw)
            .or_else(|_| self.board.make_san_move(&raw));

        match res {
            Ok(applied) => {
                self.history.push((prev, prev_last));
                self.last_move = uci_to_coords(&applied.uci).map(|(f, t, _)| (f, t));
                self.selected = None;
                if self.auto_flip {
                    self.flipped = self.board.turn() == Side::Black;
                }
                self.status_message = format!("Played {}.", applied.san);
            }
            Err(err) => {
                self.status_message = format!("Invalid move '{raw}': {err}");
            }
        }
    }

    fn try_apply_uci(&mut self, uci: &str) {
        let prev = self.board.clone();
        let prev_last = self.last_move;
        match self.board.make_uci_move(uci) {
            Ok(applied) => {
                self.history.push((prev, prev_last));
                self.last_move = uci_to_coords(&applied.uci).map(|(f, t, _)| (f, t));
                self.selected = None;
                if self.auto_flip {
                    self.flipped = self.board.turn() == Side::Black;
                }
                self.status_message = match applied.status_after {
                    GameStatus::Ongoing => {
                        if applied.is_check {
                            format!("{} — Check!", applied.san)
                        } else {
                            format!("Played {}.", applied.san)
                        }
                    }
                    terminal => format!("{} — {}", applied.san, terminal.display_label()),
                };
            }
            Err(err) => {
                self.status_message = format!("Move error: {err}");
            }
        }
    }
}

pub fn coord_to_alg((file, rank): (u8, u8)) -> String {
    format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_selection_and_undo_work() {
        let mut app = LocalGameApp::new();
        // Cursor starts at e2 (4, 1)
        app.activate_cursor();
        let targets = app.legal_targets_for_selected();
        assert!(targets.contains(&(4, 3))); // e4
        assert!(targets.contains(&(4, 2))); // e3

        app.move_cursor(0, 2); // e4
        app.activate_cursor();
        assert_eq!(app.board.turn(), Side::Black);
        assert_eq!(app.board.uci_history(), &["e2e4"]);

        app.undo();
        assert_eq!(app.board.turn(), Side::White);
        assert!(app.board.uci_history().is_empty());
    }

    #[test]
    fn san_text_move_works() {
        let mut app = LocalGameApp::new();
        app.text_buffer = "Nf3".into();
        app.submit_text_move();
        assert_eq!(app.board.san_history(), &["Nf3"]);
        assert_eq!(app.board.turn(), Side::Black);
    }

    #[test]
    fn mouse_click_and_drag_work() {
        let mut app = LocalGameApp::new();
        // Click-and-drag e2 -> e4
        app.on_mouse_down((4, 1));
        assert_eq!(app.selected, Some((4, 1)));
        app.on_mouse_drag((4, 3));
        assert_eq!(app.cursor, (4, 3));
        app.on_mouse_up(Some((4, 3)));
        assert_eq!(app.board.uci_history(), &["e2e4"]);
        assert_eq!(app.board.turn(), Side::Black);

        // Two-click move e7 -> e5
        app.on_mouse_down((4, 6));
        app.on_mouse_up(Some((4, 6)));
        assert_eq!(app.selected, Some((4, 6)));
        app.on_mouse_down((4, 4));
        app.on_mouse_up(Some((4, 4)));
        assert_eq!(app.board.uci_history(), &["e2e4", "e7e5"]);
    }

    #[test]
    fn menu_and_quit_actions_work() {
        let mut app = LocalGameApp::new();
        app.open_menu();
        assert_eq!(app.mode, InputMode::Menu);

        app.execute_menu_action(MenuAction::ToggleAutoFlip);
        assert!(app.auto_flip);

        app.execute_menu_action(MenuAction::Quit);
        assert!(app.should_quit);
    }
}
