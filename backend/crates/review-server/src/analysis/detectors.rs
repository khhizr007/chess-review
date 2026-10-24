//! Board-event detectors (fallback heuristics; see `docs/implementation-notes.md`).
//!
//! The reference engine derives Brilliant / Great Find / Miss facts from about
//! 250 tactical tags and machine-learned models that were not recovered. These
//! detectors use material changes along engine principal variations instead.
//! Their outputs feed the recovered decision tree in `review-core`.

use review_core::config::BrilliantConfig;
use shakmaty::{Chess, Color, Move, Position, Role, Square};

use super::see;
use crate::board;

/// Material profile of a line from the mover's point of view (cp).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineMaterial {
    /// Balance before the first move of the line.
    pub start: i32,
    /// Balance after the last inspected ply.
    pub end: i32,
    /// Lowest / highest balance after any inspected ply.
    pub min: i32,
    pub max: i32,
    /// Value captured by the first move of the line, if it is a capture.
    pub first_capture_value: Option<i32>,
    pub plies: u32,
    /// When the line ends up ahead (`end > start`): the ply (0-based) whose
    /// move wins that material. Only settled points count (a ply whose next
    /// move is not a capture, so an exchange in progress is not mistaken for
    /// a gain or a loss): it is the first settled ply from which the balance
    /// stays at or above `end`. The tactic behind the gain is at or before it.
    pub gain_ply: Option<u32>,
}

impl LineMaterial {
    pub fn net(&self) -> i32 {
        self.end - self.start
    }

    pub fn lost(&self) -> i32 {
        (self.start - self.end).max(0)
    }
}

/// Extra plies allowed past the horizon to finish an exchange or check sequence.
const QUIESCENCE_EXTENSION: u32 = 6;

/// Plays `pv` (UCI, starting with the mover's move) from `start` for
/// `horizon` plies, extended while captures, promotions or checks continue,
/// and records the material balance for `mover`.
pub fn walk_line(start: &Chess, mover: Color, pv: &[String], horizon: u32) -> LineMaterial {
    let mut pos = start.clone();
    let initial = board::balance(pos.board(), mover);
    let mut walk = LineMaterial {
        start: initial,
        end: initial,
        min: initial,
        max: initial,
        first_capture_value: None,
        plies: 0,
        gain_ply: None,
    };
    // (balance after the ply, whether the ply's move captured)
    let mut balances: Vec<(i32, bool)> = Vec::new();
    let mut last_was_forcing = false;
    for (i, text) in pv.iter().enumerate() {
        // Stop at the horizon, but only at a quiet point (not mid-exchange or in check).
        let past_horizon = i >= horizon as usize;
        if (past_horizon && !last_was_forcing) || i >= (horizon + QUIESCENCE_EXTENSION) as usize {
            break;
        }
        let Some(m) = board::parse_uci(&pos, text) else {
            break;
        };
        last_was_forcing = m.is_capture() || m.is_promotion();
        if i == 0 {
            walk.first_capture_value = m.capture().map(board::role_value);
        }
        let captures = m.is_capture();
        pos.play_unchecked(m);
        last_was_forcing = last_was_forcing || pos.is_check();
        let b = board::balance(pos.board(), mover);
        walk.end = b;
        walk.min = walk.min.min(b);
        walk.max = walk.max.max(b);
        walk.plies += 1;
        balances.push((b, captures));
    }
    if walk.end > walk.start {
        let end = walk.end;
        let settled: Vec<(usize, i32)> = (0..balances.len())
            .filter(|&k| {
                balances
                    .get(k + 1)
                    .is_none_or(|(_, next_captures)| !next_captures)
            })
            .map(|k| (k, balances[k].0))
            .collect();
        let first_kept = settled
            .iter()
            .rposition(|(_, b)| *b < end)
            .map_or(0, |last_below| last_below + 1);
        walk.gain_ply = settled.get(first_kept).map(|(k, _)| *k as u32);
    }
    walk
}

/// How a move gives a piece up (the reference's base tags).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SacrificeKind {
    /// The moved piece goes where it can be won (`DIRECT_SACRIFICE_*`, `is_sacrifice`).
    Moved,
    /// A threatened piece is left where it stands (`IGNORES_THREAT`).
    IgnoredThreat,
    /// The move leaves another piece newly en prise (`INDIRECT_SACRIFICE`), such as a piece
    /// left behind by castling or uncovered by the moved piece.
    LeftEnPrise,
}

/// A piece the mover gives up with a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sacrifice {
    pub square: Square,
    pub role: Role,
    /// Material the opponent wins by capturing it (static exchange, cp).
    pub see_gain: i32,
    pub kind: SacrificeKind,
}

