use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chess_core::{GameStatus, PieceKind, Side};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use ratatui_image::{
    FilterType, Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::{Protocol, kitty::Kitty},
};

use crate::assets::SvgPieceAtlas;
use crate::game::{InputMode, LocalGameApp, MENU_ACTIONS, MenuAction, coord_to_alg};

// Strictly Monochrome + Lilac HIG palette (no extra colors)
const BG_CANVAS: Color = Color::Rgb(9, 9, 11);
const BG_SURFACE: Color = Color::Rgb(17, 17, 20);
const BG_SURFACE_ACTIVE: Color = Color::Rgb(26, 26, 31);
const BG_PILL: Color = Color::Rgb(32, 32, 39);
const BG_PILL_HOVER: Color = Color::Rgb(88, 56, 134);
const BG_LILAC_SOFT: Color = Color::Rgb(44, 32, 62);
const FG_PRIMARY: Color = Color::Rgb(250, 250, 250);
const FG_MUTED: Color = Color::Rgb(161, 161, 170);
const FG_DIM: Color = Color::Rgb(82, 82, 91);
const ACCENT_LILAC: Color = Color::Rgb(192, 132, 252);

/// Holds the cached SVG piece atlas, detected terminal graphics protocol, and
/// latest encoded board image protocol without retaining raw source buffers.
pub struct BoardVisualState {
    atlas: SvgPieceAtlas,
    picker: Picker,
    protocol: Option<Protocol>,
    last_state_key: Option<u64>,
    next_kitty_id: u32,
    is_tmux: bool,
}

impl BoardVisualState {
    pub fn new(atlas: SvgPieceAtlas, picker: Picker) -> Self {
        let is_tmux = std::env::var("TERM").is_ok_and(|t| t.starts_with("tmux"))
            || std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "tmux");
        Self {
            atlas,
            picker,
            protocol: None,
            last_state_key: None,
            next_kitty_id: 1,
            is_tmux,
        }
    }

    pub fn font_size(&self) -> (u16, u16) {
        self.picker.font_size()
    }

    pub fn is_unicode_mode(&self) -> bool {
        self.picker.protocol_type() == ProtocolType::Halfblocks
    }

    pub fn protocol_label(&self) -> &'static str {
        match self.picker.protocol_type() {
            ProtocolType::Kitty => "KITTY PNG",
            ProtocolType::Sixel => "SIXEL PNG",
            ProtocolType::Iterm2 => "ITERM2 PNG",
            ProtocolType::Halfblocks => "UNICODE",
        }
    }

    fn alloc_kitty_id(&mut self) -> u32 {
        let id = self.next_kitty_id;
        self.next_kitty_id = self.next_kitty_id.wrapping_add(1).max(1);
        id
    }

    fn ensure_protocol(&mut self, app: &LocalGameApp, board: &BoardLayout, grid_rect: Rect) {
        let (fw, fh) = self.picker.font_size();
        let proto_type = self.picker.protocol_type();
        if proto_type == ProtocolType::Halfblocks {
            return;
        }

        let mut hasher = DefaultHasher::new();
        app.board.to_fen().hash(&mut hasher);
        app.selected.hash(&mut hasher);
        app.hovered.hash(&mut hasher);
        app.cursor.hash(&mut hasher);
        app.cursor_active.hash(&mut hasher);
        app.dragging_from.is_some().hash(&mut hasher);
        app.last_move.hash(&mut hasher);
        app.flipped.hash(&mut hasher);
        board.cell_w.hash(&mut hasher);
        board.cell_h.hash(&mut hasher);
        grid_rect.width.hash(&mut hasher);
        grid_rect.height.hash(&mut hasher);
        let key = hasher.finish();

        if self.last_state_key == Some(key) && self.protocol.is_some() {
            return;
        }

        let legal_targets = app.legal_targets_for_selected();
        let proto = match proto_type {
            ProtocolType::Kitty => {
                let tw = ((board.cell_w as u32) * (fw.max(1) as u32) * 2).clamp(96, 224);
                let th = ((board.cell_h as u32) * (fh.max(1) as u32) * 2).clamp(96, 224);
                if self.atlas.ensure_tile_size(tw, th).is_ok() {
                    let img = self.atlas.compose_board_image(app, &legal_targets);
                    let id = self.alloc_kitty_id();
                    Kitty::new(img, grid_rect, id, self.is_tmux)
                        .ok()
                        .map(Protocol::Kitty)
                } else {
                    None
                }
            }
            ProtocolType::Iterm2 | ProtocolType::Sixel => {
                let tw = (board.cell_w as u32) * (fw.max(1) as u32);
                let th = (board.cell_h as u32) * (fh.max(1) as u32);
                if self.atlas.ensure_tile_size(tw, th).is_ok() {
                    let img = self.atlas.compose_board_image(app, &legal_targets);
                    self.picker
                        .new_protocol(img, grid_rect, Resize::Fit(Some(FilterType::Lanczos3)))
                        .ok()
                } else {
                    None
                }
            }
            ProtocolType::Halfblocks => None,
        };

        if let Some(p) = proto {
            self.protocol = Some(p);
            self.last_state_key = Some(key);
        }
    }
}

