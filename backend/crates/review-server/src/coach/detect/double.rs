//! Double attacks: forks, double check and discovered attacks.

use shakmaty::{Position, Role, Square, attacks};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

pub fn double_check(ctx: &MoveCtx<'_>) -> Option<Found> {
    let checkers = ctx.after.checkers();
    if checkers.count() < 2 {
        return None;
    }
    let king = ctx.after.board().king_of(ctx.mover().other())?;
    Some(Found {
        tag: Tag::DoubleCheck,
        focus: ctx.moved(),
        targets: PieceAt::on(ctx.after.board(), king).into_iter().collect(),
        geometry: Geometry::Converging {
            target: king,
            sources: checkers.into_iter().collect(),
        },
        stake: 0,
    })
}

/// The moved piece (not the king) stands where it cannot be won, and attacks
/// two or more of the other side's pieces that are each either worth more
/// than it (the king counts) or undefended and unable to take it back. Pawns
/// count only when undefended. A fork of the king and queen is a royal fork.
pub fn fork(ctx: &MoveCtx<'_>) -> Option<Found> {
    if ctx.m.is_castle() {
        return None;
    }
    let forker = ctx.moved()?;
    if forker.role == Role::King || see::capture_gain(ctx.after, forker.square) > 0 {
        return None;
    }
    let board = ctx.after.board();
    let mover = ctx.mover();
    let own_value = value(forker.role);
    let forked = |t: &PieceAt| {
        let worth_more = t.role != Role::Pawn && value(t.role) > own_value;
        let undefended = facts::attackers(board, t.square, t.color).is_empty()
            && !board.attacks_from(t.square).contains(forker.square);
        worth_more || undefended
    };
    let mut targets: Vec<PieceAt> = (board.attacks_from(forker.square)
        & board.by_color(mover.other()))
    .into_iter()
    .filter_map(|t| PieceAt::on(board, t))
    .filter(forked)
    .collect();
    // A line piece also hits what lies behind a friendly piece in front of it
    // (a battery), when that makes the piece there newly lost.
    let battery = (board.attacks_from(forker.square) & board.by_color(mover))
        .into_iter()
        .filter_map(|front| facts::behind(board, forker.square, front))
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| t.color != mover && t.role != Role::King)
        .filter(forked)
        .filter(|t| facts::threat(board, t.square, mover) > 0)
        .filter(|t| facts::threat(ctx.before.board(), t.square, mover) == 0);
    for t in battery {
        if !targets.contains(&t) {
            targets.push(t);
        }
    }
    if targets.len() < 2 {
        return None;
    }
    // Most valuable first: "forks the king and rook".
    targets.sort_by_key(|t| std::cmp::Reverse(value(t.role)));
    let has = |r: Role| targets.iter().any(|t| t.role == r);
    let tag = if has(Role::King) && has(Role::Queen) {
        Tag::RoyalFork
    } else {
        Tag::Fork
    };
    // What the fork wins: the second most valuable target (the king never falls).
    let mut values: Vec<i32> = targets
        .iter()
        .filter(|t| t.role != Role::King)
        .map(|t| value(t.role))
        .collect();
    values.sort_unstable_by(|a, b| b.cmp(a));
    let stake = if has(Role::King) {
        values.first().copied().unwrap_or(0)
    } else {
        values.get(1).copied().unwrap_or(0)
    };
    Some(Found {
        tag,
        focus: Some(forker),
        geometry: Geometry::Fan {
            source: forker.square,
            targets: targets.iter().map(|t| t.square).collect(),
        },
        targets,
        stake,
    })
}

/// Own line pieces (other than the moved one) whose line ran through the
/// square the move vacated, with each of the other side's pieces they now hit.
fn uncovered(ctx: &MoveCtx<'_>) -> Vec<(Square, PieceAt)> {
    let Some(from) = ctx.m.from() else {
        return Vec::new();
    };
    if ctx.m.is_castle() {
        return Vec::new();
    }
    let board = ctx.after.board();
    let mover = ctx.mover();
    let mut out = Vec::new();
    for s in (board.by_color(mover) & board.sliders()).without(ctx.to()) {
        for t in board.attacks_from(s) & board.by_color(mover.other()) {
            if attacks::between(s, t).contains(from)
                && let Some(target) = PieceAt::on(board, t)
            {
                out.push((s, target));
            }
        }
    }
    out
}

/// The move steps off a line and a piece behind it gives check.
pub fn discovered_check(ctx: &MoveCtx<'_>) -> Option<Found> {
    if !ctx.after.is_check() {
        return None;
    }
    let checkers = ctx.after.checkers();
    let (slider, king) = uncovered(ctx)
        .into_iter()
        .find(|(s, t)| t.role == Role::King && checkers.contains(*s))?;
    Some(Found {
        tag: Tag::DiscoveredCheck,
        focus: PieceAt::on(ctx.after.board(), slider),
        targets: vec![king],
        geometry: Geometry::Relation {
            from: slider,
            to: king.square,
        },
        stake: 0,
    })
}

/// The move steps off a line and uncovers an attack on a knight, bishop, rook
/// or queen that is worth more than the attacker or hanging.
pub fn discovered_attack(ctx: &MoveCtx<'_>) -> Option<Found> {
    let board = ctx.after.board();
    let mover = ctx.mover();
    let (slider, target) = uncovered(ctx)
        .into_iter()
        .filter(|(_, t)| !matches!(t.role, Role::King | Role::Pawn))
        .filter(|(s, t)| {
            board.role_at(*s).is_some_and(|r| value(t.role) > value(r))
                || facts::threat(board, t.square, mover) > 0
        })
        .max_by_key(|(_, t)| value(t.role))?;
    Some(Found {
        tag: Tag::DiscoveredAttack,
        focus: PieceAt::on(board, slider),
        targets: vec![target],
        geometry: Geometry::Relation {
            from: slider,
            to: target.square,
        },
        stake: facts::threat(board, target.square, mover),
    })
}
