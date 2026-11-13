//! True-color terminal chess client (`ratatui` + `crossterm`).
//!
//! Provides an interactive two-player local pass-and-play game.

mod assets;
mod game;
mod ui;

use std::io;

use assets::SvgPieceAtlas;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use game::{InputMode, LocalGameApp, MENU_ACTIONS};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
use ratatui_image::picker::{Picker, ProtocolType};

struct TerminalGuard {
    saved_font_size: Option<f32>,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Some(orig_pt) = self.saved_font_size {
            set_apple_terminal_font_size(orig_pt);
        }
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[cfg(target_os = "macos")]
fn auto_zoom_apple_terminal() -> Option<f32> {
    if std::env::var("TERM_PROGRAM").as_deref() != Ok("Apple_Terminal") {
        return None;
    }
    let (cols, rows) = crossterm::terminal::size().ok()?;
    let out = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"Terminal\" to get font size of selected tab of front window",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let orig_pt: f32 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    let scale = (cols as f32 / 32.0).min(rows as f32 / 17.0);
    if scale <= 1.15 {
        return None;
    }
    let target_pt = (orig_pt * scale).round().clamp(orig_pt, 40.0);
    if set_apple_terminal_font_size(target_pt) {
        Some(orig_pt)
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn auto_zoom_apple_terminal() -> Option<f32> {
    None
}

#[cfg(target_os = "macos")]
fn set_apple_terminal_font_size(pt: f32) -> bool {
    let script = format!(
        "tell application \"Terminal\" to set font size of selected tab of front window to {pt}"
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(not(target_os = "macos"))]
fn set_apple_terminal_font_size(_pt: f32) -> bool {
    false
}

fn detect_picker() -> Picker {
    let mut picker =
        Picker::from_query_stdio().unwrap_or_else(|_| Picker::from_fontsize((10, 20)));
    if picker.protocol_type() == ProtocolType::Halfblocks {
        let term = std::env::var("TERM").unwrap_or_default();
        let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();
        if std::env::var_os("KITTY_WINDOW_ID").is_some()
            || std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some()
            || term.contains("kitty")
            || term.contains("ghostty")
        {
            picker.set_protocol_type(ProtocolType::Kitty);
        } else if term_program.contains("iTerm")
            || term_program.contains("WezTerm")
            || term_program.contains("Warp")
        {
            picker.set_protocol_type(ProtocolType::Iterm2);
        }
    }
    picker
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut guard = TerminalGuard {
        saved_font_size: None,
    };

    let picker = detect_picker();
    let mut visuals = ui::BoardVisualState::new(SvgPieceAtlas::empty(), picker);
    if visuals.is_unicode_mode() {
        guard.saved_font_size = auto_zoom_apple_terminal();
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = LocalGameApp::new();
    let mut needs_draw = true;

    while !app.should_quit {
        if needs_draw {
            terminal.draw(|frame| ui::render(frame, &app, &mut visuals))?;
            needs_draw = false;
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            let mut latest_move: Option<event::MouseEvent> = None;
            loop {
                match event::read()? {
                    Event::Mouse(mouse)
                        if matches!(
                            mouse.kind,
                            MouseEventKind::Moved | MouseEventKind::Drag(MouseButton::Left)
                        ) =>
                    {
                        latest_move = Some(mouse);
                        needs_draw = true;
                    }
                    ev => {
                        if let Some(m) = latest_move.take() {
                            handle_event(
                                &mut app,
                                &terminal,
                                visuals.font_size(),
                                visuals.is_unicode_mode(),
                                Event::Mouse(m),
                            )?;
                        }
                        handle_event(
                            &mut app,
                            &terminal,
                            visuals.font_size(),
                            visuals.is_unicode_mode(),
                            ev,
                        )?;
                        needs_draw = true;
                        if app.should_quit {
                            break;
                        }
                    }
                }
                if !event::poll(std::time::Duration::ZERO)? {
                    break;
                }
            }
            if !app.should_quit && let Some(m) = latest_move {
                handle_event(
                    &mut app,
                    &terminal,
                    visuals.font_size(),
                    visuals.is_unicode_mode(),
                    Event::Mouse(m),
                )?;
                needs_draw = true;
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

fn handle_event(
    app: &mut LocalGameApp,
    terminal: &Terminal<CrosstermBackend<io::Stdout>>,
    font_size: (u16, u16),
    unicode_mode: bool,
    ev: Event,
) -> anyhow::Result<()> {
    match ev {
        Event::Resize(_, _) => {}
        Event::Mouse(mouse) => {
            let size = terminal.size()?;
            let area = Rect::new(0, 0, size.width, size.height);
            let layout =
                ui::compute_ui_layout_for_mode(area, app.flipped, font_size, unicode_mode);
            let col = mouse.column;
            let row = mouse.row;
            app.pointer_pos = Some((col, row));

            if app.mode == InputMode::Menu {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    if !ui::rect_contains(layout.menu_modal, col, row) {
                        app.close_menu();
                    } else {
                        for (idx, rect) in layout.menu_rows.iter().enumerate() {
                            if ui::rect_contains(*rect, col, row) {
                                app.execute_menu_action(MENU_ACTIONS[idx]);
                                break;
                            }
                        }
                    }
                } else if let MouseEventKind::Moved = mouse.kind {
                    for (idx, rect) in layout.menu_rows.iter().enumerate() {
                        if ui::rect_contains(*rect, col, row) {
                            app.menu_index = idx;
                            break;
                        }
                    }
                }
                return Ok(());
            }

            if let InputMode::PromotionChoice { .. } = app.mode {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    if ui::rect_contains(layout.promo_cancel, col, row) {
                        app.mode = InputMode::BoardCursor;
                        app.status_message = "Promotion cancelled.".into();
                    } else {
                        for (ch, rect) in layout.promo_buttons {
                            if ui::rect_contains(rect, col, row) {
                                app.complete_promotion(ch);
                                break;
                            }
                        }
                    }
                }
                return Ok(());
            }

            // Action pills on the telemetry rail
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                if ui::rect_contains(layout.btn_menu, col, row) {
                    app.open_menu();
                    return Ok(());
                } else if ui::rect_contains(layout.btn_undo, col, row) {
                    app.undo();
                    return Ok(());
                } else if ui::rect_contains(layout.btn_flip, col, row) {
                    app.flipped = !app.flipped;
                    return Ok(());
                } else if ui::rect_contains(layout.btn_quit, col, row) {
                    app.should_quit = true;
                    return Ok(());
                }
            }

            let sq = layout.board.hit_test(col, row);
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(target) = sq {
                        app.on_mouse_down(target);
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if let Some(target) = sq {
                        app.on_mouse_drag(target);
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    app.on_mouse_up(sq);
                }
                MouseEventKind::Moved => {
                    app.on_mouse_move(sq);
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    app.selected = None;
                    app.dragging_from = None;
                    app.status_message = "Selection cleared.".into();
                }
                _ => {}
            }
        }
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c' | 'C'))
            {
                app.should_quit = true;
                return Ok(());
            }

            match app.mode {
                InputMode::Menu => match key.code {
                    KeyCode::Esc => app.close_menu(),
                    KeyCode::Up => app.move_menu_selection(-1),
                    KeyCode::Down => app.move_menu_selection(1),
                    KeyCode::Enter => {
                        app.execute_menu_action(MENU_ACTIONS[app.menu_index]);
                    }
                    _ => {}
                },
                InputMode::TextMove => match key.code {
                    KeyCode::Esc => {
                        app.text_buffer.clear();
                        app.mode = InputMode::BoardCursor;
                    }
                    KeyCode::Enter => app.submit_text_move(),
                    KeyCode::Backspace => {
                        app.text_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        app.text_buffer.push(c);
                    }
                    _ => {}
                },
                InputMode::PromotionChoice { .. } => match key.code {
                    KeyCode::Esc => {
                        app.mode = InputMode::BoardCursor;
                        app.status_message = "Promotion cancelled.".into();
                    }
                    KeyCode::Char('q' | 'Q') => app.complete_promotion('q'),
                    KeyCode::Char('r' | 'R') => app.complete_promotion('r'),
                    KeyCode::Char('b' | 'B') => app.complete_promotion('b'),
                    KeyCode::Char('n' | 'N') => app.complete_promotion('n'),
                    _ => {}
                },
                InputMode::BoardCursor => match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Esc => {
                        if app.selected.is_some() {
                            app.selected = None;
                            app.status_message = "Selection cleared.".into();
                        } else {
                            app.open_menu();
                        }
                    }
                    KeyCode::Left => app.move_cursor(-1, 0),
                    KeyCode::Right => app.move_cursor(1, 0),
                    KeyCode::Up => app.move_cursor(0, 1),
                    KeyCode::Down => app.move_cursor(0, -1),
                    KeyCode::Enter => app.activate_cursor(),
                    KeyCode::Char(':') => {
                        app.text_buffer.clear();
                        app.mode = InputMode::TextMove;
                    }
                    KeyCode::Char('f') => {
                        app.flipped = !app.flipped;
                    }
                    KeyCode::Char('a') => app.execute_menu_action(game::MenuAction::ToggleAutoFlip),
                    KeyCode::Char('u') => app.undo(),
                    KeyCode::Char('r') => app.reset(),
                    _ => {}
                },
            }
        }
        _ => {}
    }
    Ok(())
}
