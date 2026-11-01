//! Trades: taking back, even exchanges and offers to exchange (group 8, plus
//! `OffersTrade` in group 10). Trades the mover wins are `WinsTrade` (a free
//! material tactic, in `material.rs`); trades it loses are `LosesTrade` and
//! `BadTrade` (bad-move bases, in `bad.rs`).

use shakmaty::{Position, Role};

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, PieceAt, value};
use crate::coach::geometry::Geometry;

/// Exchanges within this much (cp) are even.
const EVEN_CP: i32 = 100;

/// Trade tags of the move, best first.
pub fn trade_tags(ctx: &MoveCtx<'_>) -> Vec<Found> {
    [recapture(ctx), equal_trade(ctx), offers_trade(ctx)]
        .into_iter()
        .flatten()
        .collect()
}

/// The piece the move captures, as the text names it.
fn captured(ctx: &MoveCtx<'_>) -> Option<PieceAt> {
    let role = ctx.m.capture()?;
    Some(PieceAt {
        role,
        color: ctx.mover().other(),
        square: ctx.to(),
    })
}

/// The opponent just captured on a square and the move takes back there,
/// ending the exchange without losing it. When the opponent can take back
/// again for an even result, the exchange goes on: that is an `EqualTrade`.
fn recapture(ctx: &MoveCtx<'_>) -> Option<Found> {
    if !takes_back(ctx) || ctx.m.is_en_passant() {
        return None;
    }
    let taken = captured(ctx)?;
    let back = see::capture_gain(ctx.after, ctx.to());
    let net = value(taken.role) - back;
    if net <= -EVEN_CP || (back > 0 && net < EVEN_CP) {
        return None;
    }
    Some(Found {
        tag: Tag::Recapture,
        focus: Some(taken),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: ctx.m.from().unwrap_or(ctx.to()),
            to: ctx.to(),
        },
        stake: 0,
    })
}

/// The opponent just captured on the square the move captures on.
fn takes_back(ctx: &MoveCtx<'_>) -> bool {
    ctx.previous
        .is_some_and(|p| p.is_capture() && p.to() == ctx.to())
}

/// The move captures a piece that the opponent can take back with profit,
/// and the exchange comes out even: it starts an even trade, or (taking back)
/// carries one on.
fn equal_trade(ctx: &MoveCtx<'_>) -> Option<Found> {
    let taken = captured(ctx).filter(|p| p.role != Role::King)?;
    let back = see::capture_gain(ctx.after, ctx.to());
    let net = value(taken.role) - back;
    (back > 0 && net.abs() < EVEN_CP).then(|| Found {
        tag: Tag::EqualTrade,
        focus: Some(taken),
        targets: ctx.moved().into_iter().collect(),
        geometry: Geometry::Relation {
            from: ctx.m.from().unwrap_or(ctx.to()),
            to: ctx.to(),
        },
        stake: 0,
    })
}

/// A quiet move puts a knight, bishop, rook or queen where it attacks an
/// opponent piece of the same value that attacks it back, and neither side
/// wins anything by taking first: an offer to trade.
fn offers_trade(ctx: &MoveCtx<'_>) -> Option<Found> {
    if ctx.m.is_capture() || ctx.m.is_castle() || ctx.after.is_check() {
        return None;
    }
    let moved = ctx
        .moved()
        .filter(|p| !matches!(p.role, Role::Pawn | Role::King))?;
    if see::capture_gain(ctx.after, moved.square) > 0 {
        return None;
    }
    let board = ctx.after.board();
    let mover = ctx.mover();
    (board.attacks_from(moved.square) & board.by_color(mover.other()))
        .into_iter()
        .filter_map(|t| PieceAt::on(board, t))
        .filter(|t| t.role != Role::King && t.role != Role::Pawn)
        .filter(|t| (value(t.role) - value(moved.role)).abs() < EVEN_CP)
        .filter(|t| board.attacks_from(t.square).contains(moved.square))
        .filter(|t| facts::threat(board, t.square, mover) == 0)
        .max_by_key(|t| value(t.role))
        .map(|t| Found {
            tag: Tag::OffersTrade,
            focus: Some(t),
            targets: vec![moved],
            geometry: Geometry::Relation {
                from: moved.square,
                to: t.square,
            },
            stake: 0,
        })
}
