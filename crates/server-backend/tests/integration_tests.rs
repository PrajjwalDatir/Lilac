use std::net::SocketAddr;

use futures_util::{SinkExt, StreamExt};
use lilac_proto::{
    AuthResponse, ClientMsg, CohortLeaderboardDto, GlobalLeaderboardDto, LoginRequest,
    PlayerProfileDto, RankTier, RegisterRequest, ServerMsg,
};
use server_backend::{
    app::{AppState, build_router},
    config::AppConfig,
    db::{self, CohortMembershipRow, CohortRow},
    events::InMemoryEventBus,
    ws_hub::ConnectionRegistry,
};
use tokio::net::TcpListener;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tower::ServiceExt;
use uuid::Uuid;

struct TestApp {
    addr: SocketAddr,
    state: AppState,
    router: axum::Router,
}

impl TestApp {
    async fn spawn() -> Self {
        let database_url = std::env::var("TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://monarch@localhost:5432/lilac_test".into());

        let config = AppConfig {
            bind_addr: "127.0.0.1:0".into(),
            database_url: database_url.clone(),
            jwt_secret: "test-jwt-secret-key".into(),
            jwt_ttl_hours: 1,
            gold_qualification_target: RankTier::DEFAULT_GOLD_QUALIFICATION_GAMES,
        };

        let pool = db::connect_pool(&database_url)
            .await
            .expect("failed to connect to test Postgres database");
        db::run_migrations(&pool)
            .await
            .expect("failed to run migrations");

        let state = AppState {
            config,
            db: pool,
            event_bus: InMemoryEventBus::new(128),
            ws_hub: ConnectionRegistry::new(),
        };
        let router = build_router(state.clone());

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let serve_router = router.clone();
        tokio::spawn(async move {
            axum::serve(listener, serve_router).await.unwrap();
        });

        Self {
            addr,
            state,
            router,
        }
    }
}

