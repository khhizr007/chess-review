//! Converts UCI moves to SAN in the positions of a PGN game (calibration tooling).
//!
//! ```text
//! cargo run --release -p review-server --example uci2san -- <game.pgn> <moves.json>
//! ```
//!
//! `<moves.json>` is either a list `[{"ply": 12, "uci": "c3d5"}, …]` or a calibration
//! `labels.json`, whose `plies[].app_best_uci` entries are converted. `ply` is 1-based and the
//! move is played in the position *before* that ply's move. Prints a JSON list
//! `[{"ply": 12, "uci": "c3d5", "san": "Nd5", "error": null}, …]` to stdout.
//! `tools/calibrate/fill_best_san.py` uses it to fill `app_best_san` in labels files.

use std::process::ExitCode;

use review_server::board;
use review_server::pgn::{ParsedGame, parse_pgn};
use serde_json::{Value, json};
use shakmaty::san::SanPlus;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("uci2san: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pgn_path, moves_path] = args.as_slice() else {
        return Err("usage: uci2san <game.pgn> <moves.json | labels.json>".to_owned());
    };
    let pgn = std::fs::read_to_string(pgn_path).map_err(|e| format!("{pgn_path}: {e}"))?;
    let game = parse_pgn(&pgn)
        .map_err(|e| format!("{pgn_path}: {e}"))?
        .into_iter()
        .next()
        .ok_or_else(|| format!("{pgn_path}: no game"))?
        .map_err(|e| format!("{pgn_path}: {e}"))?;
    let text = std::fs::read_to_string(moves_path).map_err(|e| format!("{moves_path}: {e}"))?;
    let input: Value = serde_json::from_str(&text).map_err(|e| format!("{moves_path}: {e}"))?;
    let Some(requests) = requests(&input) else {
        return Err(format!(
            "{moves_path}: expected [{{ply, uci}}] or a labels object"
        ));
    };
    let out: Vec<Value> = requests
        .into_iter()
        .map(|(ply, uci)| match san_at(&game, ply, &uci) {
            Ok(san) => json!({ "ply": ply, "uci": uci, "san": san, "error": null }),
            Err(e) => json!({ "ply": ply, "uci": uci, "san": null, "error": e }),
        })
        .collect();
    println!("{:#}", Value::Array(out));
    Ok(())
}

/// `(ply, uci)` pairs from a list of `{ply, uci}` or from a labels object's `app_best_uci`.
fn requests(input: &Value) -> Option<Vec<(u64, String)>> {
    let (items, field) = match input {
        Value::Array(items) => (items, "uci"),
        Value::Object(obj) => (obj.get("plies")?.as_array()?, "app_best_uci"),
        _ => return None,
    };
    Some(
        items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let uci = item.get(field)?.as_str()?.trim();
                let ply = item
                    .get("ply")
                    .and_then(Value::as_u64)
                    .unwrap_or(i as u64 + 1);
                (!uci.is_empty()).then(|| (ply, uci.to_owned()))
            })
            .collect(),
    )
}

fn san_at(game: &ParsedGame, ply: u64, uci: &str) -> Result<String, String> {
    let index = usize::try_from(ply)
        .ok()
        .and_then(|p| p.checked_sub(1))
        .filter(|i| *i < game.moves.len())
        .ok_or_else(|| format!("ply {ply} is outside the game (1..={})", game.moves.len()))?;
    let pos = &game.positions[index];
    let m = board::parse_uci(pos, uci)
        .ok_or_else(|| format!("{uci:?} is not a legal move before ply {ply}"))?;
    Ok(SanPlus::from_move(pos.clone(), m).to_string())
}
