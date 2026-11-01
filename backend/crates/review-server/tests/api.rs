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
use review_server::store::ReviewStore;
use serde_json::{Value, json};
use tower::ServiceExt;

fn app(with_engine: bool) -> Router {
    app_with(with_engine, JobStore::new(2, common::review_store()))
}

fn app_with(with_engine: bool, jobs: JobStore) -> Router {
    router(AppState {
        evaluator: with_engine
            .then(|| Arc::new(common::FakeEngine::default()) as Arc<dyn Evaluator>),
        engine_error: (!with_engine).then(|| "no engine configured".to_owned()),
        openings: common::openings(),
        jobs,
    })
}

/// Starts a review of `pgn` and waits until it has finished.
async fn finished_review(app: &Router, pgn: &str) -> String {
    let (status, created) = call(
        app,
        "POST",
        "/api/reviews",
        Some(json!({ "pgn": pgn, "strength": "fast" })),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = created["id"].as_str().unwrap().to_owned();
    for _ in 0..200 {
        let (_, status) = call(app, "GET", &format!("/api/reviews/{id}"), None).await;
        if status["state"] == "completed" {
            return id;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("review {id} did not complete");
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
        json!({ "kind": "nodes", "nodes": 150000 })
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

/// Line from a review's JSON as the judge endpoint takes it.
fn judge_line(line: &Value) -> Value {
    let pv: Vec<&Value> = line["moves"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| &m["uci"])
        .collect();
    json!({ "eval": line["eval"], "depth": line["depth"], "pv": pv })
}

#[tokio::test]
async fn judging_a_sideline_move() {
    let app = app(true);
    let pgn = common::fixture("scholars_mate.pgn");
    let (_, created) = call(&app, "POST", "/api/reviews", Some(json!({ "pgn": pgn }))).await;
    let id = created["id"].as_str().unwrap().to_owned();
    for _ in 0..100 {
        let (_, status) = call(&app, "GET", &format!("/api/reviews/{id}"), None).await;
        if status["state"] == "completed" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (_, result) = call(&app, "GET", &format!("/api/reviews/{id}/result"), None).await;
    let plies = result["plies"].as_array().unwrap();

    // The game's 3...Nf6 asked as a sideline from ply 5 is judged as the review judged it.
    let nf6 = &plies[5];
    let body = json!({
        "from_ply": 5,
        "moves": [nf6["uci"]],
        "before": nf6["lines"].as_array().unwrap().iter().map(judge_line).collect::<Vec<_>>(),
        "after": judge_line(&plies[6]["lines"][0]),
    });
    let uri = format!("/api/reviews/{id}/judge");
    let (status, judged) = call(&app, "POST", &uri, Some(body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{judged}");
    assert_eq!(judged["classification"], nf6["classification"]);
    assert_eq!(judged["ep_lost"], nf6["ep_lost"]);
    assert_eq!(judged["best_move"]["san"], nf6["best_move"]["san"]);

    let mut illegal = body.clone();
    illegal["moves"] = json!(["e8e6"]);
    let (status, err) = call(&app, "POST", &uri, Some(illegal)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(err["error"]["code"], "illegal_move");

    let unknown = format!("/api/reviews/{}/judge", uuid::Uuid::new_v4());
    let (status, _) = call(&app, "POST", &unknown, Some(body)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_finished_review_leaves_memory_and_is_served_from_disk() {
    let dir = common::store_dir();
    let open = || ReviewStore::open(&dir, Duration::from_secs(3600), 100).unwrap();
    let jobs = JobStore::new(2, open());
    let app = app_with(true, jobs.clone());
    let id = finished_review(&app, &common::fixture("scholars_mate.pgn")).await;

    assert_eq!(
        jobs.in_memory(),
        0,
        "nothing of the finished review stays in memory"
    );
    assert!(dir.join(format!("{id}.json")).is_file());
    let (status, result) = call(&app, "GET", &format!("/api/reviews/{id}/result"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["plies"].as_array().unwrap().len(), 7);
    let (status, cancelled) = call(&app, "DELETE", &format!("/api/reviews/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["state"], "completed");

    // A restarted server (a new job store on the same directory) still has it.
    let restarted = app_with(true, JobStore::new(2, open()));
    let (status, again) = call(
        &restarted,
        "GET",
        &format!("/api/reviews/{id}/result"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, result, "the stored review reads back unchanged");
    let (status, ply) = call(
        &restarted,
        "GET",
        &format!("/api/reviews/{id}/plies/7"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ply["san"], "Qxf7#");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn an_expired_review_is_gone() {
    let dir = common::store_dir();
    let jobs = JobStore::new(
        2,
        ReviewStore::open(&dir, Duration::from_secs(3600), 100).unwrap(),
    );
    let app = app_with(true, jobs);
    let id = finished_review(&app, &common::fixture("scholars_mate.pgn")).await;

    // Same directory, a time to live every file has already outlived.
    let expired = JobStore::new(2, ReviewStore::open(&dir, Duration::ZERO, 100).unwrap());
    let app = app_with(true, expired.clone());
    let (status, _) = call(&app, "GET", &format!("/api/reviews/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&app, "GET", &format!("/api/reviews/{id}/result"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        !dir.join(format!("{id}.json")).exists(),
        "the read deleted it"
    );
    assert_eq!(expired.sweep().await.unwrap().kept, 0);
    std::fs::remove_dir_all(&dir).unwrap();
}