pub fn render(frame: &mut Frame, app: &LocalGameApp, visuals: &mut BoardVisualState) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG_CANVAS)), area);

    let layout = compute_ui_layout_for_mode(
        area,
        app.flipped,
        visuals.font_size(),
        visuals.is_unicode_mode(),
    );
    render_arena(frame, app, &layout.board, visuals);
    if layout.rail_area.width > 0 {
        render_telemetry_rail(frame, app, &layout, visuals.protocol_label());
        render_command_bar(frame, app, layout.command_bar);
    } else {
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Rgb(14, 14, 20))),
            layout.command_bar,
        );
        render_action_pills(frame, app, &layout);
        let shortcut_row = Rect::new(
            layout.command_bar.x,
            layout.command_bar.y + 1,
            layout.command_bar.width,
            1,
        );
        render_command_bar(frame, app, shortcut_row);
    }

    match app.mode {
        InputMode::Menu => render_menu_modal(frame, app, &layout),
        InputMode::PromotionChoice { .. } => render_promotion_modal(frame, app, &layout),
        _ => {}
    }

    if app.mode == InputMode::TextMove && area.width > 0 && area.height > 0 {
        let cmd_y = if layout.rail_area.width > 0 {
            layout.command_bar.y
        } else {
            layout.command_bar.y + 1
        };
        let cx = (layout.command_bar.x + 4 + app.text_buffer.chars().count() as u16)
            .min(layout.command_bar.right().saturating_sub(1));
        frame.set_cursor_position(Position::new(cx, cmd_y));
    }
}

fn is_hovered(app: &LocalGameApp, rect: Rect) -> bool {
    app.pointer_pos
        .is_some_and(|(col, row)| rect_contains(rect, col, row))
}

#[derive(Clone, Copy)]
struct SquareScale {
    cell_w: usize,
    cell_h: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BoardLayout {
    pub grid_x: u16,
    pub grid_y: u16,
    pub cell_w: u16,
    pub cell_h: u16,
    pub top_bar: Rect,
    pub bottom_bar: Rect,
    pub file_labels_y: u16,
    pub flipped: bool,
}

impl BoardLayout {
    /// Maps a terminal `(column, row)` coordinate to a chess square `(file, rank)`.
    pub fn hit_test(&self, col: u16, row: u16) -> Option<(u8, u8)> {
        let board_w = 8 * self.cell_w;
        let board_h = 8 * self.cell_h;
        if col < self.grid_x
            || row < self.grid_y
            || col >= self.grid_x + board_w
            || row >= self.grid_y + board_h
        {
            return None;
        }
        let display_col = ((col - self.grid_x) / self.cell_w) as u8;
        let display_row = ((row - self.grid_y) / self.cell_h) as u8;
        let file = if self.flipped {
            7 - display_col
        } else {
            display_col
        };
        let rank = if self.flipped {
            display_row
        } else {
            7 - display_row
        };
        Some((file, rank))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UiLayout {
    pub board: BoardLayout,
    pub rail_area: Rect,
    pub command_bar: Rect,
    pub btn_menu: Rect,
    pub btn_undo: Rect,
    pub btn_flip: Rect,
    pub btn_quit: Rect,
    pub menu_modal: Rect,
    pub menu_rows: [Rect; 7],
    pub promo_modal: Rect,
    pub promo_buttons: [(char, Rect); 4],
    pub promo_cancel: Rect,
}

pub fn rect_contains(r: Rect, col: u16, row: u16) -> bool {
    col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height
}

#[cfg(test)]
pub fn compute_ui_layout(terminal_area: Rect, flipped: bool) -> UiLayout {
    compute_ui_layout_for_mode(terminal_area, flipped, (10, 20), false)
}

pub fn compute_ui_layout_for_mode(
    terminal_area: Rect,
    flipped: bool,
    font_size: (u16, u16),
    unicode_mode: bool,
) -> UiLayout {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(12), Constraint::Length(2)])
        .split(terminal_area);

