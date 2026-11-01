//! Defender tactics: removing a defender, overloading one, luring a piece.

use shakmaty::{Position, Role};

use super::{Found, MoveCtx, Tag};
use crate::coach::facts::{self, PieceAt};
use crate::coach::geometry::Geometry;

/// The mover takes `t` on its square with one of its next two moves in the
/// line: what a defender tactic claims is then won is won.
fn taken_later(ctx: &MoveCtx<'_>, t: &PieceAt) -> bool {
    ctx.rest
        .iter()
        .skip(1)
        .step_by(2)
        .take(2)
        .any(|m| m.to() == t.square && m.capture() == Some(t.role))
}

/// The move captures a piece that was guarding another of the other side's
/// pieces, which was safe before, can now be won, and is won in the line.
pub fn removes_defender(ctx: &MoveCtx<'_>) -> Option<Found> {
    let captured = ctx.m.capture().filter(|r| *r != Role::King)?;
    let to = ctx.to();
    let mover = ctx.mover();
    let before = ctx.before.board();
    let after = ctx.after.board();
    let defender = PieceAt {
        role: captured,
        color: mover.other(),
        square: to,
    };
    facts::defended_by(before, to)
        .into_iter()
        .filter_map(|t| PieceAt::on(before, t))
        .filter(|t| t.role != Role::King)
        .filter(|t| after.piece_at(t.square).is_some_and(|p| p.color == t.color))
        .filter(|t| facts::threat(before, t.square, mover) == 0)
        .filter(|t| taken_later(ctx, t))
        .filter_map(|t| {
            let gain = facts::threat(after, t.square, mover);
            (gain >= 100).then_some((t, gain))
        })
        .max_by_key(|(_, gain)| *gain)
        .map(|(t, gain)| Found {
            tag: Tag::RemovesDefender,
            focus: Some(defender),
            targets: vec![t],
            geometry: Geometry::Triangle {
                defended: t.square,
                defender: to,
                attacker: facts::least_valuable_attacker(after, t.square, mover).unwrap_or(to),
            },
            stake: gain,
        })
}

/// The move captures on a square whose only defender also guards another
/// piece: the defender takes back, and that piece is lost in the line.
pub fn overloaded(ctx: &MoveCtx<'_>) -> Option<Found> {
    let captured = ctx.m.capture().filter(|r| *r != Role::King)?;
    if ctx.m.is_en_passant() {
        return None;
    }
    let x = ctx.to();
    let mover = ctx.mover();
    let board = ctx.after.board();
    let defender = facts::attackers(board, x, mover.other()).single_square()?;
    let takes_back = ctx
        .rest
        .first()
        .is_some_and(|r| r.from() == Some(defender) && r.to() == x);
    if !takes_back {
        return None;
    }
    let duty = PieceAt::on(board, defender).filter(|d| d.role != Role::King)?;
    // The position after the defender takes back on `x`.
    let mut recaptured = board.clone();
    recaptured.discard_piece_at(defender);
    recaptured.set_piece_at(
        x,
        shakmaty::Piece {
            color: duty.color,
            role: duty.role,
        },
    );
    facts::defended_by(board, defender)
        .without(x)
        .into_iter()
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| t.role != Role::King)
        .filter(|t| facts::threat(board, t.square, mover) == 0)
        .filter(|t| taken_later(ctx, t))
        .filter_map(|t| {
            let gain = facts::threat(&recaptured, t.square, mover);
            (gain >= 100).then_some((t, gain))
        })
        .max_by_key(|(_, gain)| *gain)
        .map(|(t, gain)| Found {
            tag: Tag::Overloaded,
            focus: Some(duty),
            targets: vec![
                PieceAt {
                    role: captured,
                    color: mover.other(),
                    square: x,
                },
                t,
            ],
            geometry: Geometry::Triangle {
                defended: t.square,
                defender,
                attacker: facts::least_valuable_attacker(board, t.square, mover).unwrap_or(x),
            },
            stake: gain,
        })
}

/// The move goes to a square where the other side's king, queen or rook
/// takes it; the next move attacks the piece there, and either it is the king
/// (now in check) or it is then captured.
pub fn attraction(ctx: &MoveCtx<'_>) -> Option<Found> {
    let [reply, follow, rest @ ..] = ctx.rest else {
        return None;
    };
    let s = ctx.to();
    if ctx.m.is_castle() || reply.to() != s || !reply.is_capture() {
        return None;
    }
    let lured = reply.role();
    if !matches!(lured, Role::King | Role::Queen | Role::Rook) {
        return None;
    }
    let mut pos = ctx.after.clone();
    pos.play_unchecked(*reply);
    pos.play_unchecked(*follow);
    let hits = pos.board().attacks_from(follow.to()).contains(s);
    let pays = if lured == Role::King {
        pos.is_check() && hits
    } else {
        hits && rest.get(1).is_some_and(|m| m.to() == s && m.is_capture())
    };
    pays.then(|| Found {
        tag: Tag::Attraction,
        focus: Some(PieceAt {
            role: lured,
            color: ctx.mover().other(),
            square: s,
        }),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: reply.from().unwrap_or(s),
            to: s,
        },
        stake: 0,
    })
}
