//! Review pipeline: engine searches → facts → recovered rules → summary.

pub mod detectors;
pub mod explain;
mod facts;
pub mod model;
pub mod see;
pub mod sideline;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use futures::StreamExt;
use review_core::caps2::{self, Caps2Move, Caps2State, ScoredMove};
use review_core::expected_points::{
    ExpectedOutcome, expected_outcome, node_expected_points, rating_for_expected_points,
};
use review_core::summary::tally;
use review_core::{Classification, Eval, Phase, ReviewConfig, Side, Value};
use shakmaty::san::SanPlus;
use shakmaty::{Chess, Color, Move, Position};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::board::{self, side};
use crate::coach;
use crate::engine::{
    AnalysisRequest, EngineError, EngineLine, Evaluator, PositionAnalysis, SearchLimit,
};
use crate::human::{HumanModel, MoveQuery};
use crate::openings::OpeningBook;
use crate::pgn::ParsedGame;
use detectors::takes_free_piece;
use model::*;

/// Moves shown per engine line in the UI.
const LINE_DISPLAY_PLIES: usize = 10;

#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub game: ParsedGame,
    pub limit: SearchLimit,
    pub multipv: u8,
    pub config: ReviewConfig,
    /// The side the player played, if known (the coach's voice).
    pub player_side: Option<Side>,
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
    /// Best move of a shallow search, where the played move is the best move.
    shallow_best: Option<String>,
    /// Best line of the null-move threat search: this position with the other
    /// side to move (for the coach).
    threat: Option<EngineLine>,
    /// Best line of the null-move threat search after the engine's best move
    /// from this position, its mover to move again (for the coach, where the
    /// played move is not the best move).
    best_threat: Option<EngineLine>,
}

pub async fn run_review(
    request: ReviewRequest,
    evaluator: Arc<dyn Evaluator>,
    openings: Arc<OpeningBook>,
    cancel: CancellationToken,
    progress: ProgressFn,
) -> Result<ReviewResult, ReviewError> {
    run_review_with_model(request, evaluator, None, openings, cancel, progress).await
}

/// [`run_review`] with a human-move model for the coach's difficulty wording
/// (`crate::human`). A failing model is logged and the review goes on
/// without it.
pub async fn run_review_with_model(
    request: ReviewRequest,
    evaluator: Arc<dyn Evaluator>,
    human: Option<Arc<dyn HumanModel>>,
    openings: Arc<OpeningBook>,
    cancel: CancellationToken,
    progress: ProgressFn,
) -> Result<ReviewResult, ReviewError> {
    let game = &request.game;
    if game.moves.is_empty() {
        return Err(ReviewError::NoMoves);
    }
    let book = book_flags(game, &openings, &request.config);
    let positions =
        analyse_positions(&request, &book, evaluator.as_ref(), &cancel, progress).await?;
    let odds = match human {
        Some(model) => human_odds(&request, &positions, &book, model.as_ref()).await,
        None => Vec::new(),
    };
    Ok(build_review(
        &request,
        &positions,
        &openings,
        &book,
        &odds,
        &evaluator.name(),
    ))
}

/// Queries per call to the human-move model.
const HUMAN_BATCH: usize = 256;
/// The human-move model is asked from this move number on. Maia-2 was trained
/// without the first ten moves of each game (`first_n_moves: 10` in its
/// config), and its predictions there are not sound (it rates 1.e4 below
/// 1.Na3 at 1200).
const HUMAN_FIRST_MOVE: u32 = 11;

