//! HTTP API end to end on the fake engine.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use review_server::api::{AppState, router};
use review_server::engine::Evaluator;
use review_server::jobs::JobStore;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app(with_engine: bool) -> Router {
    router(AppState {
        evaluator: with_engine
            .then(|| Arc::new(common::FakeEngine::default()) as Arc<dyn Evaluator>),
        engine_error: (!with_engine).then(|| "no engine configured".to_owned()),
        openings: common::openings(),
        jobs: JobStore::new(2, 10),
    })
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(json) => {
            req = req.header("content-type", "application/json");
            Body::from(json.to_string())
        }
        None => Body::empty(),
    };
    let res = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn health_and_defaults() {
    let app = app(true);
    let (status, body) = call(&app, "GET", "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["opening_positions"].as_u64().unwrap() > 1000);

    let (_, defaults) = call(&app, "GET", "/api/config/defaults", None).await;
    assert_eq!(
        defaults["config"]["precision_thresholds"],
        json!([0.02, 0.05, 0.1, 0.2])
    );
    assert_eq!(
        defaults["config"]["outcome_thresholds"],
        json!([0.1, 0.2, 0.35, 0.65, 0.8, 0.9])
    );

    let (_, degraded) = call(&self::app(false), "GET", "/api/health", None).await;
    assert_eq!(degraded["status"], "degraded");
}

#[tokio::test]
async fn parse_reports_games_and_errors() {
    let app = app(true);
    let pgn = common::fixture("multi_games.pgn");
    let (status, body) = call(
        &app,
        "POST",
        "/api/games/parse",
        Some(json!({ "pgn": pgn })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let games = body["games"].as_array().unwrap();
    assert_eq!(games.len(), 3);
    assert_eq!(games[0]["game"]["moves"][3]["san"], "Qh4#");
    assert_eq!(games[1]["error"]["kind"], "illegal_move");

    let (status, body) = call(
        &app,
        "POST",
        "/api/games/parse",
        Some(json!({ "pgn": "  " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "pgn_empty");

    let (status, body) = call(
        &app,
        "POST",
        "/api/games/parse",
        Some(json!({ "wrong": 1 })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_request_body");
}

#[tokio::test]
async fn full_review_flow() {
    let app = app(true);
    let pgn = common::fixture("scholars_mate.pgn");
    let (status, created) = call(
        &app,
        "POST",
        "/api/reviews",
        Some(json!({ "pgn": pgn, "strength": "fast" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = created["id"].as_str().unwrap().to_owned();

    let mut state = String::new();
    for _ in 0..100 {
        let (_, status) = call(&app, "GET", &format!("/api/reviews/{id}"), None).await;
        state = status["state"].as_str().unwrap().to_owned();
        if state == "completed" {
            assert_eq!(status["progress"]["done"], status["progress"]["total"]);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(state, "completed");

    let (status, result) = call(&app, "GET", &format!("/api/reviews/{id}/result"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["plies"].as_array().unwrap().len(), 7);
    assert_eq!(
        result["settings"]["limit"],
        json!({ "kind": "depth", "depth": 12 })
    );
    assert_eq!(result["game"]["white"], "Alice");

    let (status, ply) = call(&app, "GET", &format!("/api/reviews/{id}/plies/7"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ply["san"], "Qxf7#");
    assert_eq!(
        ply["eval_after"],
        json!({ "kind": "mate", "winner": "white", "moves": 0 })
    );

    let (status, _) = call(&app, "GET", &format!("/api/reviews/{id}/plies/99"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn review_validation_errors() {
    let app = app(true);
    let pgn = common::fixture("scholars_mate.pgn");
    let cases = [
        (json!({ "pgn": pgn, "depth": 99 }), "invalid_settings"),
        (
            json!({ "pgn": pgn, "depth": 10, "movetime_ms": 100 }),
            "invalid_settings",
        ),
        (json!({ "pgn": pgn, "multipv": 9 }), "invalid_settings"),
        (json!({ "pgn": pgn, "game_index": 3 }), "game_not_found"),
        (
            json!({ "pgn": common::fixture("invalid.pgn") }),
            "pgn_invalid",
        ),
        (json!({ "pgn": "[Event \"x\"]\n\n*" }), "no_moves"),
    ];
    for (body, code) in cases {
        let (status, res) = call(&app, "POST", "/api/reviews", Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{res}");
        assert_eq!(res["error"]["code"], code);
    }

    let (status, res) = call(
        &self::app(false),
        "POST",
        "/api/reviews",
        Some(json!({ "pgn": pgn })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(res["error"]["code"], "engine_unavailable");

    let (status, _) = call(&app, "GET", "/api/reviews/not-a-uuid", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn position_analysis() {
    let app = app(true);
    let (status, body) = call(
        &app,
        "POST",
        "/api/positions/analyze",
        Some(json!({ "fen": "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1", "multipv": 2, "depth": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["lines"][0]["moves"][0]["san"], "Rd8#");

    let (status, body) = call(
        &app,
        "POST",
        "/api/positions/analyze",
        Some(json!({ "fen": "not a fen" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_fen");
}

#[tokio::test]
async fn the_player_side_is_kept_with_the_review() {
    let app = app(true);
    let pgn = common::fixture("scholars_mate.pgn");
    let (status, created) = call(
        &app,
        "POST",
        "/api/reviews",
        Some(json!({ "pgn": pgn, "strength": "fast", "player_side": "black" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = created["id"].as_str().unwrap().to_owned();
    for _ in 0..100 {
        let (_, status) = call(&app, "GET", &format!("/api/reviews/{id}"), None).await;
        if status["state"] == "completed" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (status, result) = call(&app, "GET", &format!("/api/reviews/{id}/result"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["settings"]["player_side"], "black");

    // Without it the field is null, and an unknown side is rejected.
    let (status, _) = call(
        &app,
        "POST",
        "/api/reviews",
        Some(json!({ "pgn": pgn, "player_side": "green" })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
