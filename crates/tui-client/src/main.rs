//! True-color terminal chess client (`ratatui` + `crossterm`).
//!
//! Provides an interactive two-player local pass-and-play game today, while
//! Task 5 wires in live WebSocket matchmaking, clocks, animations, and
//! Valorant-style Cohort/Nemesis overlays.

mod game;
mod ui;

use std::io;

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

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut app = LocalGameApp::new();

    while !app.should_quit {
        terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            match event::read()? {
                Event::Mouse(mouse) => {
                    let size = terminal.size()?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    let layout = ui::compute_ui_layout(area, app.flipped);
                    let col = mouse.column;
                    let row = mouse.row;

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
                        continue;
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
                        continue;
                    }

                    // Action pills on the telemetry rail
                    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        if ui::rect_contains(layout.btn_menu, col, row) {
                            app.open_menu();
                            continue;
                        } else if ui::rect_contains(layout.btn_undo, col, row) {
                            app.undo();
                            continue;
                        } else if ui::rect_contains(layout.btn_flip, col, row) {
                            app.flipped = !app.flipped;
                            continue;
                        } else if ui::rect_contains(layout.btn_quit, col, row) {
                            app.should_quit = true;
                            continue;
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
                        break;
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
                            KeyCode::Char('q') => break,
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
                            KeyCode::Char('a') => {
                                app.execute_menu_action(game::MenuAction::ToggleAutoFlip)
                            }
                            KeyCode::Char('u') => app.undo(),
                            KeyCode::Char('r') => app.reset(),
                            _ => {}
                        },
                    }
                }
                _ => {}
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}
