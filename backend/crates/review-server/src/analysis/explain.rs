//! Factual move descriptions generated from engine output.

use review_core::{Classification, MissKind, Side, Value};

use crate::openings::OpeningName;

pub struct MoveContext<'a> {
    pub mover: Side,
    pub classification: Option<Classification>,
    pub miss_kind: Option<MissKind>,
    pub played_is_best: bool,
    pub best_san: Option<&'a str>,
    pub played: Value,
    pub best: Value,
    pub ep_before: f64,
    pub ep_after: f64,
    pub material_lost_cp: i32,
    pub best_gain_cp: i32,
    pub opening: Option<&'a OpeningName>,
}

fn pawns(cp: i32) -> String {
    let p = f64::from(cp) / 100.0;
    if (p - 1.0).abs() < 0.05 {
        "1 pawn".to_owned()
    } else {
        format!("{p:.1} pawns")
    }
}

fn moves(n: i32) -> String {
    if n == 1 {
        "1 move".to_owned()
    } else {
        format!("{n} moves")
    }
}

pub fn describe(ctx: &MoveContext<'_>) -> Vec<String> {
    let mut out = Vec::new();
    match ctx.classification {
        Some(Classification::Book) => {
            out.push(match ctx.opening {
                Some(o) => format!("Book move: {} ({}).", o.name, o.eco),
                None => "Book move.".to_owned(),
            });
        }
        Some(Classification::Forced) => out.push("This was the only legal move.".to_owned()),
        _ => {}
    }

    match ctx.played.mate_moves() {
        Some(0) => out.push("Checkmate.".to_owned()),
        Some(n) if n < 0 => out.push(format!("This allows a forced mate in {}.", moves(-n))),
        Some(n) => out.push(format!("This keeps a forced mate in {} more.", moves(n))),
        None => {}
    }

    if !ctx.played_is_best
        && let Some(best) = ctx.best_san
    {
        match ctx.best.mate_moves() {
            Some(n) if n >= 0 && ctx.played.mate_moves().is_none_or(|p| p < 0 || p > n) => {
                out.push(format!("{best} forces mate in {}.", moves(n + 1)));
            }
            _ => {
                if ctx
                    .classification
                    .is_some_and(Classification::shows_best_move)
                {
                    out.push(format!("The engine's top choice was {best}."));
                }
            }
        }
        if ctx.best_gain_cp >= 200 && ctx.best.mate_moves().is_none() {
            out.push(format!(
                "In the engine line, {best} wins about {} of material.",
                pawns(ctx.best_gain_cp)
            ));
        }
    }

    if !ctx.played_is_best && ctx.material_lost_cp >= 100 && ctx.played.mate_moves().is_none() {
        out.push(format!(
            "In the engine line after this move, {} loses about {} of material.",
            side_name(ctx.mover),
            pawns(ctx.material_lost_cp)
        ));
    }

    if ctx.miss_kind == Some(MissKind::Opportunity) {
        out.push("The opponent's previous move gave an opportunity that was not taken.".to_owned());
    }

    out.push(format!(
        "Expected score for {}: {:.0}% before the move, {:.0}% after.",
        side_name(ctx.mover),
        ctx.ep_before * 100.0,
        ctx.ep_after * 100.0
    ));
    out
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::White => "White",
        Side::Black => "Black",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use review_core::Eval;

    #[test]
    fn missed_mate_is_described_with_parent_move_count() {
        let mate_after_best = Value::after_move(
            Eval::Mate {
                winner: Side::White,
                moves: 0,
            },
            Side::White,
        );
        let ctx = MoveContext {
            mover: Side::White,
            classification: Some(Classification::Miss),
            miss_kind: Some(MissKind::Mate),
            played_is_best: false,
            best_san: Some("Qxf7#"),
            played: Value(250),
            best: mate_after_best,
            ep_before: 0.99,
            ep_after: 0.70,
            material_lost_cp: 0,
            best_gain_cp: 0,
            opening: None,
        };
        let text = describe(&ctx);
        assert!(
            text.iter().any(|s| s == "Qxf7# forces mate in 1 move."),
            "{text:?}"
        );
        assert!(text.last().unwrap().contains("99%"));
    }
}
