//! Mates and the named mate patterns.

use shakmaty::{Bitboard, Chess, Color, Position, Rank, Role, Square, attacks};

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
/// taken by its own pieces or covered by the mating side, with at least one
/// of its own pieces among them (the king is boxed in by its own army).
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
    let own = board.by_color(color);
    let mating: Color = color.other();
    let covered =
        |sq: Square| own.contains(sq) || board.attacks_to(sq, mating, board.occupied()).any();
    front.into_iter().all(covered) && (front & own).any()
}
