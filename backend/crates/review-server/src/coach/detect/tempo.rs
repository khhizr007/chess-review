//! Tempo: attacking a piece that has to move away (group 10).
//!
//! A threat the opponent answers simply by moving the attacked piece is not a
//! threat to win it; it gains time. A pawn doing it *kicks* the piece; another
//! piece *gains a tempo*. Threats to win material that cannot be answered so
//! simply stay THREATENS events (`circ::threatens`).

use shakmaty::{Position, Role};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt};
use crate::coach::geometry::Geometry;

/// The tempo tag of the move, if any.
pub fn tempo_tags(ctx: &MoveCtx<'_>) -> Vec<Found> {
    attacked_with_escape(ctx).into_iter().collect()
}

/// The one opponent piece (not a pawn or the king) that the moved piece newly
/// threatens to win, from a square where the moved piece is safe.
fn newly_attacked(ctx: &MoveCtx<'_>) -> Option<PieceAt> {
    let moved = ctx.moved().filter(|_| !ctx.m.is_castle())?;
    if moved.role == Role::King || ctx.after.is_check() {
        return None;
    }
    // A piece that can be taken where it stands gains no time.
    if see::capture_gain(ctx.after, moved.square) > 0 {
        return None;
    }
    let board = ctx.after.board();
    let mover = ctx.mover();
    let targets: Vec<PieceAt> = (board.attacks_from(moved.square) & board.by_color(mover.other()))
        .into_iter()
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| !matches!(t.role, Role::King | Role::Pawn))
        .filter(|t| facts::threat(board, t.square, mover) > 0)
        .filter(|t| facts::threat(ctx.before.board(), t.square, mover) == 0)
        .collect();
    // Two pieces hit at once is a fork, not a kick.
    match targets.as_slice() {
        [t] => Some(*t),
        _ => None,
    }
}

fn attacked_with_escape(ctx: &MoveCtx<'_>) -> Option<Found> {
    let target = newly_attacked(ctx)?;
    if !facts::has_safe_square(ctx.after, target.square) {
        return None;
    }
    let moved = ctx.moved()?;
    Some(Found {
        tag: if moved.role == Role::Pawn {
            Tag::Kicks
        } else {
            Tag::GainsTempo
        },
        focus: Some(target),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: moved.square,
            to: target.square,
        },
        stake: 0,
    })
}
