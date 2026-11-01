//! Score representations.
//!
//! [`Eval`] is the public, engine-facing representation: always from White's
//! point of view. [`Value`] is the internal representation used by the review
//! rules: the score of the position *after* a move, from the perspective of
//! the side that made that move, using the reference engine's integer encoding
//! (centipawns, with mates encoded near ±32000). Keeping that encoding lets the
//! recovered rules be expressed exactly as recovered (functional spec §4, §5.6).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Side to move / colour of a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Side {
    White,
    Black,
}

impl Side {
    pub fn opponent(self) -> Side {
        match self {
            Side::White => Side::Black,
            Side::Black => Side::White,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Side::White => 0,
            Side::Black => 1,
        }
    }
}

/// Engine evaluation of a position, always from White's point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[ts(export)]
pub enum Eval {
    /// Centipawns; positive favours White.
    Cp { cp: i32 },
    /// `winner` delivers mate in `moves` of its own moves. `moves == 0` means
    /// the position on the board is already checkmate.
    Mate { winner: Side, moves: u32 },
}

impl Eval {
    pub const DRAW: Eval = Eval::Cp { cp: 0 };

    /// Converts a UCI score (relative to `side_to_move`) into a White-relative evaluation.
    pub fn from_uci(side_to_move: Side, score: UciScore) -> Eval {
        match score {
            UciScore::Cp(cp) => Eval::Cp {
                cp: if side_to_move == Side::White { cp } else { -cp },
            },
            UciScore::Mate(n) if n > 0 => Eval::Mate {
                winner: side_to_move,
                moves: n.unsigned_abs(),
            },
            UciScore::Mate(n) => Eval::Mate {
                winner: side_to_move.opponent(),
                moves: n.unsigned_abs(),
            },
        }
    }

    /// Score suitable for an evaluation bar or graph: centipawns from White's
    /// perspective, with mates mapped to ±`mate_cp`.
    pub fn white_cp_clamped(self, limit: i32, mate_cp: i32) -> i32 {
        match self {
            Eval::Cp { cp } => cp.clamp(-limit, limit),
            Eval::Mate {
                winner: Side::White,
                ..
            } => mate_cp,
            Eval::Mate {
                winner: Side::Black,
                ..
            } => -mate_cp,
        }
    }
}

/// A score as reported by a UCI engine, relative to the side to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UciScore {
    Cp(i32),
    /// Positive: side to move mates in n moves. Negative: side to move is mated in |n|.
    Mate(i32),
}

/// Reference-engine `Value`: mate scores occupy `[31000, 32000]` in magnitude.
pub const VALUE_MATE: i32 = 32_000;
const MATE_RANGE_LOW: i32 = 31_000;

/// Mover-perspective score of the position after a move (see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Value(pub i32);

impl Value {
    /// Builds the mover-perspective value from a White-relative evaluation of
    /// the position *after* the mover's move.
    pub fn after_move(eval: Eval, mover: Side) -> Value {
        match eval {
            Eval::Cp { cp } => {
                let cp = cp.clamp(-(MATE_RANGE_LOW - 1), MATE_RANGE_LOW - 1);
                Value(if mover == Side::White { cp } else { -cp })
            }
            Eval::Mate { winner, moves } => {
                let moves = moves.min(499) as i32;
                if winner == mover {
                    // Mover delivers mate after `moves` more of its own moves.
                    Value(VALUE_MATE - 2 * moves)
                } else {
                    // Opponent (to move) mates in `moves`; at least one move is needed.
                    let moves = moves.max(1);
                    Value(-(VALUE_MATE - (2 * moves - 1)))
                }
            }
        }
    }

    pub fn is_mate(self) -> bool {
        (MATE_RANGE_LOW..=VALUE_MATE).contains(&self.0.abs())
    }

    /// Mate distance as decoded by the reference engine: the number of moves
    /// still needed to mate. Positive when the mover mates, negative when the
    /// mover gets mated, `Some(0)` when mate has been delivered.
    pub fn mate_moves(self) -> Option<i32> {
        if !self.is_mate() {
            return None;
        }
        let a = self.0.abs();
        let n = if (a >> 8) < 0x7d {
            ((31_999 - a) >> 1) + 1
        } else {
            0
        };
        Some(if self.0 >= 0 { n } else { -n })
    }

    /// Score in pawns, with mate in N mapped to ±(1000 − 10·min(N, 50)).
    pub fn to_pawns(self) -> f64 {
        match self.mate_moves() {
            None => f64::from(self.0) / 100.0,
            Some(n) => {
                let s = f64::from(1000 - 10 * n.abs().min(50));
                if self.0 < 0 { -s } else { s }
            }
        }
    }

    /// True when the mover has just delivered checkmate.
    pub fn is_mate_delivered(self) -> bool {
        self.0 == VALUE_MATE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uci_scores_are_converted_to_white_perspective() {
        assert_eq!(
            Eval::from_uci(Side::Black, UciScore::Cp(50)),
            Eval::Cp { cp: -50 }
        );
        assert_eq!(
            Eval::from_uci(Side::Black, UciScore::Mate(-2)),
            Eval::Mate {
                winner: Side::White,
                moves: 2
            }
        );
        assert_eq!(
            Eval::from_uci(Side::White, UciScore::Mate(3)),
            Eval::Mate {
                winner: Side::White,
                moves: 3
            }
        );
    }

    #[test]
    fn mate_encoding_round_trips_through_reference_decoding() {
        for moves in 0..20u32 {
            let v = Value::after_move(
                Eval::Mate {
                    winner: Side::White,
                    moves,
                },
                Side::White,
            );
            assert_eq!(
                v.mate_moves(),
                Some(moves as i32),
                "winning mate in {moves}"
            );
        }
        for moves in 1..20u32 {
            let v = Value::after_move(
                Eval::Mate {
                    winner: Side::Black,
                    moves,
                },
                Side::White,
            );
            assert_eq!(
                v.mate_moves(),
                Some(-(moves as i32)),
                "losing mate in {moves}"
            );
        }
    }

    #[test]
    fn to_pawns_matches_reference_to_score() {
        assert_eq!(Value(150).to_pawns(), 1.5);
        assert_eq!(Value(-35).to_pawns(), -0.35);
        assert_eq!(Value(VALUE_MATE).to_pawns(), 1000.0);
        let mated_in_3 = Value::after_move(
            Eval::Mate {
                winner: Side::Black,
                moves: 3,
            },
            Side::White,
        );
        assert_eq!(mated_in_3.to_pawns(), -970.0);
        let mate_in_80 = Value::after_move(
            Eval::Mate {
                winner: Side::White,
                moves: 80,
            },
            Side::White,
        );
        assert_eq!(mate_in_80.to_pawns(), 500.0);
    }

    #[test]
    fn mover_perspective_flips_for_black() {
        assert_eq!(
            Value::after_move(Eval::Cp { cp: 120 }, Side::Black),
            Value(-120)
        );
        assert_eq!(
            Value::after_move(Eval::Cp { cp: 120 }, Side::White),
            Value(120)
        );
    }
}
