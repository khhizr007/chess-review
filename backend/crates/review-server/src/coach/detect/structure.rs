//! Pawn structure: passed pawns made and pushed, and weaknesses forced on
//! the opponent (group 13); weaknesses a move makes in its own pawns
//! (group 14).

use review_core::Phase;
use shakmaty::{Bitboard, Color, Position, Role, Square};

use super::{Found, MoveCtx, Tag};
use crate::coach::facts::{self, PieceAt};
use crate::coach::geometry::Geometry;

/// Passed-pawn tags of the move, best first.
pub fn structure_tags(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out: Vec<Found> = [pushes_passed(ctx, phase), makes_passed(ctx)]
        .into_iter()
        .flatten()
        .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// Weaknesses the move makes in its own pawns, best first.
pub fn structure_errors(ctx: &MoveCtx<'_>) -> Vec<Found> {
    let mover = ctx.mover();
    let mut out: Vec<Found> = [
        (
            Tag::DoubledPawns,
            facts::doubled as fn(&shakmaty::Board, Color) -> Bitboard,
        ),
        (Tag::IsolatedPawn, facts::isolated),
        (Tag::BackwardPawn, facts::backward),
    ]
    .into_iter()
    .filter_map(|(tag, weak)| {
        let was = weak(ctx.before.board(), mover);
        let mut new = weak(ctx.after.board(), mover) & !was;
        // A pawn that was already weak and moves on is no new weakness.
        if ctx.m.from().is_some_and(|f| was.contains(f)) {
            new.discard(ctx.to());
        }
        let sq = new
            .into_iter()
            .find(|sq| *sq == ctx.to())
            .or_else(|| new.into_iter().next())?;
        // Doubled pawns already there: the count must grow, not just move.
        if tag == Tag::DoubledPawns
            && weak(ctx.after.board(), mover).count() <= weak(ctx.before.board(), mover).count()
        {
            return None;
        }
        Some(pawn_found(tag, ctx.after.board(), sq, &[]))
    })
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

fn pawn_found(tag: Tag, board: &shakmaty::Board, sq: Square, others: &[Square]) -> Found {
    Found {
        tag,
        focus: PieceAt::on(board, sq),
        targets: others
            .iter()
            .filter_map(|s| PieceAt::on(board, *s))
            .collect(),
        geometry: Geometry::Squares(std::iter::once(sq).chain(others.iter().copied()).collect()),
        stake: 0,
    }
}

/// A pawn that was passed moves on: in the endgame `PushesPassedPawnEndgame`,
/// before it `PushesPassedPawn`.
fn pushes_passed(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    if ctx.m.role() != Role::Pawn
        || ctx.m.is_promotion()
        || !facts::is_passed(ctx.before.board(), from)
    {
        return None;
    }
    let tag = if phase == Phase::Endgame {
        Tag::PushesPassedPawnEndgame
    } else {
        Tag::PushesPassedPawn
    };
    Some(Found {
        tag,
        focus: PieceAt::on(ctx.after.board(), ctx.to()),
        targets: Vec::new(),
        geometry: Geometry::Relation { from, to: ctx.to() },
        stake: 0,
    })
}

/// After the move we have a passed pawn that was not passed before (the
/// moved pawn counts only if it was not passed where it stood).
fn makes_passed(ctx: &MoveCtx<'_>) -> Option<Found> {
    let mover = ctx.mover();
    let before = facts::passed(ctx.before.board(), mover);
    let moved_was_passed = ctx.m.from().is_some_and(|f| before.contains(f));
    let new = facts::passed(ctx.after.board(), mover) & !before;
    let sq = new
        .into_iter()
        .find(|sq| !(moved_was_passed && *sq == ctx.to()))?;
    Some(pawn_found(Tag::MakesPassedPawn, ctx.after.board(), sq, &[]))
}

/// A capture that the opponent's next move in the line takes back with a
/// pawn, leaving it doubled pawns (`ForcesDoubledPawns`) or an isolated pawn
/// (`ForcesIsolatedPawn`) it did not have.
pub fn forces_structure(ctx: &MoveCtx<'_>) -> Option<Found> {
    let reply = ctx.rest.first()?;
    if !ctx.m.is_capture()
        || reply.role() != Role::Pawn
        || reply.to() != ctx.to()
        || !reply.is_capture()
    {
        return None;
    }
    let mut after = ctx.after.clone();
    after.play_unchecked(*reply);
    let them = ctx.mover().other();
    let board = after.board();
    let to = ctx.to();
    let doubled = facts::doubled(board, them) & Bitboard::from_file(to.file());
    if doubled.contains(to)
        && !facts::doubled(ctx.before.board(), them).intersects(Bitboard::from_file(to.file()))
    {
        let mut found = pawn_found(Tag::ForcesDoubledPawns, board, to, &[]);
        found.targets = doubled
            .into_iter()
            .filter_map(|sq| PieceAt::on(board, sq))
            .collect();
        found.geometry = Geometry::Squares(doubled.into_iter().collect());
        return Some(found);
    }
    let isolated = facts::isolated(board, them) & !facts::isolated(ctx.before.board(), them);
    let sq = isolated
        .into_iter()
        .find(|sq| *sq == to)
        .or_else(|| isolated.into_iter().next())?;
    Some(pawn_found(Tag::ForcesIsolatedPawn, board, sq, &[]))
}