/// The sacrifice behind a Brilliant move (functional spec §5.5, after the reference's base tags
/// 20–23 and `node_lib::is_sacrifice`). The most valuable piece given up is returned.
///
/// A piece is given up when the opponent wins at least `min_threat_cp` by capturing it (static
/// exchange). Pawns count only with `pawns` (moved pawns; a pawn left standing never does).
/// - **Moved piece:** it had a safe square it could have gone to. If it was already threatened,
///   no other piece of at least its value (the mover's or the opponent's) was threatened, and
///   the move does not save more than one other piece worth as much.
/// - **Ignored threat** (`ignored_threats`): a piece threatened before the move, which had a safe
///   square to go to, is still threatened after it, and the mover's previous move did not already
///   ignore it (`ignored_before`).
/// - **Left en prise** (`left_en_prise`): a piece threatened after the move and not before.
///
/// The last two need a move that neither gives check, nor answers one, nor promotes, and a piece
/// that is not pinned to its king. In high-rated games (`high_rated`) the piece must also be worth
/// more than what the move captured plus the most valuable piece it newly attacks by discovery
/// (no disguised trade); for newer players there is no such test. Castling and promotions never
/// sacrifice the moved piece.
pub fn sacrifice(
    before: &Chess,
    m: Move,
    cfg: &BrilliantConfig,
    high_rated: bool,
    ignored_before: Option<Square>,
    history: &[EarlierOwnMove],
    alt_defends: bool,
) -> Option<Sacrifice> {
    let mover = before.turn();
    let opponent = mover.other();
    let mut after = before.clone();
    after.play_unchecked(m);
    if after.legal_moves().is_empty() {
        return None;
    }
    let captured = m.capture().map_or(0, see::value);
    let discovered = if cfg.reference_moved {
        reference_discovered_attack_value(before, &after, m)
    } else {
        discovered_attack_value(before, &after, m)
    };
    // A disguised trade: the piece is worth no more than what the move wins at once.
    let worth_giving = |role: Role| !high_rated || captured + discovered < see::value(role);
    let threatened_before =
        |sq: Square| see::see(before.board(), sq, opponent) >= cfg.min_threat_cp;
    let threat_after = |sq: Square| see::capture_gain(&after, sq);
    let given_up = |sq: Square, role: Role, kind: SacrificeKind| {
        let gain = threat_after(sq);
        (gain >= cfg.min_threat_cp).then_some(Sacrifice {
            square: sq,
            role,
            see_gain: gain,
            kind,
        })
    };
    let mut found: Vec<Sacrifice> = Vec::new();

    // The moved piece.
    if let Some(from) = m.from().filter(|_| !m.is_castle() && !m.is_promotion()) {
        let role = m.role();
        let moved = (role != Role::King && (role != Role::Pawn || cfg.pawns))
            .then(|| given_up(m.to(), role, SacrificeKind::Moved))
            .flatten()
            .filter(|s| {
                // `is_safe(n)`: the move's own exchange must lose material, so a trade is none.
                (!cfg.moved_net_loss || s.see_gain > captured)
                    && if cfg.reference_safe_squares {
                        has_reference_safe_square(before, from)
                    } else {
                        !safe_squares(before, from).is_empty()
                    }
                    && (!threatened_before(from)
                        || !mutual_or_bigger_threat(
                            before,
                            from,
                            role,
                            if cfg.reference_moved {
                                STRONG_THREAT_CP
                            } else {
                                1
                            },
                        ))
                    && if cfg.reference_moved {
                        reference_saved_pieces(before, &after, from, s.see_gain) <= 1
                    } else {
                        saved_pieces(before, &after, from, s.see_gain) <= 1
                    }
                    && worth_giving(role)
                    // High-rated only (`is_sacrifice`, report 4 §3 and §7): a capturer that is not
                    // relatively pinned exists, and the move is no deflection.
                    && (!high_rated
                        || !cfg.ref_deflection
                        || (!(see_threateners(after.board(), m.to())
                            & !pinned_attackers(after.board(), m.to(), true, cfg.ref_decoded_pins))
                        .is_empty()
                            && !has_overloaded_attacker(after.board(), m.to(), captured)))
            });
        found.extend(moved);
    }

    // Pieces the move leaves standing.
    let quiet_context = !after.is_check() && !before.is_check() && !m.is_promotion();
    if quiet_context && cfg.reference_standing {
        let ab = after.board();
        let standing = |sq: Square, kind: SacrificeKind| {
            ab.role_at(sq).map(|role| Sacrifice {
                square: sq,
                role,
                see_gain: see::see(ab, sq, opponent).max(0),
                kind,
            })
        };
        if cfg.ignored_threats {
            found.extend(
                reference_ignored_threat(before, &after, m, history, cfg, alt_defends)
                    .and_then(|sq| standing(sq, SacrificeKind::IgnoredThreat)),
            );
        }
        if cfg.left_en_prise {
            found.extend(
                reference_indirect_sacrifice(before, &after, m, cfg)
                    .and_then(|sq| standing(sq, SacrificeKind::LeftEnPrise)),
            );
        }
    } else if quiet_context && (cfg.ignored_threats || cfg.left_en_prise) {
        let moved_to = (!m.is_castle()).then(|| m.to());
        for sq in after.board().by_color(mover) {
            let Some(role) = after.board().role_at(sq) else {
                continue;
            };
            if Some(sq) == moved_to || matches!(role, Role::King | Role::Pawn) {
                continue;
            }
            // Castling moves the rook and king; any other piece stands where it stood.
            let stood = before.board().piece_at(sq) == after.board().piece_at(sq);
            let kind = if stood && threatened_before(sq) {
                // Ignoring a threat gives the piece up only if it could have been saved.
                if !cfg.ignored_threats
                    || ignored_before == Some(sq)
                    || (cfg.ignored_needs_safe_square && safe_squares(before, sq).is_empty())
                {
                    continue;
                }
                SacrificeKind::IgnoredThreat
            } else if cfg.left_en_prise {
                SacrificeKind::LeftEnPrise
            } else {
                continue;
            };
            let pinned = crate::coach::facts::is_absolutely_pinned(after.board(), sq);
            // The move hits back at a piece worth as much: an exchange of threats, not a sacrifice.
            let counter = cfg.counter_threat_excludes && counter_threat(&after, see::value(role));
            // Leaving a piece for one at least as big that the move just took is a trade.
            let trade = cfg.standing_trade_excludes && captured >= see::value(role);
            found.extend(
                given_up(sq, role, kind)
                    .filter(|_| !pinned && !counter && !trade && worth_giving(role)),
            );
        }
    }
    found.into_iter().max_by_key(|s| see::value(s.role))
}

/// For a moved piece that was already threatened: another piece of at least its value (the
/// mover's or the opponent's) was threatened too, so moving it is no sacrifice.
fn mutual_or_bigger_threat(before: &Chess, from: Square, role: Role, min_threat: i32) -> bool {
    let mover = before.turn();
    let opponent = mover.other();
    let board = before.board();
    let value = see::value(role);
    let threatened = |sq: Square, by: Color| {
        board
            .role_at(sq)
            .is_some_and(|r| r != Role::King && see::value(r) >= value)
            && see::see(board, sq, by) >= min_threat
    };
    board
        .by_color(mover)
        .without(from)
        .into_iter()
        .any(|sq| threatened(sq, opponent))
        || board
            .by_color(opponent)
            .into_iter()
            .any(|sq| threatened(sq, mover))
}

/// After the move, the mover (now not to move in `after`) threatens to win an opponent piece (not
/// the king) worth at least `value` by exchange.
fn counter_threat(after: &Chess, value: i32) -> bool {
    let mover = after.turn().other();
    let board = after.board();
    board.by_color(mover.other()).into_iter().any(|sq| {
        board
            .role_at(sq)
            .is_some_and(|r| r != Role::King && see::value(r) >= value)
            && see::see(board, sq, mover) > 0
    })
}

/// Other pieces of the mover that the move saves: threatened by at least `loss` before it and
/// no longer threatened after it (`saved_pieces`).
fn saved_pieces(before: &Chess, after: &Chess, from: Square, loss: i32) -> usize {
    let mover = before.turn();
    let opponent = mover.other();
    before
        .board()
        .by_color(mover)
        .without(from)
        .into_iter()
        .filter(|&sq| {
            see::see(before.board(), sq, opponent) >= loss
                && after.board().piece_at(sq) == before.board().piece_at(sq)
                && see::capture_gain(after, sq) <= 0
        })
        .count()
}

/// The most valuable opponent piece (not the king) that one of the mover's other pieces attacks
/// after the move and did not attack before it: the move uncovered the attack.
fn discovered_attack_value(before: &Chess, after: &Chess, m: Move) -> i32 {
    let mover = before.turn();
    let theirs = after.board().by_color(mover.other());
    after
        .board()
        .by_color(mover)
        .without(m.to())
        .into_iter()
        .filter(|&sq| before.board().piece_at(sq) == after.board().piece_at(sq))
        .flat_map(|sq| {
            let newly = after.board().attacks_from(sq) & !before.board().attacks_from(sq) & theirs;
            newly.into_iter().collect::<Vec<_>>()
        })
        .filter_map(|target| after.board().role_at(target).filter(|r| *r != Role::King))
        .map(see::value)
        .max()
        .unwrap_or(0)
}

/// Legal destinations of the piece on `from` where the opponent cannot win it
/// by exchange.
fn safe_squares(before: &Chess, from: Square) -> Vec<Square> {
    before
        .legal_moves()
        .iter()
        .filter(|mv| mv.from() == Some(from))
        .filter(|mv| {
            let mut p = before.clone();
            p.play_unchecked(**mv);
            see::capture_gain(&p, mv.to()) <= 0
        })
        .map(|mv| mv.to())
        .collect()
}

