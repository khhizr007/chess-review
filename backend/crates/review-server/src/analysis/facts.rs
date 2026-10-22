//! The facts about one move and its classification, from the engine's view of the positions around
//! it. Shared by the review (`build_review`) and by sideline moves the reader plays
//! (`sideline::judge`, docs/explore-mode.md §5.5), so the two cannot drift.

use review_core::classification::{
    self, ClassificationOutcome, MaterialFacts, MissedChances, MoveEvents, MoveFacts,
};
use review_core::expected_points::{ExpectedOutcome, expected_outcome, node_expected_points};
use review_core::{Eval, Phase, Precision, ReviewConfig, Side, Value};
use shakmaty::{Chess, Move, Position};

use super::detectors::{
    LineMaterial, Sacrifice, captures_free_piece, is_only_escape, is_quiet_retreat, is_recapture,
    material_gate, static_sacrifice, takes_free_piece, threatens_promotion, walk_line,
};
use super::see;
use super::{eval_after_first_move, line_input};
use crate::board::{self, side};
use crate::coach::{self, CoachNote};
use crate::engine::EngineLine;

/// One move to classify, with the engine's view of the positions around it.
pub(crate) struct MoveInput<'a> {
    pub before: &'a Chess,
    pub after: &'a Chess,
    pub mv: Move,
    pub uci: &'a str,
    /// The move before it (a recapture test), if any.
    pub previous: Option<Move>,
    /// Engine lines of `before`, best first (at least one).
    pub lines: &'a [EngineLine],
    /// Evaluation of `before`, and whether `before` is drawn.
    pub eval_before: Eval,
    pub before_is_draw: bool,
    /// Evaluation of `after`, its best line if it was searched, and whether `after` is drawn.
    pub eval_after: Eval,
    pub after_line: Option<&'a EngineLine>,
    pub after_is_draw: bool,
    pub is_book: bool,
    pub phase_after: Phase,
    /// The move is not the game's first (Great Find needs a position that was not chosen).
    pub has_previous_move: bool,
    /// Best move of a shallow search of `before`, where one was made.
    pub shallow_best: Option<&'a str>,
    /// Expected points the opponent lost on their previous move, if known.
    pub opponent_previous_ep_lost: Option<f64>,
    /// Rating for the expected-points curve, and the higher of the two players' Elo (0 if unknown).
    pub rating: i32,
    pub highest_elo: u32,
}

/// The facts found for a move, its classification, and the values the review goes on to use.
pub(crate) struct MoveJudgement {
    pub mover: Side,
    pub played_rank: Option<usize>,
    pub played_is_best: bool,
    pub best_eval: Eval,
    pub best_value: Value,
    pub played_value: Value,
    pub mover_value_before: Value,
    pub opponent_value_before: Value,
    pub ep_before: f64,
    pub ep_played: f64,
    pub ep_best: f64,
    pub best_move: Option<Move>,
    /// The played move's line: its engine line, or the move and the reply search.
    pub played_pv: Vec<String>,
    pub played_walk: LineMaterial,
    pub material_lost: i32,
    pub best_gain: i32,
    pub sacrifice: Option<Sacrifice>,
    /// Pawns count as sacrifices (low-rated games).
    pub pawns: bool,
    pub good_grade: bool,
    pub recapture: bool,
    pub mover_completely_winning_before: bool,
    pub facts: MoveFacts,
    pub outcome: ClassificationOutcome,
}

