//! Bad-move bases: what a move does wrong by itself (group 7).
//!
//! A static fact alone can mislead (a "hanging" piece may be poisoned), so
//! where the opponent's best line is known, a tag about a lost piece also needs
//! that line to take it.

use shakmaty::{Move, Position, Role, Square, attacks};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

/// Bad-move tags of the played move `ctx.m`, best first. `ctx.rest` is the
/// opponent's best line after it; `best` is the engine's best move instead.
pub fn bad_tags(ctx: &MoveCtx<'_>, best: Option<Move>) -> Vec<Found> {
    let mut out: Vec<Found> = Vec::new();
    out.extend(moves_pinned_piece(ctx));
    out.extend(did_not_recapture(ctx, best));
    out.extend(ignores_threat(ctx));
    let own_defender = removes_own_defender(ctx);
    out.extend(own_defender.clone());
    out.extend(bad_trade(ctx));
    out.extend(hangs(ctx, own_defender.map(|f| f.focus)));
    out.sort_by_key(|f| f.tag);
    out
}

/// The opponent's line takes on `sq` with one of its first two moves (or no
/// line is known).
fn line_takes(rest: &[Move], sq: Square) -> bool {
    rest.is_empty()
        || rest
            .iter()
            .step_by(2)
            .take(2)
            .any(|m| m.to() == sq && m.is_capture())
}

/// An opponent piece aimed at `sq` after the move, for the arrow.
fn attacked_by(ctx: &MoveCtx<'_>, sq: Square) -> Geometry {
    match facts::least_valuable_attacker(ctx.after.board(), sq, ctx.mover().other()) {
        Some(from) => Geometry::Relation { from, to: sq },
        None => Geometry::Squares(vec![sq]),
    }
}

/// The moved piece was pinned to a more valuable piece, leaves the pin line,
/// and the piece behind it can now be won.
fn moves_pinned_piece(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx.moved()?;
    if ctx.m.is_castle() {
        return None;
    }
    let mover = ctx.mover();
    let captured = ctx.m.capture().map_or(0, value);
    facts::pins(ctx.before.board(), mover)
        .into_iter()
        .filter(|p| p.pinned == from && !p.absolute && p.pinner != ctx.to())
        .filter(|p| !attacks::between(p.pinner, p.behind).contains(ctx.to()))
        .filter(|p| {
            ctx.after
                .board()
                .color_at(p.pinner)
                .is_some_and(|c| c != mover)
        })
        .filter_map(|p| {
            let behind = PieceAt::on(ctx.after.board(), p.behind).filter(|b| b.color == mover)?;
            let gain = see::capture_gain(ctx.after, p.behind);
            (gain > captured && line_takes(ctx.rest, p.behind)).then_some((p, behind, gain))
        })
        .max_by_key(|(_, _, gain)| *gain)
        .map(|(p, behind, gain)| Found {
            tag: Tag::MovesPinnedPiece,
            focus: Some(moved),
            targets: vec![behind],
            geometry: Geometry::XRay {
                attacker: p.pinner,
                front: from,
                behind: p.behind,
            },
            stake: gain,
        })
}

/// The opponent just captured, taking back was the best move and wins
/// material back, and the move does something else.
fn did_not_recapture(ctx: &MoveCtx<'_>, best: Option<Move>) -> Option<Found> {
    let previous = ctx.previous.filter(|m| m.is_capture())?;
    let x = previous.to();
    let best = best.filter(|b| b.to() == x && b.is_capture())?;
    if ctx.to() == x {
        return None;
    }
    let gain = see::capture_gain(ctx.before, x);
    if gain < 100 {
        return None;
    }
    Some(Found {
        tag: Tag::DidNotRecapture,
        focus: PieceAt::on(ctx.before.board(), x),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: best.from().unwrap_or(x),
            to: x,
        },
        stake: gain,
    })
}

