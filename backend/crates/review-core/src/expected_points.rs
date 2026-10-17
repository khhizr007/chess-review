//! Expected points (win probability) and expected-outcome buckets.
//! Functional spec §5.1–§5.2 (Confirmed).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::ReviewConfig;
use crate::value::Value;

/// Rating used when there is no game context at all (`elo_rating_for_expected_points`).
pub const RATING_WITHOUT_GAME: i32 = 2000;

/// Rating fed to the expected-points curve: `max(white, black)` clamped to the
/// configured range. Missing ratings count as 0, so an unrated game uses the minimum.
pub fn rating_for_expected_points(
    white: Option<u32>,
    black: Option<u32>,
    cfg: &ReviewConfig,
) -> i32 {
    let highest = white.unwrap_or(0).max(black.unwrap_or(0));
    let highest = i32::try_from(highest).unwrap_or(i32::MAX);
    highest.clamp(cfg.rating_min, cfg.rating_max)
}

/// Slope of the logistic curve per centipawn for a rating.
fn slope(rating: i32) -> f64 {
    0.004 * ((f64::from(rating) * 0.0685 - 12.0) / 100.0)
}

/// `TEP::centipawns_to_expected_points`.
pub fn cp_to_expected_points(cp: i32, rating: i32) -> f64 {
    let e = (-slope(rating) * f64::from(cp)).exp();
    (2.0 / (e + 1.0) - 1.0) * 0.5 + 0.5
}

/// `TEP::expected_points_to_centipawns` (truncates toward zero like the reference).
pub fn expected_points_to_cp(ep: f64, rating: i32) -> i32 {
    let cp = (1.0 / ep - 1.0).ln() / (-slope(rating));
    cp as i32
}

/// Expected points of the mover after a move; 0.5 when the position is drawn by rule.
pub fn node_expected_points(value: Value, is_draw: bool, rating: i32) -> f64 {
    if is_draw {
        0.5
    } else {
        cp_to_expected_points(value.0, rating)
    }
}

/// Expected-outcome bucket, from the mover's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ExpectedOutcome {
    Lost = 0,
    Losing = 1,
    Worse = 2,
    Equal = 3,
    Better = 4,
    Winning = 5,
    CompletelyWinning = 6,
}

/// `TEP::expected_outcome(Value, rating)`.
pub fn expected_outcome(value: Value, rating: i32, cfg: &ReviewConfig) -> ExpectedOutcome {
    use ExpectedOutcome::*;
    const ORDER: [ExpectedOutcome; 6] = [Losing, Worse, Equal, Better, Winning, CompletelyWinning];
    for (i, bucket) in ORDER.iter().enumerate().rev() {
        if value.0 >= expected_points_to_cp(cfg.outcome_thresholds[i], rating) {
            return *bucket;
        }
    }
    Lost
}

/// Bucket for an expected-points value directly (used for draws and summaries).
pub fn outcome_for_expected_points(ep: f64, cfg: &ReviewConfig) -> ExpectedOutcome {
    use ExpectedOutcome::*;
    const ORDER: [ExpectedOutcome; 6] = [Losing, Worse, Equal, Better, Winning, CompletelyWinning];
    for (i, bucket) in ORDER.iter().enumerate().rev() {
        if ep >= cfg.outcome_thresholds[i] {
            return *bucket;
        }
    }
    Lost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{Eval, Side};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn recovered_curve_values() {
        assert!(close(cp_to_expected_points(100, 1000), 0.5563));
        assert!(close(cp_to_expected_points(100, 1500), 0.5898));
        assert!(close(cp_to_expected_points(100, 2500), 0.6540));
        assert_eq!(cp_to_expected_points(0, 1500), 0.5);
        assert!(close(cp_to_expected_points(-100, 1500), 1.0 - 0.5898));
    }

    #[test]
    fn inverse_round_trips() {
        for rating in [1000, 1500, 2000, 2500] {
            for cp in [-800, -150, -20, 35, 240, 900] {
                let back = expected_points_to_cp(cp_to_expected_points(cp, rating), rating);
                assert!(
                    (back - cp).abs() <= 1,
                    "rating {rating} cp {cp} back {back}"
                );
            }
        }
    }

    #[test]
    fn rating_rule_uses_higher_rating_and_clamps() {
        let cfg = ReviewConfig::default();
        assert_eq!(
            rating_for_expected_points(Some(1200), Some(1850), &cfg),
            1850
        );
        assert_eq!(rating_for_expected_points(Some(3100), None, &cfg), 2500);
        assert_eq!(rating_for_expected_points(None, None, &cfg), 1000);
        assert_eq!(rating_for_expected_points(Some(640), Some(800), &cfg), 1000);
    }

    #[test]
    fn mate_scores_saturate() {
        let win = Value::after_move(
            Eval::Mate {
                winner: Side::White,
                moves: 3,
            },
            Side::White,
        );
        assert!(node_expected_points(win, false, 1000) > 0.999_999);
        let lose = Value::after_move(
            Eval::Mate {
                winner: Side::Black,
                moves: 3,
            },
            Side::White,
        );
        assert!(node_expected_points(lose, false, 1000) < 1e-6);
        assert_eq!(node_expected_points(Value(400), true, 1500), 0.5);
    }

    #[test]
    fn outcome_buckets_follow_recovered_thresholds() {
        let cfg = ReviewConfig::default();
        let r = 1500;
        let at = |ep: f64| expected_outcome(Value(expected_points_to_cp(ep, r)), r, &cfg);
        assert_eq!(at(0.05), ExpectedOutcome::Lost);
        assert_eq!(at(0.15), ExpectedOutcome::Losing);
        assert_eq!(at(0.30), ExpectedOutcome::Worse);
        assert_eq!(at(0.50), ExpectedOutcome::Equal);
        assert_eq!(at(0.70), ExpectedOutcome::Better);
        assert_eq!(at(0.85), ExpectedOutcome::Winning);
        assert_eq!(at(0.95), ExpectedOutcome::CompletelyWinning);
        assert_eq!(
            outcome_for_expected_points(0.5, &cfg),
            ExpectedOutcome::Equal
        );
    }
}
