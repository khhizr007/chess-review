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
    /// Calibrated upper bound: 3000. On Chess.com's own scores the 2500 cap makes
    /// titled games too lenient (see `docs/calibration/log.md`, exact-score oracle).
    pub rating_min: i32,
    pub rating_max: i32,
    /// Added to `max(WhiteElo, BlackElo)` before the clamp. Calibrated: 200. The
    /// recovered rule is 0; Chess.com grades match a higher rating than the PGN's
    /// (see `docs/parity.md`).
    pub rating_offset: i32,
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
    /// Book moves must form an unbroken run from the start. Off by default: Chess.com
    /// also marks a move Book when play transposes back into the book (calibration).
    pub book_requires_unbroken: bool,
    pub coach: CoachConfig,
}

/// Brilliant detection after Chess.com's published definition ("a good piece
/// sacrifice": the best move, not in a bad position after it, not completely
/// winning without it), with a static sacrifice test (SEE) and the recovered
/// gates. Thresholds are calibrated on Chess.com reviews (`docs/calibration/log.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct BrilliantConfig {
    pub enabled: bool,
    /// Minimum net material (cp) given up: what the opponent wins by static
    /// exchange on the sacrificed piece's square, minus what the move captured.
    pub min_sacrifice_cp: i32,
    /// The opponent's exchange must also win at least this much (cp) in total, or
    /// the whole piece when it is worth less (a pawn). Calibrated: 200; every
    /// Chess.com Brilliant in the dataset gives up at least a minor piece for a pawn.
    pub min_exchange_gain_cp: i32,
    /// Minimum value (cp) of the piece given up (excludes pawns, but see
    /// `pawn_sacrifice_below_elo`).
    pub min_piece_value_cp: i32,
    /// Chess.com is "more generous in defining a piece sacrifice for newer
    /// players": below this `max(WhiteElo, BlackElo)` a pawn the move puts en
    /// prise also counts. 0 turns it off. Calibrated: 1000.
    pub pawn_sacrifice_below_elo: i32,
    /// The second-best move must lose at least this much EP against the played
    /// move, or fall into a lower outcome bucket. 0 turns the test off (the
    /// default): Chess.com requires the opposite, that a non-sacrificing move
    /// was available, and most of its Brilliants have a near-equal second best.
    pub min_gap_to_second_best: f64,
    /// No Brilliant for a move that forces mate itself. Off: Chess.com rates
    /// mating sacrifices Brilliant.
    pub exclude_mating_moves: bool,
    /// "You should not be completely winning even if you hadn't found the move":
    /// the second-best move's EP must be below this (Completely Winning starts
    /// at 0.90). A forced mate on the second-best line does not count.
    pub max_alternative_ep: f64,
    /// Also count a piece the move leaves newly en prise without moving it (the
    /// queen in Legal's trap, a bishop left behind by castling), not only the
    /// moved piece.
    pub count_pieces_left_hanging: bool,
    /// In the endgame a sacrifice also needs this much material gain along the
    /// line, unless the second-best move drops an outcome bucket. Recovered: 200.
    pub endgame_min_gain_cp: i32,
    /// "You should not be in a bad position after a Brilliant move": the mover's
    /// EP after the move must be at least this.
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
    /// "Missed brilliant": the best move is a sacrifice and its EP is at least this.
    pub brilliant_min_best_ep: f64,
}

/// Engine work done for the move coach only (it never changes a classification).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CoachConfig {
    /// Depth of the null-move threat search (MultiPV 1) on the positions before
    /// and after each move with the side to move flipped: what a move threatens,
    /// and which threat it stops. Book and forced moves are skipped. 0 turns it
    /// off.
    pub threat_search_depth: u32,
    /// Also run the threat search after the engine's best move, where the
    /// played move is not it: what the best move would have threatened.
    pub best_threat_search: bool,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            precision_thresholds: [0.02, 0.05, 0.10, 0.20],
            outcome_thresholds: [0.10, 0.20, 0.35, 0.65, 0.80, 0.90],
            rating_min: 1000,
            rating_max: 3000,
            rating_offset: 200,
            blunder_material_threshold_cp: 100,
            forgiving_material_lost_cp: 200,
            allow_tricky: false,
            brilliant: BrilliantConfig::default(),
            great_find: GreatFindConfig::default(),
            miss: MissConfig::default(),
            caps_exclude_completely_lost: true,
            material_horizon_plies: 6,
            book_requires_unbroken: false,
            coach: CoachConfig::default(),
        }
    }
}

impl Default for BrilliantConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_sacrifice_cp: 100,
            min_exchange_gain_cp: 200,
            min_piece_value_cp: 300,
            pawn_sacrifice_below_elo: 1000,
            min_gap_to_second_best: 0.0,
            exclude_mating_moves: false,
            max_alternative_ep: 0.90,
            count_pieces_left_hanging: true,
            endgame_min_gain_cp: 200,
            min_ep_after: 0.50,
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

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            threat_search_depth: 10,
            best_threat_search: true,
        }
    }
}

impl Default for MissConfig {
    fn default() -> Self {
        Self {
            free_piece_min_value_cp: 300,
            tactic_min_gain_cp: 200,
            opportunity_min_opponent_ep_lost: 0.10,
            brilliant_min_best_ep: 0.20,
        }
    }
}