/// The reference's `Piece::safe_squares` is non-empty. `collect_eval_terms` builds it from the
/// piece's attacked squares, not its legal moves: a destination not held by its own side, along the
/// pin line if the piece is pinned to its king, whose own exchange does not lose (SEE ≥ 0). Check is
/// ignored, and an even trade counts as safe, unlike [`safe_squares`]. Pawns and kings never reach
/// this test.
fn has_reference_safe_square(before: &Chess, from: Square) -> bool {
    let board = before.board();
    let Some(piece) = board.piece_at(from) else {
        return false;
    };
    let mover = piece.color;
    let opponent = mover.other();
    let pinned = crate::coach::facts::is_absolutely_pinned(board, from);
    let king = board.king_of(mover);
    let targets = board.attacks_from(from) & !board.by_color(mover);
    targets.into_iter().any(|to| {
        if pinned && !king.is_some_and(|k| shakmaty::attacks::aligned(from, to, k)) {
            return false;
        }
        let captured = board.role_at(to).map_or(0, see::value);
        let mut b = board.clone();
        b.discard_piece_at(from);
        b.set_piece_at(to, piece);
        // What the opponent then wins by capturing on `to` (never negative: it need not capture).
        let lost = see::see(&b, to, opponent).max(0);
        captured - lost >= 0
    })
}

/// A move the same side played earlier, nearest first (`ignored_threats`' walk-back): the board after
/// it and the threatened piece it ignored, if any.
#[derive(Debug, Clone)]
pub struct EarlierOwnMove {
    pub board_after: shakmaty::Board,
    pub ignored: Option<Square>,
}

/// The reference's SEE thresholds (`collect_eval_terms`): a capture that wins at least this much
/// SEE-threatens the piece, and at least [`STRONG_THREAT_CP`] strongly threatens it.
const THREAT_CP: i32 = 100;
const STRONG_THREAT_CP: i32 = 200;

/// The square of the piece the move moved (the rook when castling).
fn moved_square(before: &Chess, m: Move) -> Square {
    if m.is_castle() {
        let king = m.from().unwrap_or(m.to());
        let rook = m.to();
        let back = king.rank();
        let file = if rook.file() > king.file() {
            shakmaty::File::F
        } else {
            shakmaty::File::D
        };
        let _ = before;
        Square::from_coords(file, back)
    } else {
        m.to()
    }
}

/// The squares in `squares` holding the most valuable pieces (all of them, on a tie).
fn most_valuable(board: &shakmaty::Board, squares: impl Iterator<Item = Square>) -> Vec<Square> {
    let with_value: Vec<(Square, i32)> = squares
        .filter_map(|sq| board.role_at(sq).map(|r| (sq, see::value(r))))
        .collect();
    let Some(top) = with_value.iter().map(|(_, v)| *v).max() else {
        return Vec::new();
    };
    with_value
        .into_iter()
        .filter(|(_, v)| *v == top)
        .map(|(sq, _)| sq)
        .collect()
}

/// `node_lib::ignored_threats(n, false, false, true)`: the single most valuable piece of the mover
/// strongly threatened both before and after the move, not already ignored by an earlier move of the
/// same side while it stayed threatened, and given up (`is_sacrifice_of_not_moved`). The caller has
/// excluded checks, check evasions and promotions.
pub fn reference_ignored_threat(
    before: &Chess,
    after: &Chess,
    m: Move,
    history: &[EarlierOwnMove],
    cfg: &BrilliantConfig,
    alt_defends: bool,
) -> Option<Square> {
    let mover = before.turn();
    let opponent = mover.other();
    let candidates = after.board().by_color(mover).into_iter().filter(|&sq| {
        after.board().role_at(sq).is_some_and(|r| r != Role::King)
            && before.board().piece_at(sq) == after.board().piece_at(sq)
            && see::see(before.board(), sq, opponent) >= STRONG_THREAT_CP
            && see::see(after.board(), sq, opponent) >= STRONG_THREAT_CP
    });
    let top = most_valuable(after.board(), candidates);
    let [sq] = top[..] else {
        return None;
    };
    for earlier in history {
        if earlier.board_after.piece_at(sq) != after.board().piece_at(sq)
            || see::see(&earlier.board_after, sq, opponent) < THREAT_CP
        {
            break;
        }
        if earlier.ignored == Some(sq) {
            return None;
        }
    }
    sacrifice_of_not_moved(before, after, m, sq, true, cfg, alt_defends).then_some(sq)
}

/// `node_lib::is_indirect_sacrifice`: of the mover's pieces strongly threatened after the move and
/// not threatened before it, the most valuable one that is given up (`is_sacrifice_of_not_moved`).
pub fn reference_indirect_sacrifice(
    before: &Chess,
    after: &Chess,
    m: Move,
    cfg: &BrilliantConfig,
) -> Option<Square> {
    let mover = before.turn();
    let opponent = mover.other();
    let candidates = after.board().by_color(mover).into_iter().filter(|&sq| {
        after.board().role_at(sq).is_some_and(|r| r != Role::King)
            && see::see(after.board(), sq, opponent) >= STRONG_THREAT_CP
            && !(before.board().piece_at(sq) == after.board().piece_at(sq)
                && see::see(before.board(), sq, opponent) >= THREAT_CP)
    });
    most_valuable(after.board(), candidates)
        .into_iter()
        .find(|&sq| sacrifice_of_not_moved(before, after, m, sq, false, cfg, false))
}

/// `node_lib::is_sacrifice_of_not_moved(n, sq, from_ignored)` (decompile report 2 §1). Parts not
/// modelled: `has_pinned_attacker`, `has_overloaded_attacker`, the safe-promotion-square test and
/// `defensible_by_alternative` (a relatively pinned piece always passes).
fn sacrifice_of_not_moved(
    before: &Chess,
    after: &Chess,
    m: Move,
    sq: Square,
    from_ignored: bool,
    cfg: &BrilliantConfig,
    alt_defends: bool,
) -> bool {
    let mover = before.turn();
    let opponent = mover.other();
    let ab = after.board();
    let moved_to = moved_square(before, m);
    if sq == moved_to {
        return false;
    }
    let Some(role) = ab.role_at(sq) else {
        return false;
    };
    let value = see::value(role);
    let cap_val = m.capture().map_or(0, see::value);
    let occupied = ab.occupied();
    let attackers_of_sq = if cfg.ref_created_see_threats {
        // Chess.com excludes only SEE-threateners of `sq` (`see_threats_to`), not every piece that
        // geometrically attacks it: a piece that attacks `sq` but cannot capture it without losing
        // material still counts as a threat the move created (report, §12.3).
        see_threateners(ab, sq)
    } else {
        ab.attacks_to(sq, opponent, occupied) & ab.by_color(opponent)
    };
    // The most valuable opponent piece the move newly strongly threatens, other than the king and
    // the pieces attacking `sq`, and what taking it wins.
    let created = ab.by_color(opponent).into_iter().filter(|&s| {
        !attackers_of_sq.contains(s)
            && ab.role_at(s).is_some_and(|r| r != Role::King)
            && see::see(ab, s, mover) >= STRONG_THREAT_CP
            && !(before.board().piece_at(s) == ab.piece_at(s)
                && see::see(before.board(), s, mover) >= STRONG_THREAT_CP)
    });
    let v1 = most_valuable(ab, created)
        .first()
        .map_or(0, |&s| see::see(ab, s, mover).max(0));
    let v_sq = see::see(ab, sq, opponent).max(0);
    // `saved_pieces(n, v_sq)`: the move took a piece out of a strong threat worth at least as much.
    let saved = before.board().by_color(mover).into_iter().any(|s| {
        let threat = see::see(before.board(), s, opponent);
        threat >= STRONG_THREAT_CP
            && threat >= v_sq
            && !(ab.piece_at(s) == before.board().piece_at(s)
                && see::see(ab, s, opponent) >= STRONG_THREAT_CP)
    });
    if saved {
        return false;
    }
    if from_ignored {
        let trapped = if cfg.ref_decoded_trapped {
            decoded_trapped(ab, sq)
        } else if cfg.ref_trapped {
            is_trapped_piece(after, sq)
        } else {
            !has_reference_safe_square(after, sq)
        };
        if trapped || crate::coach::facts::is_absolutely_pinned(ab, sq) {
            return false;
        }
        // A relatively pinned piece counts only if another move could have defended it
        // (defensible_by_alternative, report 4 §5).
        if cfg.ref_relative_pin_defensible && decoded_pin(ab, sq, opponent, None).0 && !alt_defends
        {
            return false;
        }
    }
    let moved_from = if m.is_castle() {
        // The rook stood on the castling square's corner.
        m.to()
    } else {
        m.from().unwrap_or(m.to())
    };
    let moved_value = ab.role_at(moved_to).map_or(0, see::value);
    if see::see(before.board(), moved_from, opponent) >= STRONG_THREAT_CP && value <= moved_value {
        return false;
    }
    if see::see(ab, moved_to, opponent) >= STRONG_THREAT_CP && value < moved_value {
        return false;
    }
    if v_sq <= cap_val + v1 {
        return false;
    }
    // The piece's attacker is pinned, or busy guarding something else (report 4 §1, §3).
    if cfg.ref_pinned_attacker && !pinned_attackers(ab, sq, false, cfg.ref_decoded_pins).is_empty()
    {
        return false;
    }
    if cfg.ref_overloaded && has_overloaded_attacker(ab, sq, cap_val) {
        return false;
    }
    // The move took away a square where the opponent could promote safely (report 4 §6).
    if cfg.ref_promotion
        && !(safe_promotion_squares(before.board(), opponent)
            & !safe_promotion_squares(ab, opponent))
        .is_empty()
    {
        return false;
    }
    // The moved piece's own threats: hitting something at least as valuable is a counter-attack,
    // unless that piece is the one attacking `sq`.
    let targets = (ab.attacks_from(moved_to) & ab.by_color(opponent))
        .into_iter()
        .filter(|&s| {
            ab.role_at(s).is_some_and(|r| r != Role::King) && see::see(ab, s, mover) >= THREAT_CP
        });
    let Some(&target) = most_valuable(ab, targets).first() else {
        return true;
    };
    let target_value = ab.role_at(target).map_or(0, see::value);
    target_value < value || ab.attacks_from(target).contains(sq)
}

