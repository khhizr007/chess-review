//! Mates and the named mate patterns.

use shakmaty::{Bitboard, Chess, Position, Rank, Role, Square, attacks};

use super::{Found, MoveCtx, Tag};
use crate::coach::facts::PieceAt;
use crate::coach::geometry::Geometry;

/// The move gives checkmate; a back-rank mate when the pattern fits.
pub fn checkmate(ctx: &MoveCtx<'_>) -> Option<Found> {
    if !ctx.after.is_checkmate() {
        return None;
    }
    let mated = ctx.mover().other();
    let king = ctx.after.board().king_of(mated)?;
    let checkers = ctx.after.checkers();
    let tag = if is_back_rank_mate(ctx.after, king, checkers) {
        Tag::BackRankMate
    } else {
        Tag::Checkmate
    };
    Some(Found {
        tag,
        focus: ctx.moved(),
        targets: PieceAt::on(ctx.after.board(), king).into_iter().collect(),
        geometry: Geometry::Converging {
            target: king,
            sources: checkers.into_iter().collect(),
        },
        stake: 0,
    })
}

/// The mated king stands on its home rank, the checker (a rook or queen)
/// gives check along that rank, and every square in front of the king is
/// taken by its own pieces: the king is boxed in by its own army. When the
/// mating side covers a square in front, the mate is the attack's work, not
/// the back rank's.
pub fn is_back_rank_mate(pos: &Chess, king: Square, checkers: Bitboard) -> bool {
    let board = pos.board();
    let Some(color) = board.color_at(king) else {
        return false;
    };
    let home = color.fold_wb(Rank::First, Rank::Eighth);
    let front_rank = color.fold_wb(Rank::Second, Rank::Seventh);
    if king.rank() != home || checkers.is_empty() {
        return false;
    }
    let along_rank = checkers
        .into_iter()
        .all(|c| c.rank() == home && matches!(board.role_at(c), Some(Role::Rook | Role::Queen)));
    if !along_rank {
        return false;
    }
    let front = attacks::king_attacks(king) & Bitboard::from_rank(front_rank);
    front.any() && (front & !board.by_color(color)).is_empty()
}
