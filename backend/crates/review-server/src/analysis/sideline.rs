//! Classifying a move the reader plays in a sideline (docs/explore-mode.md §5.5). The review's own
//! rules (`facts::judge_move`) run on engine lines the browser sends; the game's context (rating,
//! repetitions, phase, book, the previous move's loss) comes from the stored review. No engine
//! search is made here.

use std::collections::HashMap;

use review_core::{Classification, Eval, MissKind, Phase, Rule, Side, Value};
use serde::Serialize;
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Move, Position};
use thiserror::Error;
use ts_rs::TS;

use super::facts::{CoachContext, MoveInput, PreviousMoves, coach_note, judge_move};
use super::model::{MoveRef, ReviewResult};
use super::{explain, to_san_line};
use crate::board::{self, side};
use crate::coach::CoachNote;
use crate::engine::EngineLine;
use crate::openings::OpeningBook;

/// Longest sideline accepted (plies), a bound on the work one request can ask for.
pub const MAX_SIDELINE_PLIES: usize = 400;

/// A sideline move to judge, with the engine lines of the positions around it.
pub struct SidelineMove<'a> {
    /// Game ply the sideline branches from: its first move replaces game move `from_ply + 1`.
    pub from_ply: u32,
    /// The sideline's moves (UCI) up to and including the move judged.
    pub moves: &'a [String],
    /// Engine lines of the position before the judged move, best first.
    pub before: Vec<EngineLine>,
    /// Best line of the position after it (needed unless the move is one of `before`'s lines, or
    /// the game is over after it).
    pub after: Option<EngineLine>,
    /// Expected points the opponent lost on the previous sideline move; for the first sideline
    /// move the game's previous move is used.
    pub previous_ep_lost: Option<f64>,
    /// The browser engine's null-move threat searches for the coach.
    pub threats: SidelineThreats,
}

/// A threat search: the position searched (a null-move position) and its best line.
pub struct ThreatLine {
    pub fen: String,
    pub line: EngineLine,
}

/// The coach's three null-move threat searches (`coach::CoachInput`). Each is used only when
/// its position is the one the server expects and its line starts with a legal move there.
#[derive(Default)]
pub struct SidelineThreats {
    /// After the move, its mover to move again.
    pub after: Option<ThreatLine>,
    /// Before the move, the opponent to move.
    pub before: Option<ThreatLine>,
    /// After the engine's best move (`before`'s first line), its mover to move again.
    pub best: Option<ThreatLine>,
}

/// How a sideline move is classified, as a game move would be.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct SidelineJudgement {
    pub classification: Option<Classification>,
    pub miss_kind: Option<MissKind>,
    pub rule: Rule,
    /// Expected points the move lost against the best move (≥ 0); the next move's `previous_ep_lost`.
    pub ep_lost: f64,
    pub ep_before: f64,
    pub ep_after: f64,
    pub played_is_best: bool,
    pub show_best_move: bool,
    /// The engine's best move from the position before.
    pub best_move: Option<MoveRef>,
    /// Factual sentences, as for game moves.
    pub explanation: Vec<String>,
    /// The coach's note, as for game moves (empty without the threat searches it needs).
    pub coach: CoachNote,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum JudgeError {
    #[error("ply {0} is past the end of the game")]
    BadPly(u32),
    #[error("a sideline of 1 to {MAX_SIDELINE_PLIES} moves is needed")]
    BadLength,
    #[error("{0} is not a legal move in the sideline")]
    IllegalMove(String),
    #[error("the engine lines are missing or do not start with a legal move")]
    BadLines,
    #[error("the engine line of the position after the move is needed")]
    MissingAfter,
    #[error("the review's game could not be replayed")]
    Replay,
}

