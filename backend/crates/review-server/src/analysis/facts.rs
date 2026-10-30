//! The facts about one move and its classification, from the engine's view of the positions around
//! it. Shared by the review (`build_review`) and by sideline moves the reader plays
//! (`sideline::judge`, docs/explore-mode.md §5.5), so the two cannot drift.

use review_core::classification::{
    self, ClassificationOutcome, MaterialFacts, MissedChances, MoveEvents, MoveFacts,
};
use review_core::expected_points::{ExpectedOutcome, expected_outcome, node_expected_points};
use review_core::{Eval, MissKind, Phase, Precision, ReviewConfig, Side, Value};
use shakmaty::{Chess, Move, Position, Role, Square};

use super::detectors::{
    EarlierOwnMove, LineMaterial, Sacrifice, SacrificeKind, captures_free_piece,
    captures_promotion_threat, is_only_safe_square, is_passed_pawn_push, is_recapture,
    is_safe_promotion, lands_safely, material_gate, reference_ignored_threat, removes_attacker,
    sacrifice, takes_free_piece, threatens_promotion, walk_line,
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
    /// The two moves before this one, as Great needs them.
    pub previous_moves: PreviousMoves,
    /// Rating for the expected-points curve, and the higher of the two players' Elo (0 if unknown).
    pub rating: i32,
    pub highest_elo: u32,
    /// The same side's earlier moves, nearest first (`ignored_threats`' walk-back).
    pub own_history: &'a [EarlierOwnMove],
}

/// What the two moves before this one did: Great's key moments answer the opponent's error, and its
/// winning and comeback tags compare the mover's outcome with the one after their own previous
/// move (docs/calibration/great-blunder-analysis.md §2.1).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PreviousMoves {
    /// The opponent's previous move was graded Mistake or Blunder, missed a mate, or could not be
    /// graded (the reference's `precision(parent) >= 8`). False when there was no such move.
    pub opponent_erred: bool,
    /// The mover's value after their own previous move (the position before the opponent's move).
    pub own_value: Option<Value>,
    /// The opponent's value before the mover's own previous move (after the opponent's move
    /// before that).
    pub opponent_earlier_value: Option<Value>,
    /// The mover's own previous move carried a Great tag.
    pub own_great_tag: bool,
    /// The mover's own previous move already won material along its line (the reference's
    /// `is_any_previous_move_in_sequence_wins_material`): taking it now is no new key moment.
    pub own_wins_material: bool,
    /// The threatened piece the mover's own previous move left standing, if any: leaving it again
    /// is not a new sacrifice.
    pub own_ignored_threat: Option<Square>,
}

impl PreviousMoves {
    /// Whether a move's grade is an error a key moment answers (`precision >= 8`): Mistake or
    /// Blunder, a missed mate, or no grade at all.
    pub fn is_error(precision: Option<Precision>, miss_kind: Option<MissKind>) -> bool {
        matches!(
            precision,
            None | Some(Precision::Mistake | Precision::Blunder)
        ) || matches!(miss_kind, Some(MissKind::Mate | MissKind::FasterMate))
    }
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
    /// The piece the move gives up, when the move is good enough for a Brilliant.
    pub sacrifice: Option<Sacrifice>,
    /// The square of a threatened piece the move leaves standing (an ignored threat, whatever the
    /// move's grade): the mover's next move does not ignore it again.
    pub ignored_threat: Option<Square>,
    /// `max(WhiteElo, BlackElo)` reaches `great_find.high_rated_min` (`TEP::is_high_rated`).
    pub high_rated: bool,
    pub good_grade: bool,
    /// The move carried a Great tag (before Brilliant takes precedence), and its line wins
    /// material: the next move by the same side reads both.
    pub great_tag: bool,
    pub line_wins_material: bool,
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
            // Getting mated counts unless the best move is mated too (recovered).
            loses_material_or_mated: material_lost >= cfg.blunder_material_threshold_cp
                || (played_value.mate_moves().is_some_and(|n| n < 0)
                    && !best_value.mate_moves().is_some_and(|n| n < 0)),
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

