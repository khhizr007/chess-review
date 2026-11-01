//! CAPS2 move accuracy and its aggregation. Functional spec §8 (Confirmed).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::value::{Side, Value};

/// Game phase of a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Phase {
    Opening,
    Middlegame,
    Endgame,
}

impl Phase {
    pub const ALL: [Phase; 3] = [Phase::Opening, Phase::Middlegame, Phase::Endgame];

    fn index(self) -> usize {
        match self {
            Phase::Opening => 0,
            Phase::Middlegame => 1,
            Phase::Endgame => 2,
        }
    }
}

/// Per-colour "very bad move" floor state carried along the game.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Floor {
    count: u32,
    floor: f64,
    limit: f64,
}

impl Default for Floor {
    fn default() -> Self {
        Self {
            count: 0,
            floor: -100.0,
            limit: -50.0,
        }
    }
}

impl Floor {
    /// Records one more very bad move and returns the floor that applies to it.
    /// The first such move scores -100; each later one moves the floor 25% of
    /// the way toward -50.
    fn next(&mut self) -> f64 {
        let previous = self.count;
        self.count += 1;
        if previous > 0 {
            let candidate = self.floor + (self.floor.abs() - self.limit.abs()) * 0.25;
            self.floor = candidate.min(self.limit);
        }
        self.floor
    }
}

/// State threaded through a game when scoring moves in order.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Caps2State {
    floors: [Floor; 2],
}

/// One move to score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Caps2Move {
    pub mover: Side,
    /// Mover-perspective value after the played move.
    pub played: Value,
    /// Mover-perspective value after the engine's best move.
    pub best: Value,
    pub is_book: bool,
    pub is_forced: bool,
}

/// Bands of the piecewise-linear score, as functions of |played eval| (pawns).
fn thresholds(played_pawns: f64) -> (f64, f64, f64) {
    let x = played_pawns.abs().min(20.0);
    let t1 = 0.579 - 0.138 * x + 0.0187 * x * x;
    let t2 = 1.5 - 0.0699 * x + 0.0265 * x * x;
    let t3 = 3.57 + 0.0291 * x + 0.0279 * x * x;
    (t1, t2, t3)
}

impl Caps2State {
    /// CAPS2 score of one move, 100 = perfect. `None` for forced moves.
    pub fn score(&mut self, m: &Caps2Move) -> Option<f64> {
        if m.is_forced {
            return None;
        }
        if m.is_book || m.played.is_mate_delivered() {
            return Some(100.0);
        }
        let floor = &mut self.floors[m.mover.index()];
        let (played, best) = (m.played, m.best);

        if played.is_mate() {
            if best.is_mate() {
                if played == best {
                    return Some(100.0);
                }
                if played.0 < 0 && best.0 > 0 {
                    return Some(floor.next());
                }
                let md_best = best.mate_moves().unwrap_or(0);
                let md_played = played.mate_moves().unwrap_or(0);
                let extra = (md_best - md_played).abs();
                let allowed = if md_best.abs() > 1 { 2 } else { 1 };
                return Some(if extra <= allowed { 80.0 } else { 70.0 });
            }
            if played.0 >= 0 {
                return Some(100.0);
            }
            // The played move walks into mate; the best move did not.
            let b = best.to_pawns();
            return Some(if b <= -50.0 {
                70.0
            } else if b > -10.0 {
                floor.next()
            } else {
                30.0
            });
        }

        if best.is_mate() {
            if best.0 < 0 {
                // Best line was getting mated; the played move escaped.
                return Some(100.0);
            }
            // Missed a mate.
            let p = played.to_pawns();
            return Some(if p >= 50.0 {
                70.0
            } else if p < 10.0 {
                floor.next()
            } else {
                30.0
            });
        }

        let p = played.to_pawns();
        let delta = best.to_pawns() - p;
        let (t1, t2, t3) = thresholds(p);
        let raw = if delta <= 0.0 {
            100.0
        } else if delta <= t1 {
            100.0 - 20.0 * (delta / t1)
        } else if delta <= t2 {
            80.0 - 50.0 * ((delta - t1) / (t2 - t1))
        } else if delta <= t3 {
            30.0 - 80.0 * ((delta - t2) / (t3 - t2))
        } else {
            let f = floor.next();
            let span = (t3 - t2) * 2.140_096_618_357_488;
            (-50.0 - ((delta - t3) / span) * (-50.0 - f)).max(f)
        };
        Some(clamp_by_position(raw, p, p + delta))
    }
}

