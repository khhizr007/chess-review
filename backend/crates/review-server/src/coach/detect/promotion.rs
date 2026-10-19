//! Pawn promotion.

use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::value;
use crate::coach::geometry::Geometry;

/// The move promotes a pawn, and the new piece is not simply lost.
pub fn promotion(ctx: &MoveCtx<'_>) -> Option<Found> {
    let role = ctx.m.promotion()?;
    let to = ctx.to();
    let lost = see::capture_gain(ctx.after, to);
    let stake = value(role) - 100 - lost;
    (stake > 0).then(|| Found {
        tag: Tag::Promotion,
        focus: ctx.moved(),
        targets: Vec::new(),
        geometry: Geometry::Relation {
            from: ctx.m.from().unwrap_or(to),
            to,
        },
        stake,
    })
}
