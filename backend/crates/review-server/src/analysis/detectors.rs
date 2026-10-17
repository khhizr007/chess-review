//! Board-event detectors (fallback heuristics; see `docs/implementation-notes.md`).
//!
//! The reference engine derives Brilliant / Great Find / Miss facts from about
//! 250 tactical tags and machine-learned models that were not recovered. These
//! detectors use material changes along engine principal variations instead.
//! Their outputs feed the recovered decision tree in `review-core`.

use review_core::config::BrilliantConfig;
use shakmaty::{Chess, Color, Move, Position};

use crate::board;

/// Material profile of a line from the mover's point of view (cp).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineMaterial {
    /// Balance before the first move of the line.
    pub start: i32,
    /// Balance after the last inspected ply.
    pub end: i32,
    /// Lowest / highest balance after any inspected ply.
    pub min: i32,
    pub max: i32,
    /// Most valuable mover piece captured by the opponent within the horizon.
    pub lost_piece_value: i32,
    /// Value captured by the first move of the line, if it is a capture.
    pub first_capture_value: Option<i32>,
    pub plies: u32,
}

impl LineMaterial {
    pub fn net(&self) -> i32 {
        self.end - self.start
    }

    pub fn lost(&self) -> i32 {
        (self.start - self.end).max(0)
    }

    /// Largest drop below the starting balance at any point of the line.
    pub fn dip(&self) -> i32 {
        (self.start - self.min).max(0)
    }
}

/// Extra plies allowed past the horizon to finish an exchange or check sequence.
const QUIESCENCE_EXTENSION: u32 = 6;

/// Plays `pv` (UCI, starting with the mover's move) from `start` for
/// `horizon` plies, extended while captures, promotions or checks continue,
/// and records the material balance for `mover`.
pub fn walk_line(start: &Chess, mover: Color, pv: &[String], horizon: u32) -> LineMaterial {
    let mut pos = start.clone();
    let initial = board::balance(pos.board(), mover);
    let mut walk = LineMaterial {
        start: initial,
        end: initial,
        min: initial,
        max: initial,
        lost_piece_value: 0,
        first_capture_value: None,
        plies: 0,
    };
    let mut last_was_forcing = false;
    for (i, text) in pv.iter().enumerate() {
        // Stop at the horizon, but only at a quiet point (not mid-exchange or in check).
        let past_horizon = i >= horizon as usize;
        if (past_horizon && !last_was_forcing) || i >= (horizon + QUIESCENCE_EXTENSION) as usize {
            break;
        }
        let Some(m) = board::parse_uci(&pos, text) else {
            break;
        };
        last_was_forcing = m.is_capture() || m.is_promotion();
        if let Some(role) = m.capture() {
            let value = board::role_value(role);
            if i == 0 {
                walk.first_capture_value = Some(value);
            }
            if pos.turn() != mover {
                walk.lost_piece_value = walk.lost_piece_value.max(value);
            }
        }
        pos.play_unchecked(m);
        last_was_forcing = last_was_forcing || pos.is_check();
        let b = board::balance(pos.board(), mover);
        walk.end = b;
        walk.min = walk.min.min(b);
        walk.max = walk.max.max(b);
        walk.plies += 1;
    }
    walk
}

/// Sacrifice test: a non-pawn piece is given up and material dips below the
/// starting balance by at least the configured amount.
pub fn is_sacrifice(line: &LineMaterial, cfg: &BrilliantConfig) -> bool {
    line.lost_piece_value >= cfg.min_piece_value_cp && line.dip() >= cfg.min_sacrifice_cp
}

/// Recovered gate for Brilliant/Great Find: the mover is not already ahead in
/// material by at least the value of the piece involved.
pub fn material_gate(balance_after_move: i32, piece_value: i32) -> bool {
    balance_after_move < piece_value
}

/// The first move captures a piece that is not won back within the horizon.
pub fn takes_free_piece(line: &LineMaterial, min_value: i32) -> bool {
    match line.first_capture_value {
        Some(value) if value >= min_value => line.net() >= value - 100,
        _ => false,
    }
}

/// `played` recaptures on the square where the opponent just captured.
pub fn is_recapture(previous: Option<Move>, played: Move) -> bool {
    match previous {
        Some(prev) => prev.is_capture() && played.is_capture() && prev.to() == played.to(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::CastlingMode;
    use shakmaty::fen::Fen;

    fn pos(fen: &str) -> Chess {
        fen.parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap()
    }

    fn line(moves: &[&str]) -> Vec<String> {
        moves.iter().map(|m| (*m).to_owned()).collect()
    }

    #[test]
    fn bishop_sacrifice_on_f7_is_detected() {
        // Italian-style position: Bxf7+ Kxf7 gives a bishop for a pawn.
        let p = pos("r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/3P1N2/PPP2PPP/RNBQK2R w KQkq - 0 5");
        let walk = walk_line(&p, Color::White, &line(&["c4f7", "e8f7"]), 6);
        assert_eq!(walk.dip(), 200);
        assert_eq!(walk.lost_piece_value, 300);
        assert!(is_sacrifice(&walk, &BrilliantConfig::default()));
    }

    #[test]
    fn even_trade_is_not_a_sacrifice() {
        let p = pos("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3");
        // Nxe5 d6 Nf3 Nxe4: pawn trade, never below the start.
        let walk = walk_line(
            &p,
            Color::White,
            &line(&["f3e5", "d7d6", "e5f3", "f6e4"]),
            6,
        );
        assert_eq!(walk.net(), 0);
        assert!(!is_sacrifice(&walk, &BrilliantConfig::default()));
    }

    #[test]
    fn walk_finishes_exchanges_past_the_horizon() {
        let p = pos("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3");
        // Horizon 1 would stop right after Nxe5 (+1); the capture extends the walk to d6.
        let walk = walk_line(&p, Color::White, &line(&["f3e5", "d7d6", "e5f3"]), 1);
        assert_eq!(walk.plies, 2);
        assert_eq!(walk.net(), 100);
    }

    #[test]
    fn free_piece_capture() {
        // Undefended queen on d4.
        let p = pos("4k3/8/8/8/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e8e7"]), 6);
        assert_eq!(walk.plies, 2);
        assert!(takes_free_piece(&walk, 300));

        // Defended by a pawn: Rxd4 exd4 wins material but the queen was not free.
        let p = pos("4k3/8/8/4p3/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e5d4"]), 6);
        assert_eq!(walk.net(), 400);
        assert!(!takes_free_piece(&walk, 300));
    }
}