/// A piece the opponent could already win stays where it is, and is still lost.
fn ignores_threat(ctx: &MoveCtx<'_>) -> Option<Found> {
    let mover = ctx.mover();
    let from = ctx.m.from()?;
    (ctx.before.board().by_color(mover) & !ctx.before.board().kings())
        .without(from)
        .into_iter()
        .filter(|t| facts::threat(ctx.before.board(), *t, mover.other()) > 0)
        .filter_map(|t| {
            let piece = PieceAt::on(ctx.after.board(), t).filter(|p| p.color == mover)?;
            let gain = see::capture_gain(ctx.after, t);
            (gain > 0 && line_takes(ctx.rest, t)).then_some((piece, gain))
        })
        .max_by_key(|(_, gain)| *gain)
        .map(|(piece, gain)| Found {
            tag: Tag::IgnoresThreat,
            focus: Some(piece),
            targets: Vec::new(),
            geometry: attacked_by(ctx, piece.square),
            stake: gain,
        })
}

/// The moved piece was guarding a piece that was safe, no longer guards it,
/// and that piece can now be won.
fn removes_own_defender(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx.moved()?;
    let mover = ctx.mover();
    let still_guarded = ctx.after.board().attacks_from(moved.square);
    facts::defended_by(ctx.before.board(), from)
        .without(ctx.to())
        .into_iter()
        .filter(|t| !still_guarded.contains(*t))
        .filter(|t| facts::threat(ctx.before.board(), *t, mover.other()) == 0)
        .filter_map(|t| {
            let piece = PieceAt::on(ctx.after.board(), t)
                .filter(|p| p.color == mover && p.role != Role::King)?;
            let gain = see::capture_gain(ctx.after, t);
            (gain > 0 && line_takes(ctx.rest, t)).then_some((piece, gain))
        })
        .max_by_key(|(_, gain)| *gain)
        .map(|(piece, gain)| Found {
            tag: Tag::RemovesOwnDefender,
            focus: Some(piece),
            targets: Vec::new(),
            geometry: Geometry::Triangle {
                defended: piece.square,
                defender: from,
                attacker: facts::least_valuable_attacker(
                    ctx.after.board(),
                    piece.square,
                    mover.other(),
                )
                .unwrap_or(piece.square),
            },
            stake: gain,
        })
}

/// The move captures, and the opponent wins back more than it took.
fn bad_trade(ctx: &MoveCtx<'_>) -> Option<Found> {
    let captured = ctx.m.capture()?;
    let moved = ctx.moved()?;
    let lost = see::capture_gain(ctx.after, ctx.to()) - value(captured);
    (lost >= 100 && line_takes(ctx.rest, ctx.to())).then(|| Found {
        tag: Tag::BadTrade,
        focus: Some(moved),
        targets: vec![PieceAt {
            role: captured,
            color: ctx.mover().other(),
            square: ctx.to(),
        }],
        geometry: attacked_by(ctx, ctx.to()),
        stake: lost,
    })
}

/// The move leaves a piece en prise that was safe before: the moved piece on
/// its new square, or one the move uncovered.
fn hangs(ctx: &MoveCtx<'_>, already: Option<Option<PieceAt>>) -> Option<Found> {
    let mover = ctx.mover();
    let reported = already.flatten().map(|p| p.square);
    let moved = ctx
        .moved()
        .filter(|_| !ctx.m.is_castle() && !ctx.m.is_capture());
    let others = (ctx.after.board().by_color(mover) & !ctx.after.board().kings())
        .without(ctx.to())
        .into_iter()
        .filter(|t| Some(*t) != reported)
        .filter(|t| facts::threat(ctx.before.board(), *t, mover.other()) == 0)
        .filter_map(|t| PieceAt::on(ctx.after.board(), t));
    moved
        .into_iter()
        .chain(others)
        .filter(|p| p.role != Role::King)
        .filter_map(|p| {
            let gain = see::capture_gain(ctx.after, p.square);
            (gain > 0 && line_takes(ctx.rest, p.square)).then_some((p, gain))
        })
        .max_by_key(|(_, gain)| *gain)
        .map(|(piece, gain)| Found {
            tag: if piece.role == Role::Pawn {
                Tag::HangsPawn
            } else {
                Tag::HangsPiece
            },
            focus: Some(piece),
            targets: Vec::new(),
            geometry: attacked_by(ctx, piece.square),
            stake: gain,
        })
}