/// Position-dependent clamps (applied in order): lopsided positions limit how
/// much a single move can be penalised.
fn clamp_by_position(score: f64, played: f64, best: f64) -> f64 {
    if played >= 30.0 {
        score.max(30.0)
    } else if played >= 10.0 {
        score.max(80.0)
    } else if played >= 6.0 {
        score.max(30.0)
    } else if played >= 5.0 || (played <= -25.0 && best >= -10.0) {
        score.max(-50.0)
    } else if played <= -20.0 && best >= -15.0 {
        score.max(30.0)
    } else if played > -30.0 {
        score
    } else {
        score.max(0.0)
    }
}

/// A scored move for aggregation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScoredMove {
    pub mover: Side,
    pub phase: Phase,
    pub score: Option<f64>,
    /// The opponent was completely winning (with a material lead) before this move.
    pub mover_completely_lost_before: bool,
}

/// Accuracy per side, overall and by phase. `None` when no move was scored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SideAccuracy {
    pub overall: Option<f64>,
    pub opening: Option<f64>,
    pub middlegame: Option<f64>,
    pub endgame: Option<f64>,
    /// Number of moves included in `overall`.
    pub moves_counted: u32,
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some((values.iter().sum::<f64>() / values.len() as f64).max(0.0))
}

/// `compute_caps_for_move_nodes`: averages per side and per phase.
pub fn aggregate(moves: &[ScoredMove], exclude_completely_lost: bool) -> [SideAccuracy; 2] {
    let mut overall: [Vec<f64>; 2] = Default::default();
    let mut by_phase: [[Vec<f64>; 3]; 2] = Default::default();
    for m in moves {
        let Some(score) = m.score else { continue };
        let side = m.mover.index();
        if !m.mover_completely_lost_before {
            by_phase[side][m.phase.index()].push(score);
        }
        if !(exclude_completely_lost && m.mover_completely_lost_before) {
            overall[side].push(score);
        }
    }
    let side = |s: usize| SideAccuracy {
        overall: mean(&overall[s]),
        opening: mean(&by_phase[s][0]),
        middlegame: mean(&by_phase[s][1]),
        endgame: mean(&by_phase[s][2]),
        moves_counted: overall[s].len() as u32,
    };
    [side(0), side(1)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Eval;

    fn mv(played: i32, best: i32) -> Caps2Move {
        Caps2Move {
            mover: Side::White,
            played: Value(played),
            best: Value(best),
            is_book: false,
            is_forced: false,
        }
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn perfect_and_book_moves_score_100() {
        let mut s = Caps2State::default();
        assert_eq!(s.score(&mv(30, 30)), Some(100.0));
        assert_eq!(s.score(&mv(80, 30)), Some(100.0));
        let mut book = mv(-300, 30);
        book.is_book = true;
        assert_eq!(s.score(&book), Some(100.0));
        let mut forced = mv(-300, 30);
        forced.is_forced = true;
        assert_eq!(s.score(&forced), None);
    }

    #[test]
    fn band_boundaries_from_recovered_polynomials() {
        // Played eval 0.00: t1 = 0.579, t2 = 1.5, t3 = 3.57.
        let mut s = Caps2State::default();
        assert!(approx(
            s.score(&mv(0, 57)).unwrap(),
            100.0 - 20.0 * (0.57 / 0.579)
        ));
        assert!(approx(s.score(&mv(0, 150)).unwrap(), 30.0));
        let mid = s.score(&mv(0, 100)).unwrap();
        assert!(approx(mid, 80.0 - 50.0 * ((1.0 - 0.579) / (1.5 - 0.579))));
        let band3 = s.score(&mv(0, 300)).unwrap();
        assert!(approx(band3, 30.0 - 80.0 * ((3.0 - 1.5) / (3.57 - 1.5))));
    }

    #[test]
    fn floor_moves_toward_minus_50_after_repeated_disasters() {
        let mut s = Caps2State::default();
        // Huge loss (+15 to -15): beyond t3 and past the extrapolated span, so the floor applies.
        assert_eq!(s.score(&mv(-1500, 1500)), Some(-100.0));
        assert_eq!(s.score(&mv(-1500, 1500)), Some(-87.5));
        assert_eq!(s.score(&mv(-1500, 1500)), Some(-78.125));
        // Black has its own floor.
        let mut black = mv(-1500, 1500);
        black.mover = Side::Black;
        assert_eq!(s.score(&black), Some(-100.0));
    }

    #[test]
    fn lopsided_positions_are_clamped() {
        let mut s = Caps2State::default();
        // Still +12 after the move: never below 80.
        assert_eq!(s.score(&mv(1200, 2000)), Some(80.0));
        // Already -50 (lost) and still a large drop: never below 0.
        assert_eq!(s.score(&mv(-5000, -3000)), Some(0.0));
    }

    #[test]
    fn mate_cases() {
        let mate = |w: Side, n: u32| {
            Value::after_move(
                Eval::Mate {
                    winner: w,
                    moves: n,
                },
                Side::White,
            )
        };
        let mut s = Caps2State::default();
        let m = |played: Value, best: Value| Caps2Move {
            mover: Side::White,
            played,
            best,
            is_book: false,
            is_forced: false,
        };
        assert_eq!(
            s.score(&m(mate(Side::White, 0), mate(Side::White, 0))),
            Some(100.0)
        );
        assert_eq!(
            s.score(&m(mate(Side::White, 3), mate(Side::White, 1))),
            Some(70.0)
        );
        assert_eq!(
            s.score(&m(mate(Side::White, 2), mate(Side::White, 1))),
            Some(80.0)
        );
        assert_eq!(s.score(&m(Value(6000), mate(Side::White, 2))), Some(70.0));
        assert_eq!(s.score(&m(Value(2000), mate(Side::White, 2))), Some(30.0));
        assert_eq!(s.score(&m(mate(Side::Black, 2), Value(-6000))), Some(70.0));
        assert_eq!(s.score(&m(mate(Side::Black, 2), Value(-2000))), Some(30.0));
        assert_eq!(s.score(&m(Value(-500), mate(Side::Black, 3))), Some(100.0));
    }

    #[test]
    fn aggregation_excludes_hopeless_moves_and_unscored_moves() {
        let moves = [
            ScoredMove {
                mover: Side::White,
                phase: Phase::Opening,
                score: Some(100.0),
                mover_completely_lost_before: false,
            },
            ScoredMove {
                mover: Side::White,
                phase: Phase::Middlegame,
                score: Some(50.0),
                mover_completely_lost_before: false,
            },
            ScoredMove {
                mover: Side::White,
                phase: Phase::Endgame,
                score: Some(-100.0),
                mover_completely_lost_before: true,
            },
            ScoredMove {
                mover: Side::Black,
                phase: Phase::Opening,
                score: None,
                mover_completely_lost_before: false,
            },
        ];
        let [white, black] = aggregate(&moves, true);
        assert_eq!(white.overall, Some(75.0));
        assert_eq!(white.endgame, None);
        assert_eq!(white.moves_counted, 2);
        assert_eq!(black.overall, None);
        let [white_all, _] = aggregate(&moves, false);
        assert_eq!(white_all.overall, Some(50.0f64 / 3.0));
    }
}