/// Classifies the last move of a sideline with the review's rules.
pub fn judge_sideline(
    review: &ReviewResult,
    openings: &OpeningBook,
    request: &SidelineMove<'_>,
) -> Result<SidelineJudgement, JudgeError> {
    let from = request.from_ply as usize;
    if from > review.plies.len() {
        return Err(JudgeError::BadPly(request.from_ply));
    }
    if request.moves.is_empty() || request.moves.len() > MAX_SIDELINE_PLIES {
        return Err(JudgeError::BadLength);
    }
    let cfg = &review.config;

    // The game up to the branch, then the sideline.
    let mode = if review.game.chess960 {
        CastlingMode::Chess960
    } else {
        CastlingMode::Standard
    };
    let start: Chess = review
        .game
        .start_fen
        .parse::<Fen>()
        .ok()
        .and_then(|f| f.into_position(mode).ok())
        .ok_or(JudgeError::Replay)?;
    let mut positions = vec![start];
    let mut played: Vec<Move> = Vec::new();
    let game_moves = review.plies[..from]
        .iter()
        .map(|p| (&p.uci, JudgeError::Replay));
    let sideline_moves = request
        .moves
        .iter()
        .map(|u| (u, JudgeError::IllegalMove(u.clone())));
    for (uci, error) in game_moves.chain(sideline_moves) {
        let pos = positions.last().expect("the start position");
        let m = board::parse_uci(pos, uci).ok_or(error)?;
        let mut next = pos.clone();
        next.play_unchecked(m);
        played.push(m);
        positions.push(next);
    }
    let i = played.len() - 1;
    let before = &positions[i];
    let after = &positions[i + 1];

    // Repetitions and the phase are tracked along the whole line, as in the review.
    let mut seen: HashMap<String, u32> = HashMap::new();
    let draws: Vec<bool> = positions
        .iter()
        .map(|pos| {
            let count = seen.entry(board::epd(pos)).or_insert(0);
            *count += 1;
            *count >= 3 || board::is_board_draw(pos)
        })
        .collect();
    let mut phase_after = Phase::Opening;
    for (ply, pos) in positions.iter().enumerate() {
        phase_after = phase_after.max(board::raw_phase(pos.board(), ply as u32));
    }

    // The lines must be the position's: each starts with a legal move.
    let lines = request.before.as_slice();
    if lines.is_empty()
        || lines.iter().any(|l| {
            l.pv.first()
                .is_none_or(|m| board::parse_uci(before, m).is_none())
        })
    {
        return Err(JudgeError::BadLines);
    }
    let uci = &request.moves[request.moves.len() - 1];
    let in_lines = lines.iter().any(|l| l.pv.first() == Some(uci));
    let eval_after = if after.legal_moves().is_empty() {
        if after.is_checkmate() {
            Eval::Mate {
                winner: side(after.turn().other()),
                moves: 0,
            }
        } else {
            Eval::DRAW
        }
    } else if let Some(line) = &request.after {
        line.eval
    } else if in_lines {
        // Unused: the move's own line from `before` scores it.
        lines[0].eval
    } else {
        return Err(JudgeError::MissingAfter);
    };

    let in_book = |k: usize| openings.contains(&positions[k]);
    let is_book = if cfg.book_requires_unbroken {
        (from == 0 || review.plies[from - 1].is_book) && (from + 1..=i + 1).all(in_book)
    } else {
        in_book(i + 1)
    };
    let opponent_previous_ep_lost = request
        .previous_ep_lost
        .or_else(|| (i == from && from > 0).then(|| review.plies[from - 1].ep_lost));
    let highest_elo = review
        .game
        .white_elo
        .unwrap_or(0)
        .max(review.game.black_elo.unwrap_or(0));
    // Game moves are known from the review. Earlier sideline moves are not: the opponent's is graded
    // from its loss alone, and the positions before it have no score here.
    let game_ply = |k: usize| (k < from).then(|| &review.plies[k]);
    let mover = side(before.turn());
    let previous_moves = PreviousMoves {
        opponent_erred: match i.checked_sub(1) {
            Some(k) if k < from => {
                PreviousMoves::is_error(review.plies[k].precision, review.plies[k].miss_kind)
            }
            // Mistake or worse, or not graded.
            Some(_) => {
                opponent_previous_ep_lost.is_none_or(|lost| lost > cfg.precision_thresholds[2])
            }
            None => false,
        },
        own_value: i
            .checked_sub(2)
            .and_then(game_ply)
            .map(|p| Value::after_move(p.eval_after, mover)),
        opponent_earlier_value: i
            .checked_sub(3)
            .and_then(game_ply)
            .map(|p| Value::after_move(p.eval_after, mover.opponent())),
        own_great_tag: i
            .checked_sub(2)
            .and_then(game_ply)
            .is_some_and(|p| p.classification == Some(Classification::GreatFind)),
        // Not kept in the review: only the wins-material key moment and the ignored-threat
        // sacrifice read these.
        own_wins_material: false,
        own_ignored_threat: None,
    };

    let input = MoveInput {
        before,
        after,
        mv: played[i],
        uci,
        previous: i.checked_sub(1).map(|p| played[p]),
        lines,
        eval_before: lines[0].eval,
        before_is_draw: draws[i],
        eval_after,
        after_line: request.after.as_ref(),
        after_is_draw: draws[i + 1],
        is_book,
        phase_after,
        has_previous_move: i > 0,
        // The "easy to find" search is not made for sidelines (it is off by default).
        shallow_best: None,
        opponent_previous_ep_lost,
        previous_moves,
        rating: review.rating_used,
        highest_elo,
        own_history: &[],
    };
    let judged = judge_move(&input, cfg);

    // The threat searches the browser made, where they are of the positions the coach expects.
    let threat_depth_on = cfg.coach.threat_search_depth > 0;
    let usable = |expected: Option<Chess>, given: &Option<ThreatLine>| -> Option<EngineLine> {
        let (expected, given) = (expected?, given.as_ref()?);
        let pos: Chess = given.fen.parse::<Fen>().ok()?.into_position(mode).ok()?;
        let first = given.line.pv.first()?;
        (board::epd(&pos) == board::epd(&expected) && board::parse_uci(&expected, first).is_some())
            .then(|| given.line.clone())
    };
    let after_best = judged
        .best_move
        .filter(|_| !judged.played_is_best)
        .map(|bm| {
            let mut p = before.clone();
            p.play_unchecked(bm);
            p
        });
    // As in the review, a position whose side to move has no legal move is not searched, so it
    // has no threat search either.
    let threat = usable(
        board::null_move(after).filter(|_| threat_depth_on && !after.legal_moves().is_empty()),
        &request.threats.after,
    );
    let prev_threat = usable(
        board::null_move(before).filter(|_| threat_depth_on),
        &request.threats.before,
    );
    let best_threat = usable(
        after_best
            .as_ref()
            .and_then(board::null_move)
            .filter(|_| threat_depth_on && cfg.coach.best_threat_search),
        &request.threats.best,
    );
    let coach_rating = cfg.coach.rating_gates.then(|| {
        let elo = match review.settings.player_side {
            Some(Side::White) => review.game.white_elo,
            Some(Side::Black) => review.game.black_elo,
            None => None,
        };
        elo.map_or(review.rating_used, |e| e as i32)
    });
    let coach = coach_note(
        &input,
        &judged,
        &CoachContext {
            threat: threat.as_ref(),
            prev_threat: prev_threat.as_ref(),
            best_threat: best_threat.as_ref(),
            rating: coach_rating,
            // No human-move model for sidelines: the coach words difficulty without it.
            difficulty: None,
            player_side: review.settings.player_side,
            seed: i as u32,
        },
        cfg,
    );

    let outcome = judged.outcome;
    let best_move = to_san_line(before, &lines[0].pv, 1).into_iter().next();
    let explanation = explain::describe(&explain::MoveContext {
        mover: judged.mover,
        classification: outcome.classification,
        miss_kind: outcome.miss_kind,
        played_is_best: judged.played_is_best,
        best_san: best_move.as_ref().map(|r| r.san.as_str()),
        played: judged.played_value,
        best: judged.best_value,
        ep_before: judged.ep_before,
        ep_after: judged.ep_played,
        material_lost_cp: judged.material_lost,
        best_gain_cp: judged.best_gain,
        opening: if is_book { openings.name(after) } else { None },
    });
    Ok(SidelineJudgement {
        classification: outcome.classification,
        miss_kind: outcome.miss_kind,
        rule: outcome.rule,
        ep_lost: outcome.ep_lost,
        ep_before: judged.ep_before,
        ep_after: judged.ep_played,
        played_is_best: judged.played_is_best,
        show_best_move: outcome
            .classification
            .is_some_and(Classification::shows_best_move)
            && !judged.played_is_best,
        best_move,
        explanation,
        coach,
    })
}