/// `piece_value_of_greatest_discovered_attack` (decompile report 3 §3): the most valuable opponent
/// piece (not the king) that one of the mover's sliders, uncovered by the move, now strongly
/// threatens (SEE ≥ 200) and did not before.
fn reference_discovered_attack_value(before: &Chess, after: &Chess, m: Move) -> i32 {
    let mover = before.turn();
    let ab = after.board();
    let bb = before.board();
    let moved_to = moved_square(before, m);
    let sliders = ab.by_color(mover) & (ab.bishops() | ab.rooks() | ab.queens());
    let mut best = 0;
    for from in sliders {
        if from == moved_to || bb.piece_at(from) != ab.piece_at(from) {
            continue;
        }
        let newly = ab.attacks_from(from) & !bb.attacks_from(from) & ab.by_color(mover.other());
        for target in newly {
            let Some(role) = ab.role_at(target).filter(|r| *r != Role::King) else {
                continue;
            };
            let strong_now = see::see(ab, target, mover) >= STRONG_THREAT_CP;
            let strong_before = bb.piece_at(target) == ab.piece_at(target)
                && see::see(bb, target, mover) >= STRONG_THREAT_CP;
            if strong_now && !strong_before {
                best = best.max(see::value(role));
            }
        }
    }
    best
}

/// `saved_pieces(n, loss)` for the moved piece (decompile report 3 §2): the mover's pieces other than
/// the moved one that were strongly threatened before the move by at least `loss`, and no longer
/// are.
fn reference_saved_pieces(before: &Chess, after: &Chess, from: Square, loss: i32) -> usize {
    let opponent = before.turn().other();
    let (bb, ab) = (before.board(), after.board());
    bb.by_color(before.turn())
        .without(from)
        .into_iter()
        .filter(|&s| {
            let threat = see::see(bb, s, opponent);
            threat >= STRONG_THREAT_CP
                && threat >= loss
                && !(ab.piece_at(s) == bb.piece_at(s)
                    && see::see(ab, s, opponent) >= STRONG_THREAT_CP)
        })
        .count()
}

/// SEE of the move `from`→`to` (the reference's `see_gain` / `see_greater_equal`): what the moving
/// piece captures there, minus what the other side then wins by exchange on `to`.
fn move_see(board: &shakmaty::Board, from: Square, to: Square) -> i32 {
    let Some(mover) = board.piece_at(from) else {
        return 0;
    };
    let captured = board.role_at(to).map_or(0, see::value);
    let mut b = board.clone();
    b.discard_piece_at(from);
    b.set_piece_at(to, mover);
    captured - see::see(&b, to, mover.color.other()).max(0)
}

/// The slider on `p` and the piece on `a` stand on one of the slider's lines with nothing between.
fn slider_line_to(board: &shakmaty::Board, p: Square, a: Square) -> bool {
    use shakmaty::attacks;
    if attacks::ray(p, a).is_empty() || !(attacks::between(p, a) & board.occupied()).is_empty() {
        return false;
    }
    let orthogonal = p.file() == a.file() || p.rank() == a.rank();
    match board.role_at(p) {
        Some(Role::Queen) => true,
        Some(Role::Rook) => orthogonal,
        Some(Role::Bishop) => !orthogonal,
        _ => false,
    }
}

/// The first piece behind `a` on the line from `p` through `a`.
fn first_behind(board: &shakmaty::Board, p: Square, a: Square) -> Option<Square> {
    use shakmaty::attacks;
    let occupied = board.occupied();
    (attacks::ray(p, a) & occupied).into_iter().find(|&b| {
        b != a
            && attacks::between(p, b).contains(a)
            && (attacks::between(a, b) & occupied).is_empty()
    })
}

/// Pseudo-legal destinations of the piece on `a` (its attacks on squares not held by its side, plus
/// pawn pushes).
fn pseudo_destinations(board: &shakmaty::Board, a: Square) -> shakmaty::Bitboard {
    let Some(piece) = board.piece_at(a) else {
        return shakmaty::Bitboard::EMPTY;
    };
    let own = board.by_color(piece.color);
    if piece.role == Role::Pawn {
        let theirs = board.by_color(piece.color.other());
        let mut out = shakmaty::attacks::pawn_attacks(piece.color, a) & theirs;
        let step = if piece.color == Color::White { 8 } else { -8 };
        if let Some(push) = a.offset(step).filter(|p| !board.occupied().contains(*p)) {
            out.add(push);
        }
        out
    } else {
        board.attacks_from(a) & !own
    }
}