    let gf = &cfg.great_find;
    let prev = &input.previous_moves;
    // `TEP::is_high_rated`: below it Brilliant and Great take their more generous branches.
    let high_rated = input.highest_elo as i32 >= gf.high_rated_min;
    let is_best = grade == Some(Precision::Best);
    let played_mate = played_value.mate_moves();
    // The position is critical (base tag 210, shared by Brilliant and Great): the best move keeps a
    // playable position and the second line loses at least a tenth of a point without staying
    // clearly better.
    let position_critical = is_best
        && lines.get(1).is_some_and(|alt| {
            let alt_value = Value::after_move(eval_after_first_move(alt, mover), mover);
            let alt_ep = node_expected_points(alt_value, false, rating);
            // Interposing a piece that is lost there does not count as an alternative.
            let unsafe_block = before.is_check()
                && board::parse_uci(before, &alt.pv[0]).is_some_and(|a| {
                    a.role() != Role::King && !a.is_capture() && !lands_safely(before, a)
                });
            !unsafe_block
                && ep_best >= gf.critical_min_best_ep
                && alt_ep < gf.critical_max_alternative_ep
                && ep_best - alt_ep >= gf.critical_min_alternative_loss
        });

    let bc = &cfg.brilliant;
    let best_forces_mate = best_value.mate_moves().is_some_and(|n| n >= 0);
    let qualifies = is_best
        || (bc.near_best
            && matches!(grade, Some(Precision::Excellent | Precision::Good))
            && !best_forces_mate);
    let answers_check_ok = !before.is_check()
        || lines.get(1).is_some_and(|l| {
            !Value::after_move(eval_after_first_move(l, mover), mover)
                .mate_moves()
                .is_some_and(|n| n < 0)
        });
    // `defensible_by_alternative` (report 4 §5, report 5 §5-6): another candidate move, graded Good
    // or better, loses no material along its line.
    let alt_defends = lines
        .iter()
        .filter(|l| l.pv.first().map(String::as_str) != Some(uci))
        .any(|l| {
            let alt_value = Value::after_move(eval_after_first_move(l, mover), mover);
            let alt_ep = node_expected_points(alt_value, false, rating);
            ep_best - alt_ep <= cfg.precision_thresholds[1]
                && walk_line(before, mover_color, &l.pv, 64).lost() < 1
        });
    let any_sacrifice = sacrifice(
        before,
        m,
        bc,
        high_rated,
        prev.own_ignored_threat,
        input.own_history,
        alt_defends,
    );
    let given_up = any_sacrifice.filter(|_| qualifies);
    let opponent_lost = node_expected_points(opponent_value_before, input.before_is_draw, 2000)
        < bc.opponent_lost_ep;
    let balance_after = board::balance(after.board(), mover_color);
    facts.events.brilliant = bc.enabled
        && qualifies
        && answers_check_ok
        && !threatens_promotion(before, m)
        && !captures_promotion_threat(before, m)
        && ep_played >= bc.min_ep_after
        && given_up
            .is_some_and(|s| material_gate(balance_after, see::value(s.role)) || !opponent_lost)
        && (position_critical || endgame_gain_ok(&played_walk, bc.endgame_min_gain_cp));

    // Great (`is_candidate<15>` and its tags; docs/calibration/great-blunder-analysis.md §2.1): a
    // Best or Excellent move that is not easy to find and is a key moment after the opponent's
    // error, the only good move in a critical position, or a move that makes the mover winning or
    // brings them back to equal. Low-rated games (`!high_rated`) get the more generous branch.
    // Our line's reply to the move, if it captures.
    let reply_captures = played_pv
        .get(1)
        .and_then(|reply| board::parse_uci(after, reply))
        .is_some_and(|r| r.is_capture());
    // The reference also asks the recapturing piece to be safe (`is_safe`), but its notion of
    // safety is not our exchange test: on the calibration games no best-move recapture was Great
    // whether our test called the square safe or not.
    let obvious_recapture = is_best && recapture;
    let free_piece = !recapture && captures_free_piece(before, m, 100);
    let easy_to_find = !m.is_castle()
        && (is_safe_promotion(before, m)
            || is_only_safe_square(before, m)
            || (high_rated && (obvious_recapture || free_piece))
            || input.shallow_best == Some(uci));
    // Exclusions for high-rated games only: taking back after an even trade, mating, and taking
    // the attacker in an even exchange.
    let simple_recapture = m.is_capture()
        && input.previous.is_some_and(|p| {
            p.capture().is_some_and(|theirs| {
                m.capture()
                    .is_some_and(|ours| see::value(theirs) <= see::value(ours))
            })
        })
        && lands_safely(before, m)
        && !reply_captures;
    let even_trade = reply_captures && played_walk.net().abs() < 100;
    let high_rated_excluded =
        simple_recapture || after.is_checkmate() || (removes_attacker(before, m) && even_trade);
    let candidate = good_grade
        && !easy_to_find
        && !(high_rated && high_rated_excluded)
        && !(prev.own_great_tag && (m.is_capture() || is_passed_pawn_push(before, m)));