/// Per move (book moves and the first ten moves excluded): how likely a
/// player of the mover's rating was to play it and the engine's best move.
/// Ratings come from the PGN, else the rating used for expected points.
/// Empty when the model fails.
async fn human_odds(
    request: &ReviewRequest,
    infos: &[PositionInfo],
    book: &[bool],
    model: &dyn HumanModel,
) -> Vec<Option<coach::Difficulty>> {
    let game = &request.game;
    let rating = rating_for_expected_points(
        game.elo(Color::White),
        game.elo(Color::Black),
        &request.config,
    );
    let elo = |c: Color| game.elo(c).unwrap_or(rating.max(0) as u32);
    let mut plies = Vec::new();
    let mut queries = Vec::new();
    for (i, parsed) in game.moves.iter().enumerate() {
        let best = infos[i]
            .analysis
            .as_ref()
            .and_then(PositionAnalysis::best)
            .and_then(|l| l.pv.first());
        let early = game.positions[i].fullmoves().get() < HUMAN_FIRST_MOVE;
        let (Some(best), false, false) = (best, book[i], early) else {
            continue;
        };
        let mover = game.positions[i].turn();
        plies.push(i);
        queries.push(MoveQuery {
            fen: board::fen(&game.positions[i]),
            moves: vec![parsed.uci.clone(), best.clone()],
            elo_self: elo(mover),
            elo_oppo: elo(mover.other()),
        });
    }
    let mut out = vec![None; game.moves.len()];
    for (chunk, at) in queries.chunks(HUMAN_BATCH).zip(plies.chunks(HUMAN_BATCH)) {
        match model.odds(chunk).await {
            Ok(odds) => {
                for (i, p) in at.iter().zip(odds) {
                    if let [played, best] = p[..] {
                        out[*i] = Some(coach::Difficulty { played, best });
                    }
                }
            }
            Err(e) => {
                tracing::warn!("human-move model: {e}; the coach goes on without it");
                return Vec::new();
            }
        }
    }
    out
}

/// Per move: the position after it is a book position.
fn book_flags(game: &ParsedGame, openings: &OpeningBook, cfg: &ReviewConfig) -> Vec<bool> {
    let mut in_book = true;
    game.positions[1..]
        .iter()
        .map(|after| {
            in_book = in_book && openings.contains(after);
            // The reference also marks a later move Book when play transposes back
            // into the book; `book_requires_unbroken` keeps the stricter fallback.
            if cfg.book_requires_unbroken {
                in_book
            } else {
                openings.contains(after)
            }
        })
        .collect()
}