/// Whether the piece on `a` is pinned by a slider of colour `by` other than the one on `exclude`
/// (`collect_pieces_profiles` for sliders, decoded in report 5 and re-read from the code):
/// - **absolute** (theme 8): `a` is the only piece between the slider and its own king;
/// - **relative** (theme 9): the first piece behind `a` is of the same side, not a pawn and not the
///   king; with `a` removed the slider's capture there wins at least 100 (SEE); `a` cannot capture
///   the slider losing at most 100; `a` has a pseudo-legal move off the line; and `a` is worth no
///   more than the piece behind it.
///
/// Returns (relative, absolute).
fn decoded_pin(
    board: &shakmaty::Board,
    a: Square,
    by: Color,
    exclude: Option<Square>,
) -> (bool, bool) {
    use shakmaty::attacks;
    let (mut relative, mut absolute) = (false, false);
    let Some(front) = board.piece_at(a).filter(|p| p.color != by) else {
        return (false, false);
    };
    for p in board.by_color(by) & (board.bishops() | board.rooks() | board.queens()) {
        if Some(p) == exclude || !slider_line_to(board, p, a) {
            continue;
        }
        let Some(b) = first_behind(board, p, a) else {
            continue;
        };
        let Some(back) = board.piece_at(b).filter(|x| x.color == front.color) else {
            continue;
        };
        if back.role == Role::King {
            absolute = true;
            continue;
        }
        if back.role == Role::Pawn {
            continue;
        }
        let mut without = board.clone();
        without.discard_piece_at(a);
        let wins = move_see(&without, p, b) >= 100;
        let can_take_pinner = board.attacks_from(a).contains(p) && move_see(board, a, p) >= -100;
        let off_line = !(pseudo_destinations(board, a) & !attacks::between(p, b)).is_empty();
        if wins && !can_take_pinner && off_line && see::value(front.role) <= see::value(back.role) {
            relative = true;
        }
    }
    (relative, absolute)
}

/// `Piece::is_trapped` (theme 31, `collect_pieces_profiles`, read from the code): the piece is
/// SEE-threatened; it has no escape (unpinned: no square it attacks where its own exchange does not
/// lose; pinned to its king: none losing at most 100); no piece threatening it is itself
/// SEE-threatened; and the most valuable threatener it attacks back, if any, is worth less than 500.
fn decoded_trapped(board: &shakmaty::Board, sq: Square) -> bool {
    let Some(piece) = board.piece_at(sq) else {
        return false;
    };
    let threateners = see_threateners(board, sq);
    if threateners.is_empty() {
        return false;
    }
    let escapes = board.attacks_from(sq) & !board.by_color(piece.color);
    let floor = if crate::coach::facts::is_absolutely_pinned(board, sq) {
        -100
    } else {
        0
    };
    if escapes.into_iter().any(|e| move_see(board, sq, e) >= floor) {
        return false;
    }
    if threateners
        .into_iter()
        .any(|t| !see_threateners(board, t).is_empty())
    {
        return false;
    }
    let hit_back = threateners & board.attacks_from(sq);
    most_valuable(board, hit_back.into_iter())
        .first()
        .is_none_or(|&t| board.role_at(t).map_or(0, see::value) < 500)
}

/// Material `by` wins when the piece on `from` captures first on `sq` and the exchange is then played
/// out (the reference's per-capture `see_gain`).
fn capture_see(board: &shakmaty::Board, from: Square, sq: Square) -> i32 {
    let (Some(mover), Some(target)) = (board.piece_at(from), board.piece_at(sq)) else {
        return 0;
    };
    // `collect_eval_terms` skips a capture that would break a pin on the capturer's own king
    // (`violates_pin`): such a piece threatens nothing off its pin line. Checked against the
    // reference's own threat tables (docs/calibration/brilliant-analysis.md §11).
    if mover.role != Role::King
        && crate::coach::facts::is_absolutely_pinned(board, from)
        && !board
            .king_of(mover.color)
            .is_some_and(|k| shakmaty::attacks::aligned(from, sq, k))
    {
        return i32::MIN / 2;
    }
    let mut b = board.clone();
    b.discard_piece_at(from);
    b.set_piece_at(sq, mover);
    see::value(target.role) - see::see(&b, sq, mover.color.other()).max(0)
}

/// `see_threats_to(sq)`: pieces of the other side whose own capture on `sq` wins at least
/// [`THREAT_CP`].
fn see_threateners(board: &shakmaty::Board, sq: Square) -> shakmaty::Bitboard {
    let Some(target) = board.piece_at(sq) else {
        return shakmaty::Bitboard::EMPTY;
    };
    let by = target.color.other();
    (board.attacks_to(sq, by, board.occupied()) & board.by_color(by))
        .into_iter()
        .filter(|&from| capture_see(board, from, sq) >= THREAT_CP)
        .collect()
}

/// The piece the pin on `s` holds in place: an enemy slider attacks `s` along a line, and the first
/// piece behind `s` on that line belongs to the same side. Inferred (the reference reads its pin
/// themes 8 and 9, whose builders were not decoded): the standard definition.
fn pinned_behind(board: &shakmaty::Board, s: Square) -> Vec<(Square, Square)> {
    use shakmaty::attacks;
    let Some(piece) = board.piece_at(s) else {
        return Vec::new();
    };
    let enemy = board.by_color(piece.color.other());
    let occupied = board.occupied();
    let mut behind = Vec::new();
    for pinner in enemy & (board.bishops() | board.rooks() | board.queens()) {
        let line = attacks::ray(pinner, s);
        if line.is_empty() || !(attacks::between(pinner, s) & occupied).is_empty() {
            continue;
        }
        let orthogonal = pinner.file() == s.file() || pinner.rank() == s.rank();
        let fits = match board.role_at(pinner) {
            Some(Role::Queen) => true,
            Some(Role::Rook) => orthogonal,
            Some(Role::Bishop) => !orthogonal,
            _ => false,
        };
        if !fits {
            continue;
        }
        // The first piece behind `s`, seen from the pinner.
        let back = (line & occupied).into_iter().find(|&b| {
            b != s
                && attacks::between(pinner, b).contains(s)
                && (attacks::between(s, b) & occupied).is_empty()
        });
        // A pin holds only if moving `s` would lose: the piece behind is the king, is worth more
        // than the pinner, or is undefended (inferred: the usual meaning of a pin).
        let pinner_value = board.role_at(pinner).map_or(0, see::value);
        if let Some(b) = back.filter(|&b| board.color_at(b) == Some(piece.color)) {
            let back_role = board.role_at(b);
            let mut without = board.clone();
            without.discard_piece_at(s);
            let defended = !(without.attacks_to(b, piece.color, without.occupied())
                & without.by_color(piece.color))
            .is_empty();
            if back_role == Some(Role::King)
                || back_role.is_some_and(|r| see::value(r) > pinner_value)
                || !defended
            {
                behind.push((pinner, b));
            }
        }
    }
    behind
}

/// `Piece::relatively_pinned_attackers` / `absolutely_pinned_attackers` (report 4 §1): attackers of
/// the piece on `sq` that are pinned to a more valuable piece other than their king (relative,
/// SEE-threateners only) or to their king (absolute, any attacker), not to `sq` itself.
fn pinned_attackers(
    board: &shakmaty::Board,
    sq: Square,
    relative_only: bool,
    decoded: bool,
) -> shakmaty::Bitboard {
    let Some(target) = board.piece_at(sq) else {
        return shakmaty::Bitboard::EMPTY;
    };
    let by = target.color.other();
    let threateners = see_threateners(board, sq);
    let all = board.attacks_to(sq, by, board.occupied()) & board.by_color(by);
    let mut out = shakmaty::Bitboard::EMPTY;
    if decoded {
        // Pins by the target's own side, other than by the target itself (report 4 §1, report 5).
        for a in all {
            let (relative, absolute) = decoded_pin(board, a, target.color, Some(sq));
            if (relative && threateners.contains(a)) || (!relative_only && absolute) {
                out.add(a);
            }
        }
        return out;
    }
    for a in all {
        let value = board.role_at(a).map_or(0, see::value);
        for (pinner, b) in pinned_behind(board, a) {
            // A pin by the threatened piece itself does not stop the capture (report 4 §1).
            if pinner == sq || b == sq {
                continue;
            }
            let back_role = board.role_at(b);
            let relative = threateners.contains(a)
                && back_role.is_some_and(|r| r != Role::King && see::value(r) > value);
            let absolute = back_role == Some(Role::King);
            if relative || (!relative_only && absolute) {
                out.add(a);
            }
        }
    }
    out
}