    // Key moments answer the opponent's error, from a position the mover was not already winning
    // completely after their own previous move.
    let key_moment = input.has_previous_move
        && prev.opponent_erred
        && ep_played >= gf.key_moment_min_ep
        && !prev
            .own_value
            .is_some_and(|v| oc(v) == ExpectedOutcome::CompletelyWinning)
        && !recapture;
    let forces_mate = played_mate
        .is_some_and(|n| (0..=gf.forces_mate_max_moves as i32).contains(&n))
        && oc(played_value) >= ExpectedOutcome::Winning
        && prev
            .own_value
            .is_some_and(|v| v.0 <= gf.forces_mate_max_previous_cp);
    let line_wins_material = played_walk.net() > gf.wins_material_min_gain_cp;
    let wins_material = !free_piece && line_wins_material && !prev.own_wins_material;
    let key_tag = key_moment && (forces_mate || wins_material || (!high_rated && free_piece));
    // The only good move, unless it is an obvious recapture.
    let critical = position_critical && !obvious_recapture;
    // The outcome changed since before the mover's own previous move: from the opponent's equal or
    // better to the mover's winning, or from the opponent's winning back to the mover's equal.
    let earlier = prev.opponent_earlier_value.map(oc);
    let now = oc(played_value);
    let winning =
        earlier.is_some_and(|b| b >= ExpectedOutcome::Equal) && now >= ExpectedOutcome::Winning;
    let comeback = earlier.is_some_and(|b| b >= ExpectedOutcome::Winning)
        && now >= ExpectedOutcome::Equal
        && now < ExpectedOutcome::Winning;
    let great_tag = candidate && (key_tag || critical || winning || comeback);
    facts.events.great_find = gf.enabled && !facts.events.brilliant && great_tag;

    // What the best line would have achieved (used only for Mistake/Blunder).
    let after_opponent_error = cfg.miss.tactic_min_opponent_ep_lost <= 0.0
        || input
            .opponent_previous_ep_lost
            .is_some_and(|l| l >= cfg.miss.tactic_min_opponent_ep_lost);
    facts.missed = MissedChances {
        free_piece: after_opponent_error
            && takes_free_piece(&best_walk, cfg.miss.free_piece_min_value_cp),
        tactic: after_opponent_error && best_gain >= cfg.miss.tactic_min_gain_cp,
        win: oc(best_value) >= ExpectedOutcome::Winning
            && oc(played_value) <= ExpectedOutcome::Equal,
        comeback: oc(mover_value_before) <= ExpectedOutcome::Worse
            && oc(best_value) >= ExpectedOutcome::Equal
            && oc(played_value) <= ExpectedOutcome::Worse,
        brilliant: best_move.is_some_and(|bm| {
            sacrifice(
                before,
                bm,
                bc,
                high_rated,
                prev.own_ignored_threat,
                input.own_history,
                false,
            )
            .is_some()
        }) && ep_best >= cfg.miss.brilliant_min_best_ep,
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
        sacrifice: given_up,
        // The reference's IGNORES_THREAT base event, whatever the move's grade: the walk-back of
        // the same side's later moves reads it.
        ignored_threat: if bc.reference_standing {
            (!after.is_check() && !before.is_check() && !m.is_promotion())
                .then(|| {
                    reference_ignored_threat(before, after, m, input.own_history, bc, alt_defends)
                })
                .flatten()
        } else {
            any_sacrifice
                .filter(|s| s.kind == SacrificeKind::IgnoredThreat)
                .map(|s| s.square)
        },
        high_rated,
        good_grade,
        great_tag,
        line_wins_material,
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
            .and_then(|bm| {
                sacrifice(
                    input.before,
                    bm,
                    &cfg.brilliant,
                    judged.high_rated,
                    None,
                    &[],
                    false,
                )
            })
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
