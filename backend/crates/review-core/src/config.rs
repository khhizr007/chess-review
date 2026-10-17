//! Review configuration. Defaults are the values recovered from the reference
//! engine (functional spec §5–§8); fields marked *fallback* configure
//! heuristics that stand in for behaviour that was not recovered
//! (see `docs/open-questions.md`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ReviewConfig {
    /// Expected-points-lost upper bounds for Excellent, Good, Inaccuracy, Mistake.
    /// Recovered: `[0.02, 0.05, 0.10, 0.20]`.
    pub precision_thresholds: [f64; 4],
    /// Expected-outcome bucket boundaries. Recovered: `[0.10, 0.20, 0.35, 0.65, 0.80, 0.90]`.
    pub outcome_thresholds: [f64; 6],
    /// Rating clamp applied to `max(WhiteElo, BlackElo)`. Recovered: 1000..=2500.
    pub rating_min: i32,
    pub rating_max: i32,
    /// Material (cp) a Blunder must actually lose to stay a Blunder (fallback for
    /// `is_losing_material_or_getting_mated`).
    pub blunder_material_threshold_cp: i32,
    /// Material lost (cp) that turns Excellent/Good/Inaccuracy into Mistake when
    /// losing. Recovered: 200.
    pub forgiving_material_lost_cp: i32,
    /// Enables the Tricky class. The reference client discards it; default off.
    pub allow_tricky: bool,
    pub brilliant: BrilliantConfig,
    pub great_find: GreatFindConfig,
    pub miss: MissConfig,
    /// Exclude moves made when the mover was already completely lost from the
    /// overall accuracy (Unknown caller flag; default on).
    pub caps_exclude_completely_lost: bool,
    /// Plies of the principal variation inspected for material changes (fallback).
    pub material_horizon_plies: u32,
}

/// Brilliant detection: a static sacrifice (SEE) by the engine's best move,
/// plus the recovered gates. Thresholds marked *fallback* are starting values
/// to calibrate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct BrilliantConfig {
    pub enabled: bool,
    /// Minimum material (cp) the opponent wins, by static exchange, by capturing
    /// the sacrificed piece.
    pub min_sacrifice_cp: i32,
    /// Minimum value (cp) of the piece given up (excludes pawns).
    pub min_piece_value_cp: i32,
    /// The second-best move must lose at least this much EP against the played
    /// move, or fall into a lower outcome bucket (fallback).
    pub min_gap_to_second_best: f64,
    /// No Brilliant for a move that forces mate itself (the reference excludes them).
    pub exclude_mating_moves: bool,
    /// The mover's EP before the move must be in `[min_ep_before, max_ep_before)`
    /// (fallback; neither greatly winning nor greatly losing).
    pub min_ep_before: f64,
    pub max_ep_before: f64,
    /// Also count a piece the move leaves en prise without moving it (for example
    /// the queen in Legal's trap), not only the moved piece (fallback extension).
    pub count_pieces_left_hanging: bool,
    /// In the endgame a sacrifice also needs this much material gain along the
    /// line, unless the second-best move drops an outcome bucket. Recovered: 200.
    pub endgame_min_gain_cp: i32,
    /// The mover must not be losing after the move (EP below this) (fallback).
    pub min_ep_after: f64,
}

/// Great Find detection. Gates are recovered; the "hard to find" test is a fallback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct GreatFindConfig {
    pub enabled: bool,
    /// Minimum EP gap between the best and second-best moves (fallback for the
    /// reference engine's difficulty/move-probability models).
    pub min_gap_to_second_best: f64,
    /// `TEP::is_high_rated`: `max(WhiteElo, BlackElo) >= 1000`. Recovered.
    pub require_high_rated: bool,
    pub high_rated_min: i32,
    /// "Easy to find" proxy for the reference's move-probability model: a search
    /// this deep already picks the move as best. 0 turns it off (the default:
    /// on the calibration games depths 4 to 8 found nearly every Great move).
    pub easy_search_depth: u32,
    /// A quiet retreat is "obvious" unless it wins at least this much material
    /// along the line (`is_quiet_obvious_retreat`; 200 recovered).
    pub quiet_retreat_min_stake_cp: i32,
}

/// Miss-subtype detection (fallback heuristics; guards recovered).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct MissConfig {
    /// Minimum value (cp) of a piece for "free piece".
    pub free_piece_min_value_cp: i32,
    /// Minimum material gain (cp) along the best line for "tactic".
    pub tactic_min_gain_cp: i32,
    /// Opponent's previous EP loss that counts as an opportunity to punish.
    pub opportunity_min_opponent_ep_lost: f64,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            precision_thresholds: [0.02, 0.05, 0.10, 0.20],
            outcome_thresholds: [0.10, 0.20, 0.35, 0.65, 0.80, 0.90],
            rating_min: 1000,
            rating_max: 2500,
            blunder_material_threshold_cp: 100,
            forgiving_material_lost_cp: 200,
            allow_tricky: false,
            brilliant: BrilliantConfig::default(),
            great_find: GreatFindConfig::default(),
            miss: MissConfig::default(),
            caps_exclude_completely_lost: true,
            material_horizon_plies: 6,
        }
    }
}

impl Default for BrilliantConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_sacrifice_cp: 100,
            min_piece_value_cp: 300,
            min_gap_to_second_best: 0.10,
            exclude_mating_moves: true,
            min_ep_before: 0.10,
            max_ep_before: 0.90,
            count_pieces_left_hanging: false,
            endgame_min_gain_cp: 200,
            min_ep_after: 0.20,
        }
    }
}

impl Default for GreatFindConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_gap_to_second_best: 0.20,
            require_high_rated: true,
            high_rated_min: 1000,
            easy_search_depth: 0,
            quiet_retreat_min_stake_cp: 200,
        }
    }
}

impl Default for MissConfig {
    fn default() -> Self {
        Self {
            free_piece_min_value_cp: 300,
            tactic_min_gain_cp: 200,
            opportunity_min_opponent_ep_lost: 0.10,
        }
    }
}