#[tokio::test]
async fn health_auth_profile_leaderboard_and_split_ws_test() {
    let app = TestApp::spawn().await;

    // 1. /health
    let resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    // 2. Unauthorized /api/profile/me returns 401
    let unauth_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/profile/me")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth_resp.status(), axum::http::StatusCode::UNAUTHORIZED);

    // 3a. Invalid username rejected with 400 Bad Request
    let bad_user_body = serde_json::to_vec(&RegisterRequest {
        username: "bad user!".into(),
        email: "bad@lilac.gg".into(),
        password: "strongpassword123".into(),
    })
    .unwrap();
    let bad_user_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(bad_user_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_user_resp.status(), axum::http::StatusCode::BAD_REQUEST);

    // 3b. Register a unique player + verify case-insensitive duplicate returns 409 Conflict
    let suffix = &Uuid::new_v4().simple().to_string()[..8];
    let username = format!("player_{suffix}");
    let email = format!("{username}@lilac.gg");
    let password = "strongpassword123";

    let reg_body = serde_json::to_vec(&RegisterRequest {
        username: username.clone(),
        email: email.clone(),
        password: password.to_string(),
    })
    .unwrap();

    let reg_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(reg_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reg_resp.status(), axum::http::StatusCode::CREATED);

    let dup_case_body = serde_json::to_vec(&RegisterRequest {
        username: username.to_uppercase(),
        email: format!("other_{suffix}@lilac.gg"),
        password: password.to_string(),
    })
    .unwrap();
    let dup_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(dup_case_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dup_resp.status(), axum::http::StatusCode::CONFLICT);

    // 4. Wrong password returns 401; valid login returns JWT
    let bad_login = serde_json::to_vec(&LoginRequest {
        username_or_email: username.clone(),
        password: "wrongpassword".into(),
    })
    .unwrap();
    let bad_login_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(bad_login))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bad_login_resp.status(), axum::http::StatusCode::UNAUTHORIZED);

    let login_body = serde_json::to_vec(&LoginRequest {
        username_or_email: username.clone(),
        password: password.to_string(),
    })
    .unwrap();
    let login_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(login_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_resp.status(), axum::http::StatusCode::OK);
    let login_bytes = axum::body::to_bytes(login_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let auth: AuthResponse = serde_json::from_slice(&login_bytes).unwrap();
    assert_eq!(auth.username, username);

    // 5. Fetch /api/profile/me & /api/profile/{username}
    let prof_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/profile/me")
                .header("authorization", format!("Bearer {}", auth.token))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prof_resp.status(), axum::http::StatusCode::OK);
    let prof_bytes = axum::body::to_bytes(prof_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let profile: PlayerProfileDto = serde_json::from_slice(&prof_bytes).unwrap();
    assert_eq!(profile.rank.tier, RankTier::Bronze);
    assert_eq!(profile.rank.rr_points, 50);
    assert!(!profile.rank.global_rating_qualified);
    assert_eq!(profile.rank.global_rating, None);
    assert!(!profile.achievements.is_empty());

    let pub_prof_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/profile/{username}?time_control=classical"))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(pub_prof_resp.status(), axum::http::StatusCode::OK);

    // Ensure GET /api/profile/{username}?time_control=classical did NOT mutate DB
    let classical_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM player_ratings WHERE user_id = $1 AND time_control = 'classical'",
    )
    .bind(auth.user_id)
    .fetch_one(&app.state.db)
    .await
    .unwrap();
    assert_eq!(classical_count, 0);

    // 6. Gold+ Global Leaderboard gate: Bronze user excluded, promoted Gold+ qualified user included
    let lb_before_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/leaderboard/global?time_control=blitz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(lb_before_resp.status(), axum::http::StatusCode::OK);
    let lb_before: GlobalLeaderboardDto = serde_json::from_slice(
        &axum::body::to_bytes(lb_before_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(!lb_before.entries.iter().any(|e| e.user_id == auth.user_id));

    // Qualify this user at Gold II (global_rating = 1580) & add to a test Cohort
    sqlx::query(
        r#"
        UPDATE player_ratings
        SET tier = 'gold', division = 2, rr_points = 60,
            global_rating_qualified = TRUE, global_rating = 1580, gold_qualification_games = 10
        WHERE user_id = $1 AND time_control = 'blitz'
        "#,
    )
    .bind(auth.user_id)
    .execute(&app.state.db)
    .await
    .unwrap();

    let cohort = CohortRow::create(&app.state.db, &format!("Cohort-{suffix}"), 1, 10_000)
        .await
        .unwrap();
    CohortMembershipRow::add_member(&app.state.db, cohort.id, auth.user_id, "gold")
        .await
        .unwrap();

    let lb_after_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/leaderboard/global?time_control=blitz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let lb_after: GlobalLeaderboardDto = serde_json::from_slice(
        &axum::body::to_bytes(lb_after_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        lb_after
            .entries
            .iter()
            .any(|e| e.user_id == auth.user_id && e.global_rating == Some(1580))
    );

    let cohort_lb_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/leaderboard/cohort/{}", cohort.id))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cohort_lb_resp.status(), axum::http::StatusCode::OK);
    let cohort_lb: CohortLeaderboardDto = serde_json::from_slice(
        &axum::body::to_bytes(cohort_lb_resp.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cohort_lb.entries.len(), 1);
    assert_eq!(cohort_lb.entries[0].user_id, auth.user_id);

    // 7. Connect WebSocket, verify post-connect Authenticate, Ping/Pong, AND live ws_hub push
    let ws_url = format!("ws://{}/ws", app.addr);
    let (mut ws_stream, _) = connect_async(&ws_url).await.unwrap();

    let auth_cmd = serde_json::to_string(&ClientMsg::Authenticate {
        token: auth.token.clone(),
    })
    .unwrap();
    ws_stream.send(Message::Text(auth_cmd.into())).await.unwrap();

    let first = ws_stream.next().await.unwrap().unwrap();
    let auth_msg: ServerMsg = serde_json::from_str(first.to_text().unwrap()).unwrap();
    assert!(matches!(auth_msg, ServerMsg::Authenticated { .. }));

    // Push a BountyAlert directly from server ws_hub to confirm split-WS outbound channel works
    let pushed = app
        .state
        .ws_hub
        .send_to_user(
            auth.user_id,
            ServerMsg::BountyAlert {
                target_username: "rival_x".into(),
                bounty_rr: 15,
                message: "Bounty active!".into(),
            },
        )
        .await;
    assert!(pushed);

    let pushed_frame = ws_stream.next().await.unwrap().unwrap();
    let pushed_msg: ServerMsg = serde_json::from_str(pushed_frame.to_text().unwrap()).unwrap();
    assert!(matches!(
        pushed_msg,
        ServerMsg::BountyAlert { bounty_rr: 15, .. }
    ));

    let ping = serde_json::to_string(&ClientMsg::Ping {
        client_time_ms: 12345,
    })
    .unwrap();
    ws_stream.send(Message::Text(ping.into())).await.unwrap();

    let pong_frame = ws_stream.next().await.unwrap().unwrap();
    let pong_msg: ServerMsg = serde_json::from_str(pong_frame.to_text().unwrap()).unwrap();
    assert!(matches!(
        pong_msg,
        ServerMsg::Pong {
            client_time_ms: 12345,
            ..
        }
    ));

    // 8. Web UI static files, MIT SVG assets, /api/cohorts, and Rust-backed /api/sandbox/inspect
    let index_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(index_resp.status(), axum::http::StatusCode::OK);

    let svg_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/assets/pieces/spatial/wK.svg")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(svg_resp.status(), axum::http::StatusCode::OK);

    let sandbox_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sandbox/inspect")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"uci_moves":["e2e4","c7c5"]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sandbox_resp.status(), axum::http::StatusCode::OK);
    let sandbox_bytes = axum::body::to_bytes(sandbox_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let sandbox_json: serde_json::Value = serde_json::from_slice(&sandbox_bytes).unwrap();
    assert_eq!(sandbox_json["opening"]["eco"], "B20");
    assert_eq!(sandbox_json["turn"], "white");

    let cohorts_resp = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/cohorts")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cohorts_resp.status(), axum::http::StatusCode::OK);

    // 9. Multi-tab WS safety: open a second socket, close socket 1, confirm push still lands on socket 2
    let (mut ws_stream_2, _) = connect_async(&ws_url).await.unwrap();
    let auth_cmd_2 = serde_json::to_string(&ClientMsg::Authenticate {
        token: auth.token.clone(),
    })
    .unwrap();
    ws_stream_2
        .send(Message::Text(auth_cmd_2.into()))
        .await
        .unwrap();
    let _ = ws_stream_2.next().await.unwrap().unwrap(); // Authenticated

    // Close socket 1 and yield briefly so unregister runs
    drop(ws_stream);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let still_delivered = app
        .state
        .ws_hub
        .send_to_user(
            auth.user_id,
            ServerMsg::BountyAlert {
                target_username: "rival_y".into(),
                bounty_rr: 20,
                message: "Multi-tab safe!".into(),
            },
        )
        .await;
    assert!(still_delivered);
    let tab2_frame = ws_stream_2.next().await.unwrap().unwrap();
    let tab2_msg: ServerMsg = serde_json::from_str(tab2_frame.to_text().unwrap()).unwrap();
    assert!(matches!(
        tab2_msg,
        ServerMsg::BountyAlert { bounty_rr: 20, .. }
    ));

    // 10. Verify reliable EventBus queue delivers events without broadcast lag loss
    let mut reliable_rx = app.state.event_bus.take_reliable_rx().unwrap();
    app.state.event_bus.publish(lilac_proto::DomainEvent::CohortFilled {
        cohort_id: cohort.id,
        member_count: 1,
    });
    let ev = reliable_rx.recv().await.unwrap();
    assert!(matches!(
        ev,
        lilac_proto::DomainEvent::CohortFilled { member_count: 1, .. }
    ));
}