    let (arena, rail_area) = if unicode_mode {
        (root[0], Rect::ZERO)
    } else {
        let main_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(44), Constraint::Length(34)])
            .split(root[0]);
        (main_cols[0], main_cols[1])
    };

    let avail_w = arena.width.saturating_sub(4) as usize;
    let avail_h = arena.height.saturating_sub(5) as usize;

    let fw = font_size.0.max(1) as f32;
    let fh = font_size.1.max(1) as f32;

    let mut scale = SquareScale { cell_w: 3, cell_h: 1 };
    let max_ch = if unicode_mode {
        2
    } else {
        (avail_h / 8).clamp(1, 16)
    };
    for ch in (1..=max_ch).rev() {
        let cw = ((ch as f32 * fh) / fw).round().max(3.0) as usize;
        if avail_w >= 8 * cw && avail_h >= 8 * ch {
            scale = SquareScale {
                cell_w: cw,
                cell_h: ch,
            };
            break;
        }
    }

    let board_w = (8 * scale.cell_w) as u16;
    let block_w = 3 + board_w;
    let board_h = (8 * scale.cell_h) as u16;
    let total_h = 2 + board_h + 3;

    let origin_x = arena.x + arena.width.saturating_sub(block_w) / 2;
    let origin_y = arena.y + arena.height.saturating_sub(total_h) / 2;

    let top_bar = Rect::new(origin_x + 3, origin_y, board_w, 1);
    let grid_x = origin_x + 3;
    let grid_y = origin_y + 2;
    let file_labels_y = grid_y + board_h;
    let bottom_bar = Rect::new(origin_x + 3, file_labels_y + 2, board_w, 1);

    let board = BoardLayout {
        grid_x,
        grid_y,
        cell_w: scale.cell_w as u16,
        cell_h: scale.cell_h as u16,
        top_bar,
        bottom_bar,
        file_labels_y,
        flipped,
    };

    let (btn_menu, btn_undo, btn_flip, btn_quit) = if unicode_mode {
        let btn_y = root[1].y;
        let total_btn_w = 27u16;
        let bx = root[1].x + root[1].width.saturating_sub(total_btn_w) / 2;
        let right_edge = root[1].x + root[1].width;
        let clamped_rect = |x: u16, w: u16| -> Rect {
            let avail = right_edge.saturating_sub(x);
            Rect::new(x, btn_y, w.min(avail), 1)
        };
        (
            clamped_rect(bx, 6),
            clamped_rect(bx + 7, 6),
            clamped_rect(bx + 14, 6),
            clamped_rect(bx + 21, 6),
        )
    } else {
        let rail_inner = Block::default()
            .borders(Borders::LEFT)
            .inner(rail_area);
        let btn_y = rail_inner.y + 3;
        let bx = rail_inner.x + 2;
        let right_edge = rail_area.x + rail_area.width;
        let clamped_rect = |x: u16, w: u16| -> Rect {
            let avail = right_edge.saturating_sub(x);
            Rect::new(x, btn_y, w.min(avail), 1)
        };
        (
            clamped_rect(bx, 6),
            clamped_rect(bx + 7, 6),
            clamped_rect(bx + 14, 6),
            clamped_rect(bx + 21, 6),
        )
    };

    // Centered Options Menu modal
    let mw = 44u16.min(terminal_area.width.saturating_sub(4));
    let mh = 11u16.min(terminal_area.height.saturating_sub(2));
    let menu_modal = Rect::new(
        terminal_area.x + terminal_area.width.saturating_sub(mw) / 2,
        terminal_area.y + terminal_area.height.saturating_sub(mh) / 2,
        mw,
        mh,
    );
    let mut menu_rows = [Rect::default(); 7];
    for (i, slot) in menu_rows.iter_mut().enumerate() {
        *slot = Rect::new(menu_modal.x + 2, menu_modal.y + 2 + i as u16, mw.saturating_sub(4), 1);
    }

    // Centered Promotion modal
    let pw = 46u16.min(terminal_area.width.saturating_sub(4));
    let ph = 5u16;
    let promo_modal = Rect::new(
        terminal_area.x + terminal_area.width.saturating_sub(pw) / 2,
        terminal_area.y + terminal_area.height.saturating_sub(ph) / 2,
        pw,
        ph,
    );
    let py = promo_modal.y + 2;
    let px = promo_modal.x + 2;
    let promo_buttons = [
        ('q', Rect::new(px, py, 7, 1)),
        ('r', Rect::new(px + 8, py, 7, 1)),
        ('b', Rect::new(px + 16, py, 7, 1)),
        ('n', Rect::new(px + 24, py, 7, 1)),
    ];
    let promo_cancel = Rect::new(px + 32, py, 10, 1);

    UiLayout {
        board,
        rail_area,
        command_bar: root[1],
        btn_menu,
        btn_undo,
        btn_flip,
        btn_quit,
        menu_modal,
        menu_rows,
        promo_modal,
        promo_buttons,
        promo_cancel,
    }
}

