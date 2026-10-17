//! Review pipeline: engine searches → facts → recovered rules → summary.

pub mod detectors;
pub mod explain;
pub mod model;
pub mod see;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use futures::StreamExt;
use review_core::caps2::{self, Caps2Move, Caps2State, ScoredMove};
use review_core::classification::{self, MaterialFacts, MissedChances, MoveEvents, MoveFacts};
use review_core::expected_points::{
    ExpectedOutcome, expected_outcome, node_expected_points, rating_for_expected_points,
};
use review_core::summary::tally;
use review_core::{Classification, Eval, Phase, Precision, ReviewConfig, Side, Value};
use shakmaty::san::SanPlus;
use shakmaty::{Chess, Color, Move, Position};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::board::{self, side};
use crate::engine::{
    AnalysisRequest, EngineError, EngineLine, Evaluator, PositionAnalysis, SearchLimit,
};
use crate::openings::OpeningBook;
use crate::pgn::ParsedGame;
use detectors::{
    LineMaterial, is_recapture, material_gate, static_sacrifice, takes_free_piece, walk_line,
};
use model::*;

/// Moves shown per engine line in the UI.
const LINE_DISPLAY_PLIES: usize = 10;

#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub game: ParsedGame,
    pub limit: SearchLimit,
    pub multipv: u8,
    pub config: ReviewConfig,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReviewError {
    #[error("the game has no moves to review")]
    NoMoves,
    #[error(transparent)]
    Engine(#[from] EngineError),
    #[error("the review was cancelled")]
    Cancelled,
}

/// Progress callback: (positions analysed, total positions).
pub type ProgressFn = Arc<dyn Fn(u32, u32) + Send + Sync>;

/// Engine facts about one position of the game.
struct PositionInfo {
    eval: Eval,
    analysis: Option<PositionAnalysis>,
    is_draw: bool,
}

pub async fn run_review(
    request: ReviewRequest,
    evaluator: Arc<dyn Evaluator>,
    openings: Arc<OpeningBook>,
    cancel: CancellationToken,
    progress: ProgressFn,
) -> Result<ReviewResult, ReviewError> {
    let game = &request.game;
    if game.moves.is_empty() {
        return Err(ReviewError::NoMoves);
    }
    let positions = analyse_positions(&request, evaluator.as_ref(), &cancel, progress).await?;
    Ok(build_review(
        &request,
        &positions,
        &openings,
        &evaluator.name(),
    ))
}

/// Searches every non-terminal position (in parallel up to the evaluator's limit).
async fn analyse_positions(
    request: &ReviewRequest,
    evaluator: &dyn Evaluator,
    cancel: &CancellationToken,
    progress: ProgressFn,
) -> Result<Vec<PositionInfo>, ReviewError> {
    let game = &request.game;
    let last = game.positions.len() - 1;

    // Threefold repetition is tracked along the game (the board alone cannot tell).
    let mut seen: HashMap<String, u32> = HashMap::new();
    let draws: Vec<bool> = game
        .positions
        .iter()
        .map(|pos| {
            let count = seen.entry(board::epd(pos)).or_insert(0);
            *count += 1;
            *count >= 3 || board::is_board_draw(pos)
        })
        .collect();

    // Positions without legal moves (and a drawn final position) need no search.
    let needs_search: Vec<bool> = game
        .positions
        .iter()
        .enumerate()
        .map(|(i, pos)| !pos.legal_moves().is_empty() && !(i == last && draws[i]))
        .collect();
    let total = needs_search.iter().filter(|n| **n).count() as u32;
    let done = AtomicU32::new(0);
    progress(0, total);

    let work: Vec<Option<AnalysisRequest>> = game
        .positions
        .iter()
        .zip(&needs_search)
        .map(|(pos, needs)| {
            needs.then(|| AnalysisRequest {
                fen: board::fen(pos),
                limit: request.limit,
                multipv: request.multipv.max(1),
                chess960: game.castling_mode == shakmaty::CastlingMode::Chess960,
                searchmoves: None,
            })
        })
        .collect();
    let done = &done;
    let progress = &progress;
    let searches = futures::stream::iter(work.into_iter().map(|req| async move {
        let Some(req) = req else { return Ok(None) };
        let analysis = evaluator.analyze(&req, cancel).await?;
        let n = done.fetch_add(1, Ordering::SeqCst) + 1;
        progress(n, total);
        Ok::<_, EngineError>(Some(analysis))
    }))
    .buffered(evaluator.parallelism().max(1))
    .collect::<Vec<_>>()
    .await;

    let mut infos = Vec::with_capacity(searches.len());
    for (i, result) in searches.into_iter().enumerate() {
        let analysis = match result {
            Ok(a) => a,
            Err(EngineError::Cancelled) => return Err(ReviewError::Cancelled),
            Err(e) => return Err(ReviewError::Engine(e)),
        };
        if cancel.is_cancelled() {
            return Err(ReviewError::Cancelled);
        }
        let pos = &game.positions[i];
        let eval = match &analysis {
            Some(a) => a.best().map(|l| l.eval).unwrap_or(Eval::DRAW),
            None if pos.is_checkmate() => Eval::Mate {
                winner: side(pos.turn().other()),
                moves: 0,
            },
            None => Eval::DRAW,
        };
        infos.push(PositionInfo {
            eval,
            analysis,
            is_draw: draws[i],
        });
    }
    Ok(infos)
}

/// Evaluation of the position after a line's first move: for the mover's own
/// mates, one of its moves has already been played.
fn eval_after_first_move(line: &EngineLine, mover: Side) -> Eval {
    match line.eval {
        Eval::Mate { winner, moves } if winner == mover => Eval::Mate {
            winner,
            moves: moves.saturating_sub(1),
        },
        other => other,
    }
}

pub fn to_san_line(start: &Chess, pv: &[String], limit: usize) -> Vec<MoveRef> {
    let mut pos = start.clone();
    let mut out = Vec::new();
    for uci in pv.iter().take(limit) {
        let Some(m) = board::parse_uci(&pos, uci) else {
            break;
        };
        let san = SanPlus::from_move_and_play_unchecked(&mut pos, m).to_string();
        out.push(MoveRef {
            san,
            uci: uci.clone(),
            fen: board::fen(&pos),
        });
    }
    out
}

fn phases(game: &ParsedGame) -> Vec<Phase> {
    let mut current = Phase::Opening;
    game.positions
        .iter()
        .enumerate()
        .map(|(ply, pos)| {
            current = current.max(board::raw_phase(pos.board(), ply as u32));
            current
        })
        .collect()
}

struct MoveOutcome {
    ply: PlyReview,
    scored: ScoredMove,
    ep_lost: f64,
}

fn build_review(
    request: &ReviewRequest,
    infos: &[PositionInfo],
    openings: &OpeningBook,
    engine: &str,
) -> ReviewResult {
    let game = &request.game;
    let cfg = &request.config;
    let white_elo = game.elo(Color::White);
    let black_elo = game.elo(Color::Black);
    let rating = rating_for_expected_points(white_elo, black_elo, cfg);
    let highest_elo = white_elo.unwrap_or(0).max(black_elo.unwrap_or(0));
    let phases = phases(game);
    let horizon = cfg.material_horizon_plies;

    let mut caps = Caps2State::default();
    let mut outcomes: Vec<MoveOutcome> = Vec::with_capacity(game.moves.len());
    let mut in_book = true;
    let mut opponent_previous_ep_lost: Option<f64> = None;

    for (i, parsed) in game.moves.iter().enumerate() {
        let before = &game.positions[i];
        let after = &game.positions[i + 1];
        let m = parsed.mv;
        let mover_color = before.turn();
        let mover = side(mover_color);
        let opponent = mover.opponent();
        let analysis = infos[i]
            .analysis
            .as_ref()
            .expect("a position with a played move was searched");
        let lines = &analysis.lines;
        let best_line = &lines[0];

        // Scores of the position after the played move and after the best move.
        let played_rank = lines.iter().position(|l| l.pv.first() == Some(&parsed.uci));
        let played_is_best = played_rank == Some(0);
        let best_eval = eval_after_first_move(best_line, mover);
        let played_eval = match played_rank {
            Some(0) => best_eval,
            Some(k) => eval_after_first_move(&lines[k], mover),
            None => infos[i + 1].eval,
        };
        let best_value = Value::after_move(best_eval, mover);
        let played_value = Value::after_move(played_eval, mover);
        let played_is_draw = infos[i + 1].is_draw && !after.is_checkmate();
        let best_move = board::parse_uci(before, &best_line.pv[0]);
        let best_is_draw = best_move.is_some_and(|bm| {
            let mut p = before.clone();
            p.play_unchecked(bm);
            board::is_board_draw(&p)
        });

        let ep_played = node_expected_points(played_value, played_is_draw, rating);
        let ep_best = node_expected_points(best_value, best_is_draw, rating);
        let mover_value_before = Value::after_move(infos[i].eval, mover);
        let ep_before = node_expected_points(mover_value_before, infos[i].is_draw, rating);
        let opponent_value_before = Value::after_move(infos[i].eval, opponent);
        let opponent_ep_before =
            node_expected_points(opponent_value_before, infos[i].is_draw, rating);

        let is_forced = before.legal_moves().len() == 1;
        in_book = in_book && openings.contains(after);
        let is_book = in_book;

        // Material along the engine lines.
        let played_pv: Vec<String> = match played_rank {
            Some(k) => lines[k].pv.clone(),
            None => std::iter::once(parsed.uci.clone())
                .chain(
                    infos[i + 1]
                        .analysis
                        .as_ref()
                        .and_then(PositionAnalysis::best)
                        .map(|l| l.pv.clone())
                        .unwrap_or_default(),
                )
                .collect(),
        };
        let played_walk = walk_line(before, mover_color, &played_pv, horizon);
        let best_walk = walk_line(before, mover_color, &best_line.pv, horizon);
        let material_lost = played_walk.lost();
        let best_gain = best_walk.net();

        let mut facts = MoveFacts {
            played: played_value,
            best: best_value,
            played_is_draw,
            best_is_draw,
            played_is_best_move: played_is_best,
            rating,
            is_book,
            is_forced,
            scored: true,
            has_siblings: lines.len() > 1,
            events: MoveEvents::default(),
            material: MaterialFacts {
                loses_material_or_mated: material_lost >= cfg.blunder_material_threshold_cp
                    || played_value.mate_moves().is_some_and(|n| n < 0),
                // A margin keeps PV noise in lost positions (where every line sheds
                // material) from flipping this fact.
                loses_more_material_than_best: played_walk.end
                    <= best_walk.end - cfg.forgiving_material_lost_cp,
                // In forced-mate lines material is meaningless; the mate rules decide.
                material_lost_cp: (!played_value.is_mate()).then_some(material_lost),
                drops_material: !played_is_best
                    && material_lost >= 100
                    && material_lost > best_gain.max(0),
            },
            opponent_previous_ep_lost,
            missed: MissedChances::default(),
        };
        let grade = classification::precision(&facts, cfg);
        let good_grade = matches!(grade, Some(Precision::Best | Precision::Excellent));
        let phase_after = phases[i + 1];
        let endgame_gain_ok = |walk: &LineMaterial, min: i32| {
            phase_after != Phase::Endgame || walk.max - walk.start >= min
        };
        let previous_move = i.checked_sub(1).map(|p| game.moves[p].mv);
        let recapture = is_recapture(previous_move, m);
        let oc = |v: Value| expected_outcome(v, rating, cfg);
        let mover_completely_winning_before =
            oc(mover_value_before) == ExpectedOutcome::CompletelyWinning;

        // Brilliant: the engine's best move gives up a piece (static exchange), the
        // next-best move is clearly worse, and the recovered gates hold.
        let bc = &cfg.brilliant;
        let second_value = lines
            .get(1)
            .map(|l| Value::after_move(eval_after_first_move(l, mover), mover));
        let second_drops_bucket = second_value.is_some_and(|v| oc(v) < oc(played_value));
        let second_clearly_worse = second_value.is_some_and(|v| {
            ep_played - node_expected_points(v, false, rating) >= bc.min_gap_to_second_best
        }) || second_drops_bucket;
        let played_mate = played_value.mate_moves();
        let sacrifice = played_is_best
            .then(|| static_sacrifice(before, m, bc))
            .flatten();
        let opponent_reply_captures = played_pv
            .get(1)
            .and_then(|uci| board::parse_uci(after, uci))
            .is_some_and(|reply| reply.is_capture());
        let balance_after = board::balance(after.board(), mover_color);
        facts.events.brilliant = bc.enabled
            && played_is_best
            && second_clearly_worse
            && !played_mate.is_some_and(|n| n < 0)
            && !(bc.exclude_mating_moves && played_mate.is_some_and(|n| n >= 0))
            && ep_before >= bc.min_ep_before
            && ep_before < bc.max_ep_before
            && ep_played >= bc.min_ep_after
            && sacrifice.is_some_and(|s| {
                // Material is really lost (implied by the SEE threshold, kept explicit).
                (opponent_reply_captures || s.see_gain >= 100)
                    && (material_gate(balance_after, see::value(s.role))
                        || opponent_ep_before >= 0.10)
            })
            && (endgame_gain_ok(&played_walk, bc.endgame_min_gain_cp) || second_drops_bucket);

        // Great Find: the only good move in a position that is not already won.
        let other_ep = if played_is_best {
            lines.get(1).map(|l| {
                let v = Value::after_move(eval_after_first_move(l, mover), mover);
                node_expected_points(v, false, rating)
            })
        } else {
            None
        };
        facts.events.great_find = cfg.great_find.enabled
            && !facts.events.brilliant
            && good_grade
            && i > 0
            && other_ep
                .is_some_and(|other| ep_played - other >= cfg.great_find.min_gap_to_second_best)
            && (!cfg.great_find.require_high_rated
                || highest_elo as i32 >= cfg.great_find.high_rated_min)
            && !takes_free_piece(&played_walk, 100)
            && !recapture
            && !mover_completely_winning_before
            && endgame_gain_ok(&played_walk, cfg.brilliant.endgame_min_gain_cp);

        // What the best line would have achieved (used only for Mistake/Blunder).
        facts.missed = MissedChances {
            free_piece: takes_free_piece(&best_walk, cfg.miss.free_piece_min_value_cp),
            tactic: best_gain >= cfg.miss.tactic_min_gain_cp,
            win: oc(best_value) >= ExpectedOutcome::Winning
                && oc(played_value) <= ExpectedOutcome::Equal,
            comeback: oc(mover_value_before) <= ExpectedOutcome::Worse
                && oc(best_value) >= ExpectedOutcome::Equal
                && oc(played_value) <= ExpectedOutcome::Worse,
            brilliant: best_move.is_some_and(|bm| static_sacrifice(before, bm, bc).is_some())
                && ep_best >= bc.min_ep_after,
            opportunity: opponent_previous_ep_lost
                .is_some_and(|l| l >= cfg.miss.opportunity_min_opponent_ep_lost),
        };

        let outcome = classification::classify(&facts, cfg);
        opponent_previous_ep_lost = Some(outcome.ep_lost);

        let accuracy = caps.score(&Caps2Move {
            mover,
            played: played_value,
            best: best_value,
            is_book,
            is_forced,
        });
        let mover_completely_lost_before = oc(opponent_value_before)
            == ExpectedOutcome::CompletelyWinning
            && board::balance(before.board(), mover_color.other()) >= 300;

        let is_key_moment = good_grade
            && !is_forced
            && !is_book
            && i > 0
            && !mover_completely_winning_before
            && !recapture
            && (played_walk.net() >= 200
                || played_value.mate_moves().is_some_and(|n| n >= 0)
                || m.is_promotion()
                || takes_free_piece(&played_walk, 300));
        let is_critical = matches!(
            outcome.classification,
            Some(
                Classification::Blunder
                    | Classification::Mistake
                    | Classification::Miss
                    | Classification::Brilliant
                    | Classification::GreatFind
            )
        );

        let best_ref = to_san_line(before, &best_line.pv, 1).into_iter().next();
        let explanation = explain::describe(&explain::MoveContext {
            mover,
            classification: outcome.classification,
            miss_kind: outcome.miss_kind,
            played_is_best,
            best_san: best_ref.as_ref().map(|r| r.san.as_str()),
            played: played_value,
            best: best_value,
            ep_before,
            ep_after: ep_played,
            material_lost_cp: material_lost,
            best_gain_cp: best_gain,
            opening: if is_book { openings.name(after) } else { None },
        });

        let (from, to) = board::move_squares(m);
        let ply = PlyReview {
            ply: i as u32 + 1,
            move_number: before.fullmoves().get(),
            mover,
            san: parsed.san.clone(),
            uci: parsed.uci.clone(),
            from: from.map(|s| s.to_string()),
            to: to.to_string(),
            fen_before: board::fen(before),
            fen_after: board::fen(after),
            flags: move_flags(m, after),
            eval_before: infos[i].eval,
            eval_after: infos[i + 1].eval,
            eval_best: Some(best_eval),
            ep_before,
            ep_after: ep_played,
            ep_best,
            ep_lost: outcome.ep_lost,
            classification: outcome.classification,
            miss_kind: outcome.miss_kind,
            precision: outcome.precision,
            rule: outcome.rule,
            show_best_move: outcome
                .classification
                .is_some_and(Classification::shows_best_move)
                && !played_is_best,
            best_move: best_ref,
            played_is_best,
            lines: lines
                .iter()
                .map(|l| Line {
                    rank: u32::from(l.rank),
                    eval: l.eval,
                    depth: l.depth,
                    moves: to_san_line(before, &l.pv, LINE_DISPLAY_PLIES),
                })
                .collect(),
            accuracy,
            phase: phase_after,
            is_book,
            is_key_moment,
            is_critical,
            material_lost_cp: (material_lost > 0).then_some(material_lost),
            explanation,
            pgn_comment: parsed.comment.clone(),
        };
        outcomes.push(MoveOutcome {
            scored: ScoredMove {
                mover,
                phase: phase_after,
                score: accuracy,
                mover_completely_lost_before,
            },
            ep_lost: outcome.ep_lost,
            ply,
        });
    }

    summarise(request, infos, outcomes, openings, engine, rating)
}

fn move_flags(m: Move, after: &Chess) -> MoveFlags {
    MoveFlags {
        check: after.is_check(),
        checkmate: after.is_checkmate(),
        stalemate: after.is_stalemate(),
        capture: m.is_capture(),
        castle: m.is_castle(),
        promotion: m.is_promotion(),
        en_passant: m.is_en_passant(),
    }
}

fn summarise(
    request: &ReviewRequest,
    infos: &[PositionInfo],
    outcomes: Vec<MoveOutcome>,
    openings: &OpeningBook,
    engine: &str,
    rating: i32,
) -> ReviewResult {
    let game = &request.game;
    let cfg = &request.config;
    let scored: Vec<ScoredMove> = outcomes.iter().map(|o| o.scored).collect();
    let [white_acc, black_acc] = caps2::aggregate(&scored, cfg.caps_exclude_completely_lost);

    let side_summary = |s: Side, accuracy| {
        let classes: Vec<Classification> = outcomes
            .iter()
            .filter(|o| o.ply.mover == s)
            .filter_map(|o| o.ply.classification)
            .collect();
        let count = |c: Classification| classes.iter().filter(|x| **x == c).count() as u32;
        let color = if s == Side::White {
            Color::White
        } else {
            Color::Black
        };
        SideSummary {
            side: s,
            name: game
                .header(if s == Side::White { "White" } else { "Black" })
                .map(str::to_owned),
            elo: game.elo(color),
            accuracy,
            inaccuracies: count(Classification::Inaccuracy),
            mistakes: count(Classification::Mistake),
            blunders: count(Classification::Blunder),
            misses: count(Classification::Miss),
            tally: tally(classes.iter().copied()),
        }
    };

    let graph = infos
        .iter()
        .enumerate()
        .map(|(i, info)| GraphPoint {
            ply: i as u32,
            eval: info.eval,
            white_expected_points: node_expected_points(
                Value::after_move(info.eval, Side::White),
                info.is_draw,
                rating,
            ),
            classification: i
                .checked_sub(1)
                .and_then(|p| outcomes[p].ply.classification),
        })
        .collect();

    let mut swings: Vec<(u32, f64)> = outcomes
        .iter()
        .filter(|o| o.ep_lost >= 0.10)
        .map(|o| (o.ply.ply, o.ep_lost))
        .collect();
    swings.sort_by(|a, b| b.1.total_cmp(&a.1));

    let phase_start = |phase: Phase| {
        outcomes
            .iter()
            .find(|o| o.ply.phase >= phase)
            .map(|o| o.ply.ply)
    };
    let opening = game
        .positions
        .iter()
        .rev()
        .find_map(|p| openings.name(p))
        .cloned();

    let mut notes = vec![
        format!(
            "Search: {engine}. The reference cloud review uses Stockfish 16.1 NNUE; scores near classification thresholds can differ."
        ),
        "Brilliant, Great Find and Miss subtypes use documented heuristics in place of the reference engine's unrecovered detectors.".to_owned(),
    ];
    if openings.is_empty() {
        notes.push("No opening book is loaded, so no move is classified as Book.".to_owned());
    } else {
        notes.push(
            "Book moves come from the public-domain Lichess opening list, not the reference book."
                .to_owned(),
        );
    }
    if game.elo(Color::White).is_none() && game.elo(Color::Black).is_none() {
        notes.push(format!(
            "The PGN has no ratings, so the win-probability curve uses rating {rating} (reference behaviour) and Great Find is disabled."
        ));
    }

    let header = |name: &str| game.header(name).map(str::to_owned);
    ReviewResult {
        game: GameInfo {
            index: game.index as u32,
            headers: game
                .headers
                .iter()
                .map(|(n, v)| Header {
                    name: n.clone(),
                    value: v.clone(),
                })
                .collect(),
            white: header("White"),
            black: header("Black"),
            white_elo: game.elo(Color::White),
            black_elo: game.elo(Color::Black),
            event: header("Event"),
            site: header("Site"),
            date: header("Date").or_else(|| header("UTCDate")),
            result: header("Result")
                .or_else(|| game.movetext_result.clone())
                .filter(|r| r != "*"),
            termination: header("Termination"),
            start_fen: game.start_fen(),
            final_fen: board::fen(game.final_position()),
            chess960: game.castling_mode == shakmaty::CastlingMode::Chess960,
            ply_count: game.moves.len() as u32,
            opening,
        },
        engine: engine.to_owned(),
        settings: ReviewSettingsUsed {
            limit: request.limit,
            multipv: u32::from(request.multipv),
        },
        rating_used: rating,
        config: cfg.clone(),
        initial_eval: infos[0].eval,
        white: side_summary(Side::White, white_acc),
        black: side_summary(Side::Black, black_acc),
        graph,
        key_moments: outcomes
            .iter()
            .filter(|o| o.ply.is_key_moment)
            .map(|o| o.ply.ply)
            .collect(),
        critical_moments: outcomes
            .iter()
            .filter(|o| o.ply.is_critical)
            .map(|o| o.ply.ply)
            .collect(),
        biggest_swings: swings.into_iter().take(5).map(|(p, _)| p).collect(),
        phase_starts: PhaseStarts {
            middlegame: phase_start(Phase::Middlegame),
            endgame: phase_start(Phase::Endgame),
        },
        notes,
        plies: outcomes.into_iter().map(|o| o.ply).collect(),
    }
}