/// Builds the facts about a move and classifies it with the recovered rules.
pub(crate) fn judge_move(input: &MoveInput<'_>, cfg: &ReviewConfig) -> MoveJudgement {
    let MoveInput {
        before,
        after,
        mv: m,
        uci,
        lines,
        rating,
        ..
    } = *input;
    let mover_color = before.turn();
    let mover = side(mover_color);
    let opponent = mover.opponent();
    let horizon = cfg.material_horizon_plies;
    let best_line = &lines[0];

    // Scores of the position after the played move and after the best move.
    let played_rank = lines
        .iter()
        .position(|l| l.pv.first().map(String::as_str) == Some(uci));
    let played_is_best = played_rank == Some(0);
    let best_eval = eval_after_first_move(best_line, mover);
    let played_eval = match played_rank {
        Some(0) => best_eval,
        Some(k) => eval_after_first_move(&lines[k], mover),
        None => input.eval_after,
    };
    let best_value = Value::after_move(best_eval, mover);
    let played_value = Value::after_move(played_eval, mover);
    let played_is_draw = input.after_is_draw && !after.is_checkmate();
    let best_move = board::parse_uci(before, &best_line.pv[0]);
    let best_is_draw = best_move.is_some_and(|bm| {
        let mut p = before.clone();
        p.play_unchecked(bm);
        board::is_board_draw(&p)
    });

    let ep_played = node_expected_points(played_value, played_is_draw, rating);
    let ep_best = node_expected_points(best_value, best_is_draw, rating);
    let mover_value_before = Value::after_move(input.eval_before, mover);
    let ep_before = node_expected_points(mover_value_before, input.before_is_draw, rating);
    let opponent_value_before = Value::after_move(input.eval_before, opponent);
    let opponent_ep_before =
        node_expected_points(opponent_value_before, input.before_is_draw, rating);

    let is_forced = before.legal_moves().len() == 1;
    let is_book = input.is_book;

    // Material along the engine lines.
    let played_pv: Vec<String> = match played_rank {
        Some(k) => lines[k].pv.clone(),
        None => std::iter::once(uci.to_owned())
            .chain(input.after_line.map(|l| l.pv.clone()).unwrap_or_default())
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
        opponent_previous_ep_lost: input.opponent_previous_ep_lost,
        missed: MissedChances::default(),
    };
    let grade = classification::precision(&facts, cfg);
    let good_grade = matches!(grade, Some(Precision::Best | Precision::Excellent));
    let endgame_gain_ok = |walk: &LineMaterial, min: i32| {
        input.phase_after != Phase::Endgame || walk.max - walk.start >= min
    };
    let recapture = is_recapture(input.previous, m);
    let oc = |v: Value| expected_outcome(v, rating, cfg);
    let mover_completely_winning_before =
        oc(mover_value_before) == ExpectedOutcome::CompletelyWinning;

    // Brilliant (Chess.com: "a good piece sacrifice"): the engine's best move
    // gives up material (static exchange), the mover is not in a bad position
    // after it and would not be completely winning without it, and the
    // recovered gates hold.
    let bc = &cfg.brilliant;
    let second_value = lines
        .get(1)
        .map(|l| Value::after_move(eval_after_first_move(l, mover), mover));
    let second_drops_bucket = second_value.is_some_and(|v| oc(v) < oc(played_value));
    let second_clearly_worse = bc.min_gap_to_second_best <= 0.0
        || second_drops_bucket
        || second_value.is_some_and(|v| {
            ep_played - node_expected_points(v, false, rating) >= bc.min_gap_to_second_best
        });
    // A forced mate on the second line is not "winning anyway": Chess.com rates
    // mating sacrifices Brilliant even when other moves mate too.
    let winning_without_it = second_value.is_some_and(|v| {
        !v.mate_moves().is_some_and(|n| n > 0)
            && node_expected_points(v, false, rating) >= bc.max_alternative_ep
    });
    let played_mate = played_value.mate_moves();
    let pawns =
        bc.pawn_sacrifice_below_elo > 0 && (input.highest_elo as i32) < bc.pawn_sacrifice_below_elo;
    let sacrifice = played_is_best
        .then(|| static_sacrifice(before, m, bc, pawns))
        .flatten();
    let balance_after = board::balance(after.board(), mover_color);
    facts.events.brilliant = bc.enabled
        && played_is_best
        && second_clearly_worse
        && !winning_without_it
        && !played_mate.is_some_and(|n| n < 0)
        && !(bc.exclude_mating_moves && played_mate.is_some_and(|n| n >= 0))
        && ep_played >= bc.min_ep_after
        && sacrifice.is_some_and(|s| {
            material_gate(balance_after, see::value(s.role)) || opponent_ep_before >= 0.10
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
    // Recovered exclusions: easy to find (only escape, free capture, promoting
    // pawn, or a shallow search already finds it) and obvious quiet retreats.
    let gf = &cfg.great_find;
    let easy_to_find = is_only_escape(before, m)
        || captures_free_piece(before, m, 100)
        || threatens_promotion(before, m)
        || input.shallow_best == Some(uci);
    let obvious_retreat = is_quiet_retreat(before, m)
        && played_walk.max - played_walk.start < gf.quiet_retreat_min_stake_cp;
    facts.events.great_find = gf.enabled
        && !facts.events.brilliant
        && good_grade
        && input.has_previous_move
        && other_ep.is_some_and(|other| ep_played - other >= gf.min_gap_to_second_best)
        && (!gf.require_high_rated || input.highest_elo as i32 >= gf.high_rated_min)
        && !takes_free_piece(&played_walk, 100)
        && !easy_to_find
        && !obvious_retreat
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
        brilliant: best_move.is_some_and(|bm| static_sacrifice(before, bm, bc, pawns).is_some())
            && ep_best >= cfg.miss.brilliant_min_best_ep,
        opportunity: input
            .opponent_previous_ep_lost
            .is_some_and(|l| l >= cfg.miss.opportunity_min_opponent_ep_lost),
    };

    let outcome = classification::classify(&facts, cfg);
    MoveJudgement {
        mover,
        played_rank,
        played_is_best,
        best_eval,
        best_value,
        played_value,
        mover_value_before,
        opponent_value_before,
        ep_before,
        ep_played,
        ep_best,
        best_move,
        played_pv,
        played_walk,
        material_lost,
        best_gain,
        sacrifice,
        pawns,
        good_grade,
        recapture,
        mover_completely_winning_before,
        facts,
        outcome,
    }
}

/// What the coach needs besides the move's judgement: the null-move threat searches around the
/// move, and who is being spoken to.
pub(crate) struct CoachContext<'a> {
    /// The threat search after the move (its mover to move again).
    pub threat: Option<&'a EngineLine>,
    /// The threat search before the move (the opponent to move).
    pub prev_threat: Option<&'a EngineLine>,
    /// The threat search after the engine's best move, when it is not the played move.
    pub best_threat: Option<&'a EngineLine>,
    /// Rating for the coach's rating gates (`None` says every tag).
    pub rating: Option<i32>,
    pub difficulty: Option<coach::Difficulty>,
    /// The side the reader played, if known (the coach's voice).
    pub player_side: Option<Side>,
    /// Picks among wordings (the ply index keeps it stable).
    pub seed: u32,
}