fn render_arena(
    frame: &mut Frame,
    app: &LocalGameApp,
    layout: &BoardLayout,
    visuals: &mut BoardVisualState,
) {
    let (by_white, by_black) = app.captured_pieces();
    let mat_cp = app.board.material_balance_cp();

    let (top_side, bottom_side) = if app.flipped {
        (Side::White, Side::Black)
    } else {
        (Side::Black, Side::White)
    };

    render_player_bar(
        frame,
        layout.top_bar,
        top_side,
        app.board.turn() == top_side && !app.board.status().is_terminal(),
        if top_side == Side::White { by_white } else { by_black },
        if top_side == Side::White { mat_cp } else { -mat_cp },
    );
    render_player_bar(
        frame,
        layout.bottom_bar,
        bottom_side,
        app.board.turn() == bottom_side && !app.board.status().is_terminal(),
        if bottom_side == Side::White { by_white } else { by_black },
        if bottom_side == Side::White { mat_cp } else { -mat_cp },
    );

    let cell_w = layout.cell_w as usize;
    let cell_h = layout.cell_h as usize;
    let mid_row = cell_h / 2;

    let mut rank_lines = Vec::with_capacity(8 * cell_h);
    for display_row in 0..8usize {
        let rank = if app.flipped {
            display_row as u8
        } else {
            (7 - display_row) as u8
        };

        for sub_y in 0..cell_h {
            let label = if sub_y == mid_row {
                format!(" {} ", rank + 1)
            } else {
                "   ".to_string()
            };
            rank_lines.push(Line::from(Span::styled(
                label,
                Style::default().fg(FG_DIM).bg(BG_CANVAS),
            )));
        }
    }
    let rank_rect = Rect::new(
        layout.grid_x.saturating_sub(3),
        layout.grid_y,
        3,
        8 * layout.cell_h,
    );
    frame.render_widget(Paragraph::new(rank_lines), rank_rect);

    let grid_rect = Rect::new(
        layout.grid_x,
        layout.grid_y,
        8 * layout.cell_w,
        8 * layout.cell_h,
    );
    if visuals.is_unicode_mode() {
        render_unicode_board(frame, app, layout, grid_rect);
    } else {
        visuals.ensure_protocol(app, layout, grid_rect);
        if let Some(protocol) = visuals.protocol.as_ref() {
            frame.render_widget(Image::new(protocol), grid_rect);
        }
    }

    let mut file_line = String::new();
    for display_col in 0..8usize {
        let file = if app.flipped {
            (7 - display_col) as u8
        } else {
            display_col as u8
        };
        let label = format!("{}", (b'a' + file) as char);
        file_line.push_str(&center_in_cell(&label, cell_w));
    }
    let file_rect = Rect::new(layout.grid_x, layout.file_labels_y, 8 * layout.cell_w, 1);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            file_line,
            Style::default().fg(FG_DIM).bg(BG_CANVAS),
        ))),
        file_rect,
    );
}

