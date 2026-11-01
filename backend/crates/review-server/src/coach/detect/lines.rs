//! Line tactics: pins, skewers, and attacks on pinned pieces.

use shakmaty::{Position, Role};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

fn is_slider(role: Role) -> bool {
    matches!(role, Role::Bishop | Role::Rook | Role::Queen)
}

/// The moved line piece pins one of the other side's pieces (not a pawn), to
/// the king or to a piece worth more than the pinner (so moving the pinned
/// piece loses material). The pinner must not simply be lost. A pinned pawn
/// matters when it cannot take back, which is [`exploits_pin`].
pub fn pin(ctx: &MoveCtx<'_>) -> Option<Found> {
    let pinner = ctx.moved().filter(|p| is_slider(p.role))?;
    if see::capture_gain(ctx.after, pinner.square) > 0 {
        return None;
    }
    let board = ctx.after.board();
    let opponent = ctx.mover().other();
    let before = facts::pins(ctx.before.board(), opponent);
    facts::pins(board, opponent)
        .into_iter()
        .filter(|p| p.pinner == pinner.square)
        .filter(|p| board.role_at(p.pinned) != Some(Role::Pawn))
        .filter(|p| {
            !before
                .iter()
                .any(|q| q.pinned == p.pinned && q.behind == p.behind)
        })
        .filter(|p| {
            p.absolute
                || board
                    .role_at(p.behind)
                    .is_some_and(|r| value(r) > value(pinner.role))
        })
        .filter_map(|p| {
            Some((
                p,
                PieceAt::on(board, p.pinned)?,
                PieceAt::on(board, p.behind)?,
            ))
        })
        .max_by_key(|(_, pinned, _)| value(pinned.role))
        .map(|(p, pinned, behind)| Found {
            tag: Tag::Pin,
            focus: Some(pinned),
            targets: vec![behind],
            geometry: Geometry::XRay {
                attacker: p.pinner,
                front: p.pinned,
                behind: p.behind,
            },
            stake: facts::threat(board, p.pinned, ctx.mover()),
        })
}

/// The move goes after a pinned piece: it newly attacks a piece that an
/// existing pin holds in place (and the attack wins material or comes from a
/// cheaper piece), or it captures something whose defender is pinned to its
/// king and may not take back.
pub fn exploits_pin(ctx: &MoveCtx<'_>) -> Option<Found> {
    let mover = ctx.mover();
    let attacker = ctx.moved()?;
    let board = ctx.after.board();
    let attacked_before = ctx
        .m
        .from()
        .map(|from| ctx.before.board().attacks_from(from))
        .unwrap_or_default();
    let attack = facts::pins(board, mover.other())
        .into_iter()
        .filter(|p| p.pinner != attacker.square)
        .filter(|p| board.attacks_from(attacker.square).contains(p.pinned))
        .filter(|p| !attacked_before.contains(p.pinned))
        .filter_map(|p| Some((p, PieceAt::on(board, p.pinned)?)))
        .filter(|(p, pinned)| {
            facts::threat(board, p.pinned, mover) > 0 || value(pinned.role) > value(attacker.role)
        })
        .max_by_key(|(_, pinned)| value(pinned.role));
    if let Some((p, pinned)) = attack {
        return Some(Found {
            tag: Tag::ExploitsPin,
            focus: Some(pinned),
            targets: PieceAt::on(board, p.behind).into_iter().collect(),
            geometry: Geometry::XRay {
                attacker: p.pinner,
                front: p.pinned,
                behind: p.behind,
            },
            stake: facts::threat(board, p.pinned, mover),
        });
    }

    // A capture the pinned defender cannot answer.
    let captured = ctx.m.capture()?;
    let to = ctx.to();
    let guard = facts::pinned_guards(ctx.before.board(), to, mover.other())
        .into_iter()
        .next()?;
    let net = value(captured) - see::capture_gain(ctx.after, to);
    if net < 100 {
        return None;
    }
    Some(Found {
        tag: Tag::ExploitsPin,
        focus: PieceAt::on(board, guard.pinned),
        targets: vec![PieceAt {
            role: captured,
            color: mover.other(),
            square: to,
        }],
        geometry: Geometry::XRay {
            attacker: guard.pinner,
            front: guard.pinned,
            behind: guard.behind,
        },
        stake: net,
    })
}

/// The moved line piece attacks a valuable piece that has to move (the king,
/// a piece worth more than the attacker, or a hanging one), and the less
/// valuable piece behind it on the same line is then lost. When the line goes
/// on, it shows the skewer: the front piece steps off the line and the mover
/// takes the piece behind.
pub fn skewer(ctx: &MoveCtx<'_>) -> Option<Found> {
    let attacker = ctx.moved().filter(|p| is_slider(p.role))?;
    if see::capture_gain(ctx.after, attacker.square) > 0 {
        return None;
    }
    let board = ctx.after.board();
    let mover = ctx.mover();
    facts::xrays(board, attacker.square)
        .into_iter()
        .filter_map(|x| {
            Some((
                x,
                PieceAt::on(board, x.front)?,
                PieceAt::on(board, x.behind)?,
            ))
        })
        .filter(|(_, front, behind)| front.color != mover && behind.color != mover)
        .filter(|(x, _, behind)| match ctx.rest {
            [reply, take, ..] => {
                reply.from() == Some(x.front)
                    && take.to() == x.behind
                    && take.capture() == Some(behind.role)
            }
            _ => true,
        })
        .filter(|(_, front, behind)| {
            behind.role != Role::King && value(front.role) > value(behind.role)
        })
        .filter(|(x, front, _)| {
            front.role == Role::King
                || value(front.role) > value(attacker.role)
                || facts::threat(board, x.front, mover) > 0
        })
        .filter_map(|(x, front, behind)| {
            let mut without_front = board.clone();
            without_front.discard_piece_at(x.front);
            let gain = facts::threat(&without_front, x.behind, mover);
            (gain > 0).then_some((x, front, behind, gain))
        })
        .max_by_key(|(_, _, behind, _)| value(behind.role))
        .map(|(x, front, behind, gain)| Found {
            tag: Tag::Skewer,
            focus: Some(attacker),
            targets: vec![front, behind],
            geometry: Geometry::XRay {
                attacker: x.attacker,
                front: x.front,
                behind: x.behind,
            },
            stake: gain,
        })
}
