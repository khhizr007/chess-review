//! Free and trapped material.

use shakmaty::{Position, Role};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

/// The first move of a line captures a piece that the other side cannot win
/// back by exchange (undefended, or defended too little): a free piece. When
/// the other side can take back with profit but the exchange still nets the
/// mover a pawn or more (a knight takes a rook guarded by a pawn), it is a
/// trade the mover wins (`WinsTrade`). Taking back on the
/// square where the other side just captured is never a free piece, whatever
/// the values: it ends an exchange, and what the exchange nets (say, the
/// exchange for a rook taken back with a knight) is the line's gain. A capture
/// that works only because the defender is pinned is left to `ExploitsPin`.
pub fn free_piece(ctx: &MoveCtx<'_>) -> Option<Found> {
    if !ctx.first || ctx.m.is_en_passant() {
        return None;
    }
    let captured = ctx.m.capture().filter(|r| *r != Role::King)?;
    let to = ctx.to();
    let recapture = ctx.previous.is_some_and(|p| p.to() == to && p.is_capture());
    if recapture || !facts::pinned_guards(ctx.before.board(), to, ctx.mover().other()).is_empty() {
        return None;
    }
    // What the other side wins back by taking on the square.
    let back = see::capture_gain(ctx.after, to);
    let net = value(captured) - back;
    (net >= 100).then(|| Found {
        tag: if back > 0 {
            Tag::WinsTrade
        } else {
            Tag::FreePiece
        },
        focus: Some(PieceAt {
            role: captured,
            color: ctx.mover().other(),
            square: to,
        }),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: ctx.m.from().unwrap_or(to),
            to,
        },
        stake: net,
    })
}

/// The moved piece attacks a knight, bishop, rook or queen that has no safe
/// square left, and that was not already lost before the move.
pub fn trapped_piece(ctx: &MoveCtx<'_>) -> Option<Found> {
    if ctx.after.is_check() || ctx.m.is_castle() {
        return None;
    }
    let mover = ctx.mover();
    let board = ctx.after.board();
    let attacked = board.attacks_from(ctx.to()) & board.by_color(mover.other());
    attacked
        .into_iter()
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| {
            matches!(
                t.role,
                Role::Knight | Role::Bishop | Role::Rook | Role::Queen
            )
        })
        .filter(|t| facts::threat(ctx.before.board(), t.square, mover) == 0)
        .filter(|t| facts::is_trapped(ctx.after, t.square))
        .max_by_key(|t| value(t.role))
        .map(|t| Found {
            tag: Tag::TrappedPiece,
            focus: Some(t),
            targets: Vec::new(),
            geometry: Geometry::Relation {
                from: ctx.to(),
                to: t.square,
            },
            stake: facts::threat(board, t.square, mover),
        })
}