/// The coach's note for a judged move (docs/coach-analysis.md), shared by the review and sidelines.
pub(crate) fn coach_note(
    input: &MoveInput<'_>,
    judged: &MoveJudgement,
    ctx: &CoachContext<'_>,
    cfg: &ReviewConfig,
) -> CoachNote {
    let lines = input.lines;
    let mover = judged.mover;
    let rating = input.rating;
    let oc = |v: Value| expected_outcome(v, rating, cfg);
    // The played line from `before`: a mate the mover gives after the reply
    // search is one move longer from here.
    let played_line_eval = match judged.played_rank {
        Some(k) => lines[k].eval,
        None => match input.eval_after {
            Eval::Mate { winner, moves } if winner == mover => Eval::Mate {
                winner,
                moves: moves + 1,
            },
            other => other,
        },
    };
    // The coach's "only good move": every other engine line loses more
    // than a Good move may.
    let only_move = lines.get(1).is_some_and(|l| {
        let v = Value::after_move(eval_after_first_move(l, mover), mover);
        judged.ep_best - node_expected_points(v, false, rating) > cfg.precision_thresholds[1]
    });
    let line = line_input;
    coach::coach_move(&coach::CoachInput {
        before: input.before,
        played: input.mv,
        previous: input.previous,
        reply: input.after_line.map(line),
        best: Some(line(&lines[0])),
        played_line: Some(coach::LineInput {
            pv: &judged.played_pv,
            eval: played_line_eval,
        }),
        threat: ctx.threat.map(line),
        prev_threat: ctx.prev_threat.map(line),
        best_threat: ctx.best_threat.map(line),
        only_move,
        sacrifice: judged.sacrifice.map(|s| s.square),
        best_sacrifice: judged
            .best_move
            .filter(|_| !judged.played_is_best)
            .and_then(|bm| static_sacrifice(input.before, bm, &cfg.brilliant, judged.pawns))
            .map(|s| s.square),
        swing: Some((oc(judged.mover_value_before), oc(judged.played_value))),
        classification: judged.outcome.classification,
        phase: input.phase_after,
        rating: ctx.rating,
        difficulty: ctx.difficulty,
        style: coach::Style::for_player(ctx.player_side, mover),
        horizon: cfg.material_horizon_plies,
        seed: ctx.seed,
    })
}
