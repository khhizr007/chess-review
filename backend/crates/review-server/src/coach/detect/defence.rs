//! Defence: how a move answers a threat (group 8).
//!
//! A piece is threatened when the opponent could win it by exchange before the
//! move. Where the opponent's null-move line before the move is known (what
//! they would play if it were their turn), that line must also take the piece
//! with one of its first two moves, so a static threat the opponent would not
//! carry out is not called one.

use shakmaty::{Move, Position, Role, Square, attacks};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

/// Defence tags of the played move `ctx.m`, best first. `threat` is the
/// opponent's null-move line before the move, when it is known.
pub fn defence_tags(ctx: &MoveCtx<'_>, threat: Option<&[Move]>) -> Vec<Found> {
    let mut out: Vec<Found> = [
        defends_piece(ctx, threat),
        escapes_threat(ctx, threat),
        blocks_check(ctx),
        evades_check(ctx),
        counter_threat(ctx, threat),
    ]
    .into_iter()
    .flatten()
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// The mover's piece on `sq` was under threat before the move.
fn threatened(ctx: &MoveCtx<'_>, sq: Square, threat: Option<&[Move]>) -> bool {
    facts::threat(ctx.before.board(), sq, ctx.mover().other()) > 0
        && threat.is_none_or(|line| {
            line.iter()
                .step_by(2)
                .take(2)
                .any(|m| m.to() == sq && m.is_capture())
        })
}

/// A threatened piece that stays where it is and is safe after the move,
/// because the moved piece now guards it or stands in the attacker's way.
fn defends_piece(ctx: &MoveCtx<'_>, threat: Option<&[Move]>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx.moved().filter(|_| !ctx.m.is_castle())?;
    let mover = ctx.mover();
    let before = ctx.before.board();
    let board = ctx.after.board();
    (before.by_color(mover) & !before.kings())
        .without(from)
        .into_iter()
        .filter(|t| threatened(ctx, *t, threat))
        .filter(|t| board.color_at(*t) == Some(mover))
        .filter(|t| see::capture_gain(ctx.after, *t) == 0)
        .filter_map(|t| {
            let attacker = facts::least_valuable_attacker(before, t, mover.other())?;
            let guards = board.attacks_from(moved.square).contains(t);
            let blocks = attacks::between(attacker, t).contains(moved.square);
            let piece = PieceAt::on(board, t)?;
            (guards || blocks).then_some((piece, attacker, guards))
        })
        .max_by_key(|(piece, ..)| value(piece.role))
        .map(|(piece, attacker, guards)| Found {
            tag: Tag::DefendsPiece,
            focus: Some(piece),
            targets: Vec::new(),
            // A guard is drawn guarding; a block is drawn as the attack it
            // now stands in.
            geometry: if guards {
                Geometry::Triangle {
                    defended: piece.square,
                    defender: moved.square,
                    attacker,
                }
            } else {
                Geometry::XRay {
                    attacker,
                    front: moved.square,
                    behind: piece.square,
                }
            },
            stake: facts::threat(before, piece.square, mover.other()),
        })
}

/// The moved piece (not the king) was threatened and is safe where it goes.
/// A capture is said as a capture (often it takes the attacker back).
fn escapes_threat(ctx: &MoveCtx<'_>, threat: Option<&[Move]>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx
        .moved()
        .filter(|p| p.role != Role::King && !ctx.m.is_castle() && !ctx.m.is_capture())?;
    let opponent = ctx.mover().other();
    if !threatened(ctx, from, threat) || see::capture_gain(ctx.after, moved.square) > 0 {
        return None;
    }
    let attacker = facts::least_valuable_attacker(ctx.before.board(), from, opponent)?;
    Some(Found {
        tag: Tag::EscapesThreat,
        focus: Some(moved),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: attacker,
            to: from,
        },
        stake: facts::threat(ctx.before.board(), from, opponent),
    })
}

/// A piece other than the king steps between a checking line piece and the king.
fn blocks_check(ctx: &MoveCtx<'_>) -> Option<Found> {
    let checkers = ctx.before.checkers();
    if checkers.count() != 1 {
        return None;
    }
    let checker = checkers.first()?;
    let moved = ctx.moved().filter(|p| p.role != Role::King)?;
    let king = ctx.before.board().king_of(ctx.mover())?;
    if ctx.to() == checker || !attacks::between(checker, king).contains(ctx.to()) {
        return None;
    }
    Some(Found {
        tag: Tag::BlocksCheck,
        focus: Some(moved),
        targets: PieceAt::on(ctx.after.board(), king).into_iter().collect(),
        geometry: Geometry::XRay {
            attacker: checker,
            front: ctx.to(),
            behind: king,
        },
        stake: 0,
    })
}

/// The king steps out of a check (not by castling, which cannot happen in
/// check), or a piece captures the only checking piece.
fn evades_check(ctx: &MoveCtx<'_>) -> Option<Found> {
    let checkers = ctx.before.checkers();
    let checker = checkers.first()?;
    let moved = ctx.moved()?;
    let king = ctx.before.board().king_of(ctx.mover())?;
    let captures_checker = checkers.count() == 1 && ctx.to() == checker;
    if moved.role != Role::King && !captures_checker {
        return None;
    }
    Some(Found {
        tag: Tag::EvadesCheck,
        focus: Some(moved),
        targets: PieceAt::on(ctx.before.board(), checker)
            .into_iter()
            .collect(),
        geometry: Geometry::Relation {
            from: checker,
            to: king,
        },
        stake: 0,
    })
}

/// A threatened piece is left where it is, and the moved piece attacks an
/// opponent piece (not the king) that it can now win, for at least as much.
fn counter_threat(ctx: &MoveCtx<'_>, threat: Option<&[Move]>) -> Option<Found> {
    let moved = ctx.moved().filter(|_| !ctx.m.is_castle())?;
    if ctx.after.is_check() {
        return None;
    }
    let mover = ctx.mover();
    let board = ctx.after.board();
    let (left, at_stake) = (board.by_color(mover) & !board.kings())
        .without(moved.square)
        .into_iter()
        .filter(|t| threatened(ctx, *t, threat))
        .filter_map(|t| PieceAt::on(board, t).map(|p| (p, see::capture_gain(ctx.after, t))))
        .filter(|(_, gain)| *gain > 0)
        .max_by_key(|(_, gain)| *gain)?;
    let (target, stake) = (board.attacks_from(moved.square) & board.by_color(mover.other()))
        .into_iter()
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| t.role != Role::King)
        .filter(|t| facts::threat(ctx.before.board(), t.square, mover) == 0)
        .map(|t| (t, facts::threat(board, t.square, mover)))
        .filter(|(_, gain)| *gain >= at_stake)
        .max_by_key(|(_, gain)| *gain)?;
    Some(Found {
        tag: Tag::CounterThreat,
        focus: Some(target),
        targets: vec![left],
        geometry: Geometry::Relation {
            from: moved.square,
            to: target.square,
        },
        stake,
    })
}
