use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::HeaderMap,
    response::Response,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use lilac_proto::{ClientMsg, ServerMsg};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{app::AppState, auth::verify_jwt, ws_hub::WS_OUTBOUND_BUFFER};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    // Only accept Authorization header pre-auth; browser clients authenticate post-connect
    // via `ClientMsg::Authenticate` so JWTs never leak into URL access logs.
    let pre_auth = extract_ws_bearer(&headers)
        .and_then(|tok| verify_jwt(&tok, &state.config.jwt_secret).ok())
        .map(|claims| (claims.sub, claims.username));

    ws.on_upgrade(move |socket| handle_socket(socket, state, pre_auth))
}

fn extract_ws_bearer(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    raw.strip_prefix("Bearer ")
        .or_else(|| raw.strip_prefix("bearer "))
        .map(|s| s.trim().to_string())
}

async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    mut session_user: Option<(Uuid, String)>,
) {
    let conn_id = Uuid::new_v4();
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMsg>(WS_OUTBOUND_BUFFER);

    if let Some((user_id, ref username)) = session_user {
        state.ws_hub.register(user_id, conn_id, out_tx.clone()).await;
        let _ = out_tx.try_send(ServerMsg::Authenticated {
            user_id,
            username: username.clone(),
        });
    }

    let writer_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            let payload = serde_json::to_string(&msg).unwrap_or_else(|_| "{}".into());
            if ws_sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(frame_res) = ws_receiver.next().await {
        let Ok(frame) = frame_res else {
            break;
        };

        match frame {
            Message::Text(text) => {
                let Ok(client_msg) = serde_json::from_str::<ClientMsg>(&text) else {
                    if out_tx
                        .try_send(ServerMsg::Echo {
                            message: text.to_string(),
                        })
                        .is_err()
                    {
                        break;
                    }
                    continue;
                };

                match client_msg {
                    ClientMsg::Authenticate { token } => {
                        match verify_jwt(&token, &state.config.jwt_secret) {
                            Ok(claims) => {
                                if let Some((prev_id, _)) = session_user {
                                    state.ws_hub.unregister(prev_id, conn_id).await;
                                }
                                session_user = Some((claims.sub, claims.username.clone()));
                                state
                                    .ws_hub
                                    .register(claims.sub, conn_id, out_tx.clone())
                                    .await;
                                if out_tx
                                    .try_send(ServerMsg::Authenticated {
                                        user_id: claims.sub,
                                        username: claims.username,
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(_) => {
                                let _ = out_tx.try_send(ServerMsg::Error {
                                    code: "unauthorized".into(),
                                    message: "invalid JWT".into(),
                                });
                            }
                        }
                    }
                    ClientMsg::Ping { client_time_ms } => {
                        let server_time_ms = Utc::now().timestamp_millis().max(0) as u64;
                        if out_tx
                            .try_send(ServerMsg::Pong {
                                client_time_ms,
                                server_time_ms,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    // Placeholder hook for Task 1 (State Matrix) & Task 2 (Nemesis Matchmaker).
                    _ => {
                        let msg = if session_user.is_none() {
                            ServerMsg::Error {
                                code: "unauthenticated".into(),
                                message: "authenticate before queueing or playing".into(),
                            }
                        } else {
                            ServerMsg::Error {
                                code: "not_implemented".into(),
                                message: "State Matrix / Matchmaker handler wired in Task 1 & 2"
                                    .into(),
                            }
                        };
                        if out_tx.try_send(msg).is_err() {
                            break;
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if let Some((user_id, _)) = session_user {
        state.ws_hub.unregister(user_id, conn_id).await;
    }
    drop(out_tx);
    let _ = writer_task.await;
    tracing::debug!(?session_user, %conn_id, "websocket disconnected");
}