fn render_unicode_board(
    frame: &mut Frame,
    app: &LocalGameApp,
    layout: &BoardLayout,
    grid_rect: Rect,
) {
    const SQ_LIGHT: Color = Color::Rgb(166, 166, 178);
    const SQ_DARK: Color = Color::Rgb(74, 74, 86);
    const SQ_LAST_LIGHT: Color = Color::Rgb(156, 142, 186);
    const SQ_LAST_DARK: Color = Color::Rgb(90, 76, 120);
    const SQ_HOVER_LIGHT: Color = Color::Rgb(178, 160, 214);
    const SQ_HOVER_DARK: Color = Color::Rgb(96, 82, 132);
    const SQ_SELECTED: Color = Color::Rgb(147, 51, 234);
    const SQ_CAPTURE: Color = Color::Rgb(126, 68, 184);
    const SQ_CHECK: Color = Color::Rgb(168, 85, 247);
    const SQ_CURSOR: Color = Color::Rgb(192, 132, 252);
    const PIECE_WHITE: Color = Color::Rgb(252, 252, 255);
    const PIECE_BLACK: Color = Color::Rgb(9, 9, 12);

    let cell_w = layout.cell_w as usize;
    let cell_h = layout.cell_h as usize;
    let mid_row = cell_h / 2;
    let squares = app.board.board_squares_top_to_bottom();
    let legal_targets = app.legal_targets_for_selected();
    let king_in_check = app.board.is_check();
    let turn = app.board.turn();

    let mut lines = Vec::with_capacity(8 * cell_h);
    for display_row in 0..8usize {
        let rank = if app.flipped {
            display_row as u8
        } else {
            (7 - display_row) as u8
        };

        for sub_y in 0..cell_h {
            let mut spans = Vec::with_capacity(8);
            for display_col in 0..8usize {
                let file = if app.flipped {
                    (7 - display_col) as u8
                } else {
                    display_col as u8
                };
                let coord = (file, rank);
                let sq_idx = (7 - rank as usize) * 8 + file as usize;
                let piece = squares[sq_idx];
                let is_light = (file + rank) % 2 == 1;

                let mut bg = if is_light { SQ_LIGHT } else { SQ_DARK };
                if let Some((from, to)) = app.last_move
                    && (coord == from || coord == to)
                {
                    bg = if is_light { SQ_LAST_LIGHT } else { SQ_LAST_DARK };
                }
                if app.hovered == Some(coord) {
                    bg = if is_light { SQ_HOVER_LIGHT } else { SQ_HOVER_DARK };
                }
                if king_in_check && piece == Some((turn, PieceKind::King)) {
                    bg = SQ_CHECK;
                }
                let is_legal = legal_targets.contains(&coord);
                if app.selected == Some(coord) {
                    bg = SQ_SELECTED;
                } else if is_legal && piece.is_some() {
                    bg = SQ_CAPTURE;
                }
                if (app.dragging_from.is_some() || app.cursor_active) && app.cursor == coord {
                    bg = SQ_CURSOR;
                }

                let (text, fg) = match piece {
                    Some((side, kind)) if sub_y == mid_row => (
                        center_in_cell(solid_piece_glyph(kind), cell_w),
                        match side {
                            Side::White => PIECE_WHITE,
                            Side::Black => PIECE_BLACK,
                        },
                    ),
                    None if is_legal && sub_y == mid_row => {
                        (center_in_cell("●", cell_w), ACCENT_LILAC)
                    }
                    _ => (" ".repeat(cell_w), Color::Reset),
                };

                spans.push(Span::styled(
                    text,
                    Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(spans));
        }
    }

    frame.render_widget(Paragraph::new(lines), grid_rect);
}

fn render_player_bar(
    frame: &mut Frame,
    rect: Rect,
    side: Side,
    is_active: bool,
    captured: &[PieceKind],
    adv_cp: i32,
) {
    let (dot, name) = match side {
        Side::White => ("●", "WHITE"),
        Side::Black => ("○", "BLACK"),
    };

    let mut cap_str = String::new();
    for &kind in captured {
        cap_str.push_str(solid_piece_glyph(kind));
        cap_str.push(' ');
    }
    if adv_cp > 0 {
        cap_str.push_str(&format!("+{}", adv_cp / 100));
    }

    let bar_bg = if is_active {
        BG_SURFACE_ACTIVE
    } else {
        BG_SURFACE
    };
    let indicator_color = if is_active { ACCENT_LILAC } else { FG_DIM };

    let mut spans = vec![
        Span::styled(
            format!(" {dot} "),
            Style::default().fg(indicator_color).bg(bar_bg),
        ),
        Span::styled(
            name,
            Style::default()
                .fg(if is_active { FG_PRIMARY } else { FG_MUTED })
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("   ", Style::default().bg(bar_bg)),
        Span::styled(cap_str, Style::default().fg(ACCENT_LILAC).bg(bar_bg)),
    ];

    if is_active {
        spans.push(Span::styled(
            "   YOUR MOVE",
            Style::default()
                .fg(ACCENT_LILAC)
                .bg(bar_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = (rect.width as usize).saturating_sub(used);
    if pad > 0 {
        spans.push(Span::styled(" ".repeat(pad), Style::default().bg(bar_bg)));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), rect);
}

fn render_telemetry_rail(
    frame: &mut Frame,
    app: &LocalGameApp,
    layout: &UiLayout,
    protocol_label: &str,
) {
    let rail_block = Block::default()
        .style(Style::default().bg(BG_SURFACE))
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(Color::Rgb(34, 32, 46)));
    let inner = rail_block.inner(layout.rail_area);
    frame.render_widget(rail_block, layout.rail_area);

    let padded = Rect {
        x: inner.x + 2,
        y: inner.y + 1,
        width: inner.width.saturating_sub(4),
        height: inner.height.saturating_sub(2),
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Title + clickable action bar
            Constraint::Length(5), // Status + Opening (allows 2-line wrap)
            Constraint::Length(3), // Phase & Material Balance
            Constraint::Min(6),    // Two-column tournament scoresheet
        ])
        .split(padded);

    // 1. Brand header
    let header = vec![
        Line::from(vec![
            Span::styled(
                "L I L A C",
                Style::default()
                    .fg(ACCENT_LILAC)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("   {protocol_label}"), Style::default().fg(FG_DIM)),
        ]),
        Line::from(Span::styled(
            "────────────────────────",
            Style::default().fg(Color::Rgb(36, 34, 48)),
        )),
    ];
    frame.render_widget(Paragraph::new(header), sections[0]);

    // Clickable pill buttons at exact hit-test rects with clear hover background
    render_action_pills(frame, app, layout);

    // 2. Outcome / Check & Opening
    let status_span = match app.board.status() {
        GameStatus::Ongoing => {
            if app.board.is_check() {
                Span::styled(
                    "CHECK",
                    Style::default()
                        .fg(ACCENT_LILAC)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                let who = match app.board.turn() {
                    Side::White => "White to move",
                    Side::Black => "Black to move",
                };
                Span::styled(who, Style::default().fg(FG_PRIMARY))
            }
        }
        terminal => Span::styled(
            terminal.display_label().to_uppercase(),
            Style::default()
                .fg(ACCENT_LILAC)
                .add_modifier(Modifier::BOLD),
        ),
    };

    let opening_line = app
        .board
        .opening()
        .map(|o| format!("{}  {}", o.eco, o.name))
        .unwrap_or_else(|| "Standard Position".into());

    let meta = vec![
        Line::from(status_span),
        Line::from(""),
        Line::from(vec![
            Span::styled("OPENING  ", Style::default().fg(FG_DIM)),
            Span::styled(opening_line, Style::default().fg(FG_PRIMARY)),
        ]),
    ];
    frame.render_widget(Paragraph::new(meta).wrap(Wrap { trim: true }), sections[1]);

    // 3. Phase + Material balance bar
    let div = app.board.divide_phases();
    let (phase_label, phase_bar) = if div.endgame_start_ply.is_some() {
        ("ENDGAME   ", "━━━━━━━━━●")
    } else if div.middlegame_start_ply.is_some() {
        ("MIDDLEGAME", "━━━━●─────")
    } else {
        ("PHASE     ", "●─────────")
    };
    let mat_cp = app.board.material_balance_cp();
    let mat_str = if mat_cp > 0 {
        format!("W +{:.1}", mat_cp as f32 / 100.0)
    } else if mat_cp < 0 {
        format!("B +{:.1}", (-mat_cp) as f32 / 100.0)
    } else {
        "0.0".into()
    };

    let telemetry = vec![
        Line::from(vec![
            Span::styled(format!("{phase_label}  "), Style::default().fg(FG_DIM)),
            Span::styled(phase_bar, Style::default().fg(ACCENT_LILAC)),
        ]),
        Line::from(vec![
            Span::styled("MATERIAL    ", Style::default().fg(FG_DIM)),
            Span::styled(mat_str, Style::default().fg(ACCENT_LILAC)),
            Span::styled(
                format!("   SQ {}", coord_to_alg(app.hovered.unwrap_or(app.cursor))),
                Style::default().fg(FG_MUTED),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(telemetry), sections[2]);

    // 4. Two-column Tournament Scoresheet
    let sans = app.board.san_history();
    let max_rows = sections[3].height.saturating_sub(2) as usize;
    let total_full_moves = sans.len().div_ceil(2);
    let skip_moves = total_full_moves.saturating_sub(max_rows);

    let mut score_lines = vec![Line::from(Span::styled(
        "SCORESHEET",
        Style::default().fg(FG_DIM).add_modifier(Modifier::BOLD),
    ))];

    if sans.is_empty() {
        score_lines.push(Line::from(Span::styled(
            "  Awaiting 1. ...",
            Style::default().fg(FG_MUTED),
        )));
    } else {
        for move_idx in skip_moves..total_full_moves {
            let w_ply = move_idx * 2;
            let b_ply = w_ply + 1;
            let w_san = sans.get(w_ply).map(String::as_str).unwrap_or("");
            let b_san = sans.get(b_ply).map(String::as_str).unwrap_or("");
            let w_is_last = w_ply + 1 == sans.len();
            let b_is_last = b_ply + 1 == sans.len();

            score_lines.push(Line::from(vec![
                Span::styled(
                    format!("{:>3}. ", move_idx + 1),
                    Style::default().fg(FG_DIM),
                ),
                Span::styled(
                    format!("{w_san:<8}"),
                    Style::default().fg(if w_is_last { ACCENT_LILAC } else { FG_PRIMARY }),
                ),
                Span::styled(
                    format!("{b_san:<8}"),
                    Style::default().fg(if b_is_last { ACCENT_LILAC } else { FG_MUTED }),
                ),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(score_lines), sections[3]);
}

fn render_action_pills(frame: &mut Frame, app: &LocalGameApp, layout: &UiLayout) {
    let (menu_fg, menu_bg) = if is_hovered(app, layout.btn_menu) {
        (BG_CANVAS, ACCENT_LILAC)
    } else {
        (ACCENT_LILAC, BG_LILAC_SOFT)
    };
    let (undo_fg, undo_bg) = if is_hovered(app, layout.btn_undo) {
        (FG_PRIMARY, BG_PILL_HOVER)
    } else {
        (FG_PRIMARY, BG_PILL)
    };
    let (flip_fg, flip_bg) = if is_hovered(app, layout.btn_flip) {
        (FG_PRIMARY, BG_PILL_HOVER)
    } else {
        (FG_PRIMARY, BG_PILL)
    };
    let (quit_fg, quit_bg) = if is_hovered(app, layout.btn_quit) {
        (FG_PRIMARY, BG_PILL_HOVER)
    } else {
        (FG_MUTED, BG_PILL)
    };
    render_pill(frame, layout.btn_menu, " Menu ", menu_fg, menu_bg);
    render_pill(frame, layout.btn_undo, " Undo ", undo_fg, undo_bg);
    render_pill(frame, layout.btn_flip, " Flip ", flip_fg, flip_bg);
    render_pill(frame, layout.btn_quit, " Quit ", quit_fg, quit_bg);
}

fn render_pill(frame: &mut Frame, rect: Rect, label: &str, fg: Color, bg: Color) {
    if rect.width == 0 || rect.height == 0 {
        return;
    }
    let pill = Paragraph::new(Line::from(Span::styled(
        label,
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(pill, rect);
}

fn render_command_bar(frame: &mut Frame, app: &LocalGameApp, area: Rect) {
    let line = match app.mode {
        InputMode::TextMove => Line::from(vec![
            Span::styled("  : ", Style::default().fg(ACCENT_LILAC).add_modifier(Modifier::BOLD)),
            Span::styled(&app.text_buffer, Style::default().fg(FG_PRIMARY)),
            Span::styled("▌", Style::default().fg(ACCENT_LILAC)),
        ]),
        _ if area.width < 54 => Line::from(Span::styled(
            " [Esc] [:] [f] [u] [r] [q]",
            Style::default().fg(FG_MUTED),
        )),
        _ if area.width < 76 => Line::from(Span::styled(
            "  [Esc] menu  [:] move  [f] flip  [u] undo  [r] reset  [q] quit",
            Style::default().fg(FG_MUTED),
        )),
        _ => Line::from(vec![
            Span::styled(
                format!("  {}   ", app.status_message),
                Style::default().fg(FG_MUTED),
            ),
            Span::styled(
                "[Esc] menu  [:] move  [f] flip  [a] auto  [u] undo  [r] reset  [q] quit",
                Style::default().fg(FG_DIM),
            ),
        ]),
    };

    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(14, 14, 20)));
    frame.render_widget(bar, area);
}

fn render_menu_modal(frame: &mut Frame, app: &LocalGameApp, layout: &UiLayout) {
    frame.render_widget(Clear, layout.menu_modal);
    let block = Block::default()
        .title(" LILAC // OPTIONS ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_LILAC))
        .style(Style::default().bg(BG_SURFACE_ACTIVE));
    frame.render_widget(block, layout.menu_modal);

    for (idx, action) in MENU_ACTIONS.iter().enumerate() {
        let is_selected = idx == app.menu_index;
        let label = match action {
            MenuAction::Resume => "  Resume Match                   [Esc]",
            MenuAction::NewGame => "  New Match (Reset Board)          [r]",
            MenuAction::FlipBoard => "  Flip Board                       [f]",
            MenuAction::ToggleAutoFlip => {
                if app.auto_flip {
                    "  Auto-Flip Each Turn: ON          [a]"
                } else {
                    "  Auto-Flip Each Turn: OFF         [a]"
                }
            }
            MenuAction::UndoMove => "  Undo Last Move                   [u]",
            MenuAction::TextMove => "  Type Move (SAN / UCI)            [:]",
            MenuAction::Quit => "  Quit Lilac                       [q]",
        };

        let (fg, bg) = if is_selected {
            (ACCENT_LILAC, BG_LILAC_SOFT)
        } else if *action == MenuAction::Quit {
            (FG_MUTED, BG_SURFACE_ACTIVE)
        } else {
            (FG_PRIMARY, BG_SURFACE_ACTIVE)
        };

        let row = Paragraph::new(Line::from(Span::styled(
            label,
            Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
        )));
        frame.render_widget(row, layout.menu_rows[idx]);
    }
}

fn render_promotion_modal(frame: &mut Frame, app: &LocalGameApp, layout: &UiLayout) {
    frame.render_widget(Clear, layout.promo_modal);
    let block = Block::default()
        .title(" Promote Pawn ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_LILAC))
        .style(Style::default().bg(BG_SURFACE_ACTIVE));
    frame.render_widget(block, layout.promo_modal);

    for (ch, rect) in layout.promo_buttons {
        let label = match ch {
            'q' => " [Q] ♛ ",
            'r' => " [R] ♜ ",
            'b' => " [B] ♝ ",
            _ => " [N] ♞ ",
        };
        let (fg, bg) = if is_hovered(app, rect) {
            (BG_CANVAS, ACCENT_LILAC)
        } else {
            (FG_PRIMARY, BG_PILL)
        };
        render_pill(frame, rect, label, fg, bg);
    }
    let (cancel_fg, cancel_bg) = if is_hovered(app, layout.promo_cancel) {
        (FG_PRIMARY, BG_PILL_HOVER)
    } else {
        (FG_MUTED, BG_PILL)
    };
    render_pill(frame, layout.promo_cancel, " [Esc] X ", cancel_fg, cancel_bg);
}

fn center_in_cell(glyph: &str, cell_w: usize) -> String {
    let glyph_chars = glyph.chars().count();
    let total_pad = cell_w.saturating_sub(glyph_chars);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    format!("{}{}{}", " ".repeat(left_pad), glyph, " ".repeat(right_pad))
}

fn solid_piece_glyph(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::King => "♚",
        PieceKind::Queen => "♛",
        PieceKind::Rook => "♜",
        PieceKind::Bishop => "♝",
        PieceKind::Knight => "♞",
        PieceKind::Pawn => "♟",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_layout_hit_test_maps_corners_accurately() {
        let area = Rect::new(0, 0, 120, 40);
        let normal = compute_ui_layout(area, false).board;
        assert_eq!(normal.hit_test(normal.grid_x, normal.grid_y), Some((0, 7))); // a8
        assert_eq!(
            normal.hit_test(
                normal.grid_x + 7 * normal.cell_w,
                normal.grid_y + 7 * normal.cell_h
            ),
            Some((7, 0)) // h1
        );

        let flipped = compute_ui_layout(area, true).board;
        assert_eq!(flipped.hit_test(flipped.grid_x, flipped.grid_y), Some((7, 0))); // h1 at top-left
    }
}