/// Searches every non-terminal position (in parallel up to the evaluator's
/// limit), and the null-move threat searches the coach needs.
async fn analyse_positions(
    request: &ReviewRequest,
    book: &[bool],
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
    let chess960 = game.castling_mode == shakmaty::CastlingMode::Chess960;

    // The coach's null-move threat search, on the positions before and after
    // every move that is neither book nor forced: the position after move i
    // with its mover to move again (THREATENS for move i), which is also the
    // position before move i + 1 with its mover's opponent to move (PREVENTS).
    let threat_depth = request.config.coach.threat_search_depth;
    let coached: Vec<bool> = game.positions[..game.moves.len()]
        .iter()
        .zip(book)
        .map(|(pos, book)| !book && pos.legal_moves().len() > 1)
        .collect();
    let threat_work: Vec<Option<AnalysisRequest>> = game
        .positions
        .iter()
        .enumerate()
        .map(|(j, pos)| {
            let wanted = threat_depth > 0
                && needs_search[j]
                && (j.checked_sub(1).is_some_and(|p| coached[p])
                    || coached.get(j).copied().unwrap_or(false));
            let null = board::null_move(pos).filter(|_| wanted)?;
            Some(AnalysisRequest {
                fen: board::fen(&null),
                limit: SearchLimit::Depth {
                    depth: threat_depth,
                },
                multipv: 1,
                chess960,
                searchmoves: None,
            })
        })
        .collect();

    let total =
        (needs_search.iter().filter(|n| **n).count() + threat_work.iter().flatten().count()) as u32;
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
                chess960,
                searchmoves: None,
            })
        })
        .collect();
    let done = &done;
    let progress = &progress;
    let searched = work.len();
    let searches =
        futures::stream::iter(work.into_iter().chain(threat_work).map(|req| async move {
            let Some(req) = req else { return Ok(None) };
            let analysis = evaluator.analyze(&req, cancel).await?;
            let n = done.fetch_add(1, Ordering::SeqCst) + 1;
            progress(n, total);
            Ok::<_, EngineError>(Some(analysis))
        }))
        .buffered(evaluator.parallelism().max(1))
        .collect::<Vec<_>>()
        .await;
    let mut results = Vec::with_capacity(searches.len());
    for (k, result) in searches.into_iter().enumerate() {
        match result {
            Ok(a) => results.push(a),
            Err(EngineError::Cancelled) => return Err(ReviewError::Cancelled),
            // A failed threat search only costs the coach a sentence.
            Err(e) if k >= searched => {
                tracing::warn!(error = %e, "null-move threat search failed");
                results.push(None);
            }
            Err(e) => return Err(ReviewError::Engine(e)),
        }
    }
    if cancel.is_cancelled() {
        return Err(ReviewError::Cancelled);
    }
    let threats = results.split_off(searched);

    let mut infos = Vec::with_capacity(results.len());
    for (i, (analysis, threat)) in results.into_iter().zip(threats).enumerate() {
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
            shallow_best: None,
            threat: threat.and_then(|a| a.best().cloned()),
            best_threat: None,
        });
    }

    // The coach's threat search after the best move B, where the played move
    // is not B: what B would have threatened (said in the best-move sentence).
    if threat_depth > 0 && request.config.coach.best_threat_search {
        let after_best: Vec<(usize, AnalysisRequest)> = game
            .moves
            .iter()
            .enumerate()
            .filter(|(i, _)| coached[*i])
            .filter_map(|(i, parsed)| {
                let best = infos[i].analysis.as_ref()?.best()?.pv.first()?;
                if *best == parsed.uci {
                    return None;
                }
                let mut pos = game.positions[i].clone();
                pos.play_unchecked(board::parse_uci(&pos, best)?);
                let null = board::null_move(&pos)?;
                Some((
                    i,
                    AnalysisRequest {
                        fen: board::fen(&null),
                        limit: SearchLimit::Depth {
                            depth: threat_depth,
                        },
                        multipv: 1,
                        chess960,
                        searchmoves: None,
                    },
                ))
            })
            .collect();
        let results = futures::stream::iter(
            after_best
                .into_iter()
                .map(|(i, req)| async move { (i, evaluator.analyze(&req, cancel).await) }),
        )
        .buffered(evaluator.parallelism().max(1))
        .collect::<Vec<_>>()
        .await;
        for (i, result) in results {
            match result {
                Ok(analysis) => infos[i].best_threat = analysis.best().cloned(),
                Err(EngineError::Cancelled) => return Err(ReviewError::Cancelled),
                // Like the other threat searches, a failure only costs a sentence.
                Err(e) => {
                    tracing::warn!(error = %e, "null-move threat search after the best move failed")
                }
            }
        }
        if cancel.is_cancelled() {
            return Err(ReviewError::Cancelled);
        }
    }

    // "Easy to find" proxy for Great Find: where the played move is the engine's
    // best move, does a shallow search already choose it?
    let easy_depth = request.config.great_find.easy_search_depth;
    if request.config.great_find.enabled && easy_depth > 0 {
        let shallow: Vec<(usize, AnalysisRequest)> = game
            .moves
            .iter()
            .enumerate()
            .filter(|(i, parsed)| {
                infos[*i]
                    .analysis
                    .as_ref()
                    .and_then(PositionAnalysis::best)
                    .is_some_and(|l| l.pv.first() == Some(&parsed.uci))
            })
            .map(|(i, _)| {
                (
                    i,
                    AnalysisRequest {
                        fen: board::fen(&game.positions[i]),
                        limit: SearchLimit::Depth { depth: easy_depth },
                        multipv: 1,
                        chess960,
                        searchmoves: None,
                    },
                )
            })
            .collect();
        let results = futures::stream::iter(shallow.into_iter().map(|(i, req)| async move {
            Ok::<_, EngineError>((i, evaluator.analyze(&req, cancel).await?))
        }))
        .buffered(evaluator.parallelism().max(1))
        .collect::<Vec<_>>()
        .await;
        for result in results {
            match result {
                Ok((i, analysis)) => {
                    infos[i].shallow_best = analysis.best().and_then(|l| l.pv.first().cloned());
                }
                Err(EngineError::Cancelled) => return Err(ReviewError::Cancelled),
                Err(e) => return Err(ReviewError::Engine(e)),
            }
        }
        if cancel.is_cancelled() {
            return Err(ReviewError::Cancelled);
        }
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
    book: &[bool],
    odds: &[Option<coach::Difficulty>],
    engine: &str,
) -> ReviewResult {
    let game = &request.game;
    let cfg = &request.config;
    let white_elo = game.elo(Color::White);
    let black_elo = game.elo(Color::Black);
    let rating = rating_for_expected_points(white_elo, black_elo, cfg);
    // The coach's rating gates: the player's own Elo when we know who they
    // are, else the rating used for expected points.
    let coach_rating = cfg.coach.rating_gates.then(|| {
        request
            .player_side
            .and_then(|s| {
                game.elo(if s == Side::White {
                    Color::White
                } else {
                    Color::Black
                })
            })
            .map_or(rating, |e| e as i32)
    });
    let highest_elo = white_elo.unwrap_or(0).max(black_elo.unwrap_or(0));
    let phases = phases(game);

    let mut caps = Caps2State::default();
    let mut outcomes: Vec<MoveOutcome> = Vec::with_capacity(game.moves.len());
    let mut opponent_previous_ep_lost: Option<f64> = None;

    for (i, parsed) in game.moves.iter().enumerate() {
        let before = &game.positions[i];
        let after = &game.positions[i + 1];
        let m = parsed.mv;
        let mover_color = before.turn();
        let analysis = infos[i]
            .analysis
            .as_ref()
            .expect("a position with a played move was searched");
        let lines = &analysis.lines;
        let best_line = &lines[0];
        let previous_move = i.checked_sub(1).map(|p| game.moves[p].mv);
        let phase_after = phases[i + 1];
        let input = facts::MoveInput {
            before,
            after,
            mv: m,
            uci: &parsed.uci,
            previous: previous_move,
            lines,
            eval_before: infos[i].eval,
            before_is_draw: infos[i].is_draw,
            eval_after: infos[i + 1].eval,
            after_line: infos[i + 1]
                .analysis
                .as_ref()
                .and_then(PositionAnalysis::best),
            after_is_draw: infos[i + 1].is_draw,
            is_book: book[i],
            phase_after,
            has_previous_move: i > 0,
            shallow_best: infos[i].shallow_best.as_deref(),
            opponent_previous_ep_lost,
            rating,
            highest_elo,
        };
        let judged = facts::judge_move(&input, cfg);
        let coach = facts::coach_note(
            &input,
            &judged,
            &facts::CoachContext {
                threat: infos[i + 1].threat.as_ref(),
                prev_threat: infos[i].threat.as_ref(),
                best_threat: infos[i].best_threat.as_ref(),
                rating: coach_rating,
                difficulty: odds.get(i).copied().flatten(),
                player_side: request.player_side,
                seed: i as u32,
            },
            cfg,
        );
        let facts::MoveJudgement {
            mover,
            played_is_best,
            best_eval,
            best_value,
            played_value,
            opponent_value_before,
            ep_before,
            ep_played,
            ep_best,
            played_walk,
            material_lost,
            best_gain,
            good_grade,
            recapture,
            mover_completely_winning_before,
            ref facts,
            outcome,
            ..
        } = judged;
        let is_book = facts.is_book;
        let is_forced = facts.is_forced;
        let oc = |v: Value| expected_outcome(v, rating, cfg);
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
            coach,
            pgn_comment: parsed.comment.clone(),
            clock_ms: parsed.clock_ms,
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

fn line_input(l: &EngineLine) -> coach::LineInput<'_> {
    coach::LineInput {
        pv: &l.pv,
        eval: l.eval,
    }
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
            player_side: request.player_side,
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