/// `Piece::has_overloaded_attacker(v)` (report 4 §3, re-read: the reference requires exactly one
/// SEE-threatener and compares with the threatened piece's value). The single attacker of the piece
/// on `sq` also guards one of its own pieces that it would stop guarding after capturing on `sq`,
/// and winning that piece then pays back the threatened piece's value up to `v`.
fn has_overloaded_attacker(board: &shakmaty::Board, sq: Square, v: i32) -> bool {
    let threateners = see_threateners(board, sq);
    let mut it = threateners.into_iter();
    let (Some(a), None) = (it.next(), it.next()) else {
        return false;
    };
    let (Some(target), Some(attacker)) = (board.piece_at(sq), board.piece_at(a)) else {
        return false;
    };
    let their = target.color.other();
    let occupied = board.occupied();
    // What the attacker would attack from `sq` after capturing there.
    let from_sq = shakmaty::attacks::attacks(sq, attacker, occupied.without(a));
    let guards_now = shakmaty::attacks::attacks(a, attacker, occupied);
    let king = board.king_of(their);
    let duties =
        board.by_color(their) & guards_now & !from_sq & !shakmaty::Bitboard::from_square(sq);
    let value = see::value(target.role);
    duties.into_iter().filter(|&d| Some(d) != king).any(|d| {
        let mut b = board.clone();
        b.discard_piece_at(sq);
        b.discard_piece_at(a);
        let best = see::see(&b, d, target.color);
        value - best <= v
    })
}

/// `Piece::is_trapped` (theme 31). Inferred (the theme builder was not decoded): the piece is
/// SEE-threatened and has no square it attacks where its own exchange does not lose.
fn is_trapped_piece(pos: &Chess, sq: Square) -> bool {
    !see_threateners(pos.board(), sq).is_empty() && !has_reference_safe_square(pos, sq)
}

/// `StaticAnalysis::safe_promotion_squares(c)` (report 4 §6): last-rank squares a pawn of `c` can
/// push to (empty) or capture onto (not the king), where the other side cannot win the new piece.
fn safe_promotion_squares(board: &shakmaty::Board, c: Color) -> shakmaty::Bitboard {
    let pawns = board.by_color(c) & board.pawns();
    let last = match c {
        Color::White => shakmaty::Bitboard::from(shakmaty::Rank::Eighth),
        Color::Black => shakmaty::Bitboard::from(shakmaty::Rank::First),
    };
    let mut out = shakmaty::Bitboard::EMPTY;
    for p in pawns {
        let Some(push) = p.offset(if c == Color::White { 8 } else { -8 }) else {
            continue;
        };
        let mut targets = shakmaty::Bitboard::EMPTY;
        if !board.occupied().contains(push) {
            targets.add(push);
        }
        let king = board.king_of(c.other());
        targets |= shakmaty::attacks::pawn_attacks(c, p)
            & board.by_color(c.other())
            & !king.map_or(shakmaty::Bitboard::EMPTY, shakmaty::Bitboard::from_square);
        for t in targets & last {
            let mut b = board.clone();
            b.discard_piece_at(p);
            b.set_piece_at(
                t,
                shakmaty::Piece {
                    color: c,
                    role: Role::Queen,
                },
            );
            if see::see(&b, t, c.other()) <= 0 {
                out.add(t);
            }
        }
    }
    out
}

/// Great exclusion (`is_easy_to_find`): a threatened piece (not a pawn) goes to the only square
/// where the opponent cannot win it by exchange. The reference does not say "threatened", but on
/// the calibration games the rule without it (or with pawns) removed more Great moves than wrong
/// ones.
pub fn is_only_safe_square(before: &Chess, m: Move) -> bool {
    match m.from() {
        Some(from) if !m.is_castle() && m.role() != Role::Pawn => {
            see::see(before.board(), from, before.turn().other()) > 0
                && safe_squares(before, from) == [m.to()]
        }
        _ => false,
    }
}

/// The moved piece cannot be won by exchange on its new square (`is_safe`).
pub fn lands_safely(before: &Chess, m: Move) -> bool {
    if m.is_castle() {
        return true;
    }
    let mut after = before.clone();
    after.play_unchecked(m);
    see::capture_gain(&after, m.to()) <= 0
}

/// The move captures the piece that was attacking the capturing piece (`removes_attacker_to_self`).
pub fn removes_attacker(before: &Chess, m: Move) -> bool {
    match m.from() {
        Some(from) if m.is_capture() && !m.is_castle() && !m.is_en_passant() => {
            before.board().attacks_from(m.to()).contains(from)
        }
        _ => false,
    }
}

/// The move captures a piece worth at least `min_value` that the opponent
/// cannot win back by exchange.
pub fn captures_free_piece(before: &Chess, m: Move, min_value: i32) -> bool {
    let Some(captured) = m.capture().map(board::role_value) else {
        return false;
    };
    let mut after = before.clone();
    after.play_unchecked(m);
    captured >= min_value && see::capture_gain(&after, m.to()) <= 0
}

/// Brilliant exclusion (`threatens_to_promote`): a pawn move that promotes or reaches the seventh
/// rank.
pub fn threatens_promotion(before: &Chess, m: Move) -> bool {
    let seventh = match before.turn() {
        Color::White => shakmaty::Rank::Seventh,
        Color::Black => shakmaty::Rank::Second,
    };
    m.role() == Role::Pawn && (m.is_promotion() || m.to().rank() == seventh)
}

/// Brilliant exclusion (`captures_promotion_threat`): the move takes a pawn one step from
/// promoting.
pub fn captures_promotion_threat(before: &Chess, m: Move) -> bool {
    let their_seventh = match before.turn() {
        Color::White => shakmaty::Rank::Second,
        Color::Black => shakmaty::Rank::Seventh,
    };
    m.capture() == Some(Role::Pawn) && m.to().rank() == their_seventh
}

/// Great exclusion (`is_easy_to_find`): a promotion the opponent cannot win back by exchange.
pub fn is_safe_promotion(before: &Chess, m: Move) -> bool {
    m.is_promotion() && lands_safely(before, m)
}

/// Recovered gate for Brilliant/Great Find: the mover is not already ahead in
/// material by at least the value of the piece involved.
pub fn material_gate(balance_after_move: i32, piece_value: i32) -> bool {
    balance_after_move < piece_value
}

/// The first move captures a piece that is not won back within the horizon.
pub fn takes_free_piece(line: &LineMaterial, min_value: i32) -> bool {
    match line.first_capture_value {
        Some(value) if value >= min_value => line.net() >= value - 100,
        _ => false,
    }
}

/// A quiet move of a passed pawn (`is_pushes_passed_pawn`): no opposing pawn ahead of it on its own
/// or a neighbouring file. Passed on its old square, it is passed on the new one too.
pub fn is_passed_pawn_push(before: &Chess, m: Move) -> bool {
    let Some(from) = m
        .from()
        .filter(|_| m.role() == Role::Pawn && !m.is_capture())
    else {
        return false;
    };
    let mover = before.turn();
    let theirs = before.board().pawns() & before.board().by_color(mover.other());
    !theirs.into_iter().any(|sq| {
        sq.file().distance(from.file()) <= 1
            && match mover {
                Color::White => sq.rank() > from.rank(),
                Color::Black => sq.rank() < from.rank(),
            }
    })
}

