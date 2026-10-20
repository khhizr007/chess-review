//! Runs the coach's tactic detectors on lichess puzzles (development tooling
//! for `tools/calibrate/puzzle_check.py`, see `docs/calibration/detector-check.md`).
//!
//! Reads JSON lines `{"id", "fen", "moves"}` (the lichess puzzle CSV fields:
//! `fen` is the position before the opponent's move, `moves` the opponent's
//! move and then the solution, in UCI) and writes one JSON line per puzzle:
//!
//! - `fired`: the tactics our detectors find on each of the solver's moves
//!   (`{ply, tag, role, defended}`, ply counted from the puzzle start, `role`
//!   the piece the tag is about, `defended` whether its side guarded it);
//! - `misses`: the event the coach says for the solution as a best line
//!   (MISSES: the tactic behind the material, named at any of the solver's
//!   moves up to the gain, WINS_MATERIAL on the first move, else LEADS_TO);
//! - `permits`: the same line as the reply to the opponent's move (PERMITS:
//!   a tactic is named on the first move only, or the second after a quiet
//!   check).
//!
//! ```text
//! cargo run --release -p review-server --example puzzle_tags -- <in.jsonl> <out.jsonl>
//! ```

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::ExitCode;

use review_server::board;
use review_server::coach::circ::{self, Event, Line, LineKind};
use review_server::coach::detect::{self, Found};
use review_server::coach::facts;
use serde_json::{Value, json};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Position};

/// Plies of a line inspected for material, as in the review's default.
const HORIZON: u32 = 6;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("puzzle_tags: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        return Err("usage: puzzle_tags <in.jsonl> <out.jsonl>".to_owned());
    };
    let reader = BufReader::new(std::fs::File::open(input).map_err(|e| format!("{input}: {e}"))?);
    let mut out = BufWriter::new(
        std::fs::File::create(output).map_err(|e| format!("{output}: {e}"))?,
    );
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let puzzle: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        let result = check(&puzzle).unwrap_or_else(|e| json!({ "error": e }));
        let mut record = json!({ "id": puzzle["id"] });
        if let (Value::Object(r), Value::Object(extra)) = (&mut record, result) {
            r.extend(extra);
        }
        writeln!(out, "{record}").map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())
}

fn check(puzzle: &Value) -> Result<Value, String> {
    let fen = puzzle["fen"].as_str().ok_or("no fen")?;
    let moves: Vec<String> = puzzle["moves"]
        .as_str()
        .ok_or("no moves")?
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut pos: Chess = fen
        .parse::<Fen>()
        .map_err(|e| e.to_string())?
        .into_position(CastlingMode::Standard)
        .map_err(|e| e.to_string())?;
    let (first, solution) = moves.split_first().ok_or("no moves")?;
    let opening = board::parse_uci(&pos, first).ok_or("bad first move")?;
    pos.play_unchecked(opening);

    // The solution ends in mate: the line's first mover mates in that many moves.
    let mut end = pos.clone();
    for text in solution {
        let m = board::parse_uci(&end, text).ok_or("bad solution move")?;
        end.play_unchecked(m);
    }
    let mate_in = end
        .is_checkmate()
        .then(|| u32::try_from(solution.len().div_ceil(2)).unwrap_or(u32::MAX));

    let line = Line::parse(&pos, solution, mate_in, HORIZON, Some(opening));
    if line.moves.len() != solution.len() {
        return Err("line longer than the coach reads".to_owned());
    }
    let fired: Vec<Value> = (0..line.moves.len())
        .step_by(2)
        .flat_map(|k| {
            let ctx = line.ctx(k, Some(opening));
            detect::tactics(&ctx)
                .into_iter()
                .map(|f| found_json(&f, ctx.before, k))
                .collect::<Vec<_>>()
        })
        .collect();
    let misses = circ::line_event(&line, LineKind::Best, circ::Circ::Misses, Some(opening));
    let permits = circ::permits(&line, opening);
    Ok(json!({
        "fired": fired,
        "misses": misses.as_ref().map(|e| event_json(e, &line)),
        "permits": permits.as_ref().map(|e| event_json(e, &line)),
        "net": line.net(),
        "mate_in": mate_in,
    }))
}

/// A tag found on the solver's move at `ply`, with the piece it is about and,
/// for that piece, whether its own side defended it before the move (a free
/// piece taken undefended is what lichess calls a hanging piece).
fn found_json(f: &Found, before: &Chess, ply: usize) -> Value {
    let focus = f.focus.map(|p| p.square);
    let defended = focus.and_then(|sq| {
        let color = before.board().color_at(sq)?;
        Some(facts::attackers(before.board(), sq, color).any())
    });
    json!({
        "ply": ply,
        "tag": f.tag,
        "role": f.focus.map(|p| p.role.char().to_string()),
        "defended": defended,
    })
}

fn event_json(e: &Event, line: &Line) -> Value {
    let mut v = found_json(&e.found, &line.positions[e.ply], e.ply);
    v["chain"] = json!(e.chain);
    v["gain"] = json!(e.gain.map(|g| g.cp));
    v
}