/// `played` recaptures on the square where the opponent just captured.
pub fn is_recapture(previous: Option<Move>, played: Move) -> bool {
    match previous {
        Some(prev) => prev.is_capture() && played.is_capture() && prev.to() == played.to(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::CastlingMode;
    use shakmaty::fen::Fen;

    fn pos(fen: &str) -> Chess {
        fen.parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap()
    }

    fn line(moves: &[&str]) -> Vec<String> {
        moves.iter().map(|m| (*m).to_owned()).collect()
    }

    fn sac(fen: &str, uci: &str, cfg: &BrilliantConfig) -> Option<Sacrifice> {
        let p = pos(fen);
        let m = board::parse_uci(&p, uci).expect("legal move");
        sacrifice(&p, m, cfg, true, None, &[], false)
    }

    #[test]
    fn bishop_sacrifice_on_f7_is_detected() {
        // Italian-style position: Bxf7+ Kxf7 gives a bishop for a pawn.
        let fen = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/3P1N2/PPP2PPP/RNBQK2R w KQkq - 0 5";
        let walk = walk_line(&pos(fen), Color::White, &line(&["c4f7", "e8f7"]), 6);
        assert_eq!(walk.net(), -200);
        assert_eq!(
            sac(fen, "c4f7", &BrilliantConfig::default()),
            Some(Sacrifice {
                square: Square::F7,
                role: Role::Bishop,
                see_gain: 300,
                kind: SacrificeKind::Moved,
            })
        );
    }

    #[test]
    fn quiet_moves_are_never_sacrifices() {
        let cfg = BrilliantConfig::default();
        // Be2, castling, and a king move.
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 2";
        assert_eq!(sac(fen, "f1e2", &cfg), None);
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
        assert_eq!(sac(fen, "e1g1", &cfg), None);
        assert_eq!(sac("8/8/4k3/8/8/8/5P1P/6K1 w - - 0 1", "g1g2", &cfg), None);
    }

    #[test]
    fn pawn_grab_and_later_trades_are_not_sacrifices() {
        // Nxe5 leaves nothing en prise, whatever the engine line does later
        // (Nxe5 d6 Nf3 Nxe4 dips nowhere; ...Bxc3 bxc3 lines trade evenly).
        let fen = "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3";
        let walk = walk_line(
            &pos(fen),
            Color::White,
            &line(&["f3e5", "d7d6", "e5f3", "f6e4"]),
            6,
        );
        assert_eq!(walk.net(), 0);
        assert_eq!(sac(fen, "f3e5", &BrilliantConfig::default()), None);
        // a3 attacks the bishop; the line a3 Bxc3+ bxc3 gives up nothing.
        let fen = "r1bqk2r/pppp1ppp/2n2n2/4p3/1b2P3/2N2N2/PPPP1PPP/R1BQKB1R w KQkq - 4 4";
        assert_eq!(sac(fen, "a2a3", &BrilliantConfig::default()), None);
    }

    #[test]
    fn legal_trap_gives_up_the_queen() {
        // Legal's trap, 5.Nxe5 (Chess.com: Brilliant): the queen on d1 is left en
        // prise; it is worth more than the knight, so it is the piece given up.
        let fen = "rn1qkbnr/ppp2p1p/3p2p1/4p3/2B1P1b1/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 5";
        assert_eq!(
            sac(fen, "f3e5", &BrilliantConfig::default()),
            Some(Sacrifice {
                square: Square::D1,
                role: Role::Queen,
                see_gain: 600,
                kind: SacrificeKind::LeftEnPrise,
            })
        );
        // Counting only the moved piece: for newer players the knight is given up for a pawn, but
        // in a high-rated game it is a disguised trade, since leaving f3 uncovers Qxg4.
        let cfg = BrilliantConfig {
            left_en_prise: false,
            ignored_threats: false,
            ..Default::default()
        };
        let (p, m) = mv(fen, "f3e5");
        assert_eq!(
            sacrifice(&p, m, &cfg, false, None, &[], false).map(|s| s.role),
            Some(Role::Knight)
        );
        assert_eq!(sacrifice(&p, m, &cfg, true, None, &[], false), None);
    }

    #[test]
    fn traxler_knight_sacrifice_with_a_discovered_attack() {
        // 8...Nxg3 (Chess.com game 185058622904, both players below 1000: Brilliant). hxg3 wins
        // the knight for a pawn. Newer players get no disguised-trade test; in a high-rated game
        // the discovered attack on c4 would make it one.
        let fen = "r1b1k2r/pppp1Npp/2n5/4p3/2B1n2q/6P1/PPPP3P/RNBQK2R b kq - 0 8";
        let (p, m) = mv(fen, "e4g3");
        assert_eq!(
            sacrifice(&p, m, &BrilliantConfig::default(), false, None, &[], false),
            Some(Sacrifice {
                square: Square::G3,
                role: Role::Knight,
                see_gain: 200,
                kind: SacrificeKind::Moved,
            })
        );
        assert_eq!(
            sacrifice(&p, m, &BrilliantConfig::default(), true, None, &[], false),
            None
        );
    }

    #[test]
    fn ignored_threat_walks_back_over_earlier_moves() {
        // 15...d5 (same game; Chess.com: Brilliant, tagged IgnoresThreat on the rook). The rook on
        // h8 has been en prise since 5.Nxf7, but no earlier move of Black ignored it as a sacrifice,
        // so leaving it again now counts. A pawn can never be strongly threatened (SEE ≥ 200).
        let fen = "r1b1k2r/pppp1Npp/8/4p3/2Bn3q/8/PPPP2K1/RNBQ4 b kq - 7 15";
        let (p, m) = mv(fen, "d7d5");
        let cfg = BrilliantConfig::default();
        let ignored_rook = Some(Sacrifice {
            square: Square::H8,
            role: Role::Rook,
            see_gain: 500,
            kind: SacrificeKind::IgnoredThreat,
        });
        assert_eq!(
            sacrifice(&p, m, &cfg, false, None, &[], false),
            ignored_rook
        );
        // Had Black's previous move already ignored the rook while it stood threatened, this one
        // would not.
        let earlier = EarlierOwnMove {
            board_after: p.board().clone(),
            ignored: Some(Square::H8),
        };
        assert_eq!(
            sacrifice(
                &p,
                m,
                &cfg,
                false,
                None,
                std::slice::from_ref(&earlier),
                false
            ),
            None
        );
        // An earlier move that ignored something else does not stop it.
        let other = EarlierOwnMove {
            ignored: Some(Square::A8),
            ..earlier
        };
        assert_eq!(
            sacrifice(&p, m, &cfg, false, None, &[other], false),
            ignored_rook
        );
    }

    #[test]
    fn pinned_attacker_of_a_threatened_piece() {
        // The black knight on d6 attacks the rook on e4 but is pinned by the bishop on b4 to the
        // queen on f8, worth more than the bishop.
        let p = pos("5qk1/8/3n4/8/1B2R3/8/8/6K1 w - - 0 1");
        assert!(pinned_attackers(p.board(), Square::E4, false, true).contains(Square::D6));
        // Without the bishop nothing pins it.
        let p = pos("5qk1/8/3n4/8/4R3/8/8/6K1 w - - 0 1");
        assert!(pinned_attackers(p.board(), Square::E4, false, true).is_empty());
    }

    #[test]
    fn safe_promotion_squares_of_a_pawn() {
        let p = pos("7k/1P6/8/8/8/8/8/K7 w - - 0 1");
        assert!(safe_promotion_squares(p.board(), Color::White).contains(Square::B8));
        // A rook on a8 guards b8, but taking it promotes safely.
        let p = pos("r6k/1P6/8/8/8/8/8/K7 w - - 0 1");
        let safe = safe_promotion_squares(p.board(), Color::White);
        assert!(safe.contains(Square::A8) && !safe.contains(Square::B8));
    }

    #[test]
    fn castling_can_leave_a_piece_behind() {
        // 19.O-O-O (Chess.com game 186411389477: Brilliant): the king no longer
        // guards e2, so the queen on e3 can take the bishop.
        let fen = "r3k2r/p2b1pp1/8/1pp4p/2p5/Q1P1q1PP/PP1NB1P1/R3KR2 w Qkq - 0 19";
        assert_eq!(
            sac(fen, "e1c1", &BrilliantConfig::default()).map(|s| (s.square, s.role)),
            Some((Square::E2, Role::Bishop))
        );
    }

    #[test]
    fn promotions_are_not_sacrifices() {
        // g8=Q+ Kxg8: the new queen is captured, but nothing was given up.
        let fen = "8/6Pk/8/4Q3/8/2P5/PP6/2K1R3 w - - 1 51";
        assert_eq!(sac(fen, "g7g8q", &BrilliantConfig::default()), None);
    }

    fn mv(fen: &str, uci: &str) -> (Chess, Move) {
        let p = pos(fen);
        let m = board::parse_uci(&p, uci).expect("legal move");
        (p, m)
    }

    #[test]
    fn only_safe_square() {
        // The bishop on a2 is attacked by the pawn; b1 is its only safe square
        // (Bxb3 runs into the rook).
        let fen = "1r2k3/8/8/8/8/1p6/B7/4K3 w - - 0 1";
        let (p, m) = mv(fen, "a2b1");
        assert!(is_only_safe_square(&p, m));
        let (p, m) = mv(fen, "a2b3");
        assert!(!is_only_safe_square(&p, m), "not the safe square");
        assert!(!lands_safely(&p, m));
        // Without the rook, Bxb3 is safe too: two ways out.
        let (p, m) = mv("4k3/8/8/8/8/1p6/B7/4K3 w - - 0 1", "a2b1");
        assert!(!is_only_safe_square(&p, m));
    }

    #[test]
    fn capturing_the_attacker() {
        // The knight on b3 attacks c1, so Nxb3 removes the piece attacking the knight.
        let (p, m) = mv("4k3/8/8/8/8/1n6/8/2N1K3 w - - 0 1", "c1b3");
        assert!(removes_attacker(&p, m));
        // It does not attack d1, so Bxb3 only captures.
        let (p, m) = mv("4k3/8/8/8/8/1n6/8/3BK3 w - - 0 1", "d1b3");
        assert!(!removes_attacker(&p, m));
    }

    #[test]
    fn free_piece_by_exchange() {
        let (p, m) = mv("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", "d1d5");
        assert!(captures_free_piece(&p, m, 300));
        let (p, m) = mv("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1", "d1d5");
        assert!(
            !captures_free_piece(&p, m, 300),
            "the pawn wins the rook back"
        );
    }

    #[test]
    fn safe_promotions() {
        let (p, m) = mv("4k3/2P5/8/8/8/8/8/4K3 w - - 0 1", "c7c8q");
        assert!(is_safe_promotion(&p, m));
        let (p, m) = mv("1r2k3/2P5/8/8/8/8/8/4K3 w - - 0 1", "c7c8q");
        assert!(!is_safe_promotion(&p, m), "the rook takes the new queen");
        let (p, m) = mv("4k3/8/2P5/8/8/8/8/4K3 w - - 0 1", "c6c7");
        assert!(!is_safe_promotion(&p, m), "not a promotion");
    }

    #[test]
    fn passed_pawn_pushes() {
        let (p, m) = mv("4k3/8/8/8/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(is_passed_pawn_push(&p, m));
        let (p, m) = mv("4k3/2p5/8/8/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(!is_passed_pawn_push(&p, m), "the c-pawn guards d6");
        // Passed only after the move: the e5 pawn stood ahead of d4 on the next file.
        let (p, m) = mv("4k3/8/8/4p3/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(!is_passed_pawn_push(&p, m));
        let (p, m) = mv("4k3/8/8/8/8/3p4/8/4K3 b - - 0 1", "d3d2");
        assert!(is_passed_pawn_push(&p, m));
    }

    #[test]
    fn doomed_piece_is_not_a_sacrifice() {
        // The bishop on a7 is lost wherever it goes, so Bxb6 gives nothing up.
        let fen = "r3k3/B1p5/1p6/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(sac(fen, "a7b6", &BrilliantConfig::default()), None);
    }

    #[test]
    fn opera_game_sacrifices() {
        let pgn = include_str!("../../../../../tests/fixtures/opera_game.pgn");
        let game = crate::pgn::parse_pgn(pgn).unwrap().remove(0).unwrap();
        let cfg = BrilliantConfig::default();
        let at = |ply: usize| {
            let parsed = &game.moves[ply - 1];
            (
                parsed.san.as_str(),
                sacrifice(
                    &game.positions[ply - 1],
                    parsed.mv,
                    &cfg,
                    true,
                    None,
                    &[],
                    false,
                )
                .map(|s| s.role),
            )
        };
        assert_eq!(at(19), ("Nxb5", Some(Role::Knight)));
        assert_eq!(at(21), ("Bxb5+", None));
        assert_eq!(at(25), ("Rxd7", Some(Role::Rook)));
        assert_eq!(at(29), ("Bxd7+", None), "rook for bishop is a trade");
        assert_eq!(at(31), ("Qb8+", Some(Role::Queen)));
    }

    #[test]
    fn walk_finishes_exchanges_past_the_horizon() {
        let p = pos("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3");
        // Horizon 1 would stop right after Nxe5 (+1); the capture extends the walk to d6.
        let walk = walk_line(&p, Color::White, &line(&["f3e5", "d7d6", "e5f3"]), 1);
        assert_eq!(walk.plies, 2);
        assert_eq!(walk.net(), 100);
        assert_eq!(walk.gain_ply, Some(0), "the pawn is won by the first move");
    }

    #[test]
    fn gain_ply_is_where_the_material_is_won() {
        // 1.Nc7+ Kd8 2.Nxa8: the fork comes first, the rook falls on ply 2.
        let p = pos("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["b5c7", "e8d8", "c7a8"]), 6);
        assert_eq!(walk.net(), 500);
        assert_eq!(walk.gain_ply, Some(2));
        // Rxd5 exd5 gives the exchange: no gain, no gain ply.
        let p = pos("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d5", "e6d5"]), 6);
        assert_eq!(walk.gain_ply, None);
    }

    #[test]
    fn free_piece_capture() {
        // Undefended queen on d4.
        let p = pos("4k3/8/8/8/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e8e7"]), 6);
        assert_eq!(walk.plies, 2);
        assert!(takes_free_piece(&walk, 300));

        // Defended by a pawn: Rxd4 exd4 wins material but the queen was not free.
        let p = pos("4k3/8/8/4p3/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e5d4"]), 6);
        assert_eq!(walk.net(), 400);
        assert!(!takes_free_piece(&walk, 300));
    }
}
