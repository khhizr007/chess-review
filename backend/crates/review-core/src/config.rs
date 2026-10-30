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
    /// Rating clamp applied to `max(WhiteElo, BlackElo)`. Calibrated: 1200..=3000
    /// (see `docs/calibration/log.md`).
    pub rating_min: i32,
    pub rating_max: i32,
    /// Added to `max(WhiteElo, BlackElo)` before the clamp. Calibrated: 200
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
    /// Book moves must form an unbroken run from the start. Off by default: a move is
    /// also marked Book when play transposes back into the book (calibration).
    pub book_requires_unbroken: bool,
    pub coach: CoachConfig,
}

/// Brilliant detection: a Best move, or a Good one when
/// the best move does not force mate, that sacrifices a piece and leaves the mover in a playable
/// position. Newer players (below `great_find.high_rated_min`) get the more generous sacrifice test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct BrilliantConfig {
    pub enabled: bool,
    /// What the opponent must win by capturing the piece (static exchange, cp) for it to count
    /// as given up.
    pub min_threat_cp: i32,
    /// A moved pawn can be the piece given up. The reference allows it, but a pawn can never be
    /// strongly threatened (its SEE is at most 100), so this only matters below min_threat_cp 200.
    pub pawns: bool,
    /// A threatened piece left where it stands counts (`IGNORES_THREAT`). Recovered: yes.
    pub ignored_threats: bool,
    /// A piece the move leaves newly en prise counts (`INDIRECT_SACRIFICE`): a piece left behind by
    /// castling, or one the moved piece stopped guarding. Recovered: yes.
    pub left_en_prise: bool,
    /// A move graded Good or better qualifies when the best move does not force mate; otherwise
    /// only the best move does. Recovered: yes.
    pub near_best: bool,
    /// "You should not be in a bad position after a Brilliant move": the mover's EP after the
    /// move must be at least this (`is_bad_position`). Recovered: 0.47.
    pub min_ep_after: f64,
    /// In the endgame a sacrifice also needs this much material gain along the line, unless the
    /// position is critical (the Great tag's test). Recovered: 200.
    pub endgame_min_gain_cp: i32,
    /// No Brilliant for a mover already ahead by the sacrificed piece's value when the opponent's
    /// EP before the move, at rating 2000, was below this. Recovered: 0.10.
    pub opponent_lost_ep: f64,
    /// A moved piece counts only when the move's own exchange loses material (`is_safe`), so a
    /// plain trade is no sacrifice at any rating.
    pub moved_net_loss: bool,
    /// Legacy (`reference_standing` off): a piece left standing does not count when the move
    /// threatens an opponent piece worth at least as much.
    pub counter_threat_excludes: bool,
    /// Legacy (`reference_standing` off): a piece left standing does not count when the move
    /// captured a piece worth at least as much.
    pub standing_trade_excludes: bool,
    /// "Had a safe square" uses the reference's safe-square rule: a move of the piece whose own
    /// exchange does not lose (an even trade is safe), over the squares it attacks. Recovered: on.
    pub reference_safe_squares: bool,
    /// Legacy (`reference_standing` off): an ignored threat counts only if the piece had a safe
    /// square. The reference has no such test.
    pub ignored_needs_safe_square: bool,
    /// Pieces left standing follow the reference's ignored-threat test (with its walk-back over
    /// earlier moves) and its indirect-sacrifice and left-standing sacrifice tests instead of the
    /// approximation above. Recovered: on.
    pub reference_standing: bool,
    /// The moved piece's tests follow the reference exactly: strong threats (SEE ≥ 200) in the
    /// mutual-threat and saved-pieces tests, and a discovered attack is a new strong threat by an
    /// uncovered slider. Recovered: on.
    pub reference_moved: bool,
    /// The left-standing sacrifice test also needs no pinned attacker.
    pub ref_pinned_attacker: bool,
    /// No overloaded attacker (`has_overloaded_attacker`).
    pub ref_overloaded: bool,
    /// The opponent keeps its safe promotion squares (`safe_promotion_squares`).
    pub ref_promotion: bool,
    /// Trapped means threatened with no safe square.
    pub ref_trapped: bool,
    /// High-rated moved piece: a non-pinned capturer and no deflection.
    pub ref_deflection: bool,
    /// Pins follow the reference's slider rules instead of the standard definition.
    pub ref_decoded_pins: bool,
    /// Trapped follows the reference's own rule instead of "threatened, no safe square".
    pub ref_decoded_trapped: bool,
    /// An ignored piece that is relatively pinned counts only if another Good-or-better move
    /// loses no material.
    pub ref_relative_pin_defensible: bool,
    /// In the left-standing sacrifice test, the threats the move created exclude only the
    /// SEE-threateners of the sacrificed square, not every geometric attacker.
    pub ref_created_see_threats: bool,
}

/// Great detection. A Great move is a Best or Excellent move that is
/// not easy to find and is a key moment after the opponent's error (it forces mate, wins material,
/// or, in low-rated games, takes a free piece), the only good move in a critical position, or a
/// move that makes the mover winning or brings them back to equal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct GreatFindConfig {
    pub enabled: bool,
    /// High-rated game: `max(WhiteElo, BlackElo)` at least this. Not a requirement: below it
    /// Great is more generous (free captures and recaptures are not "easy", a free piece taken
    /// after the opponent's error is a key moment, recaptures and mates are not excluded).
    /// Recovered: 1000.
    pub high_rated_min: i32,
    /// Key moments need the mover's EP after the move at least this. Recovered: 0.47.
    pub key_moment_min_ep: f64,
    /// Wins-material key moment: the line gains more than this (cp). Recovered: 100.
    pub wins_material_min_gain_cp: i32,
    /// Forces-mate key moment: mate in at most this many moves, from a position where the mover's
    /// score after their own previous move was at most `forces_mate_max_previous_cp`.
    /// Recovered: 4 and 1499.
    pub forces_mate_max_moves: u32,
    pub forces_mate_max_previous_cp: i32,
    /// Critical position (the only good move): the best move's EP is at least
    /// `critical_min_best_ep`, the second line's EP is below `critical_max_alternative_ep` and
    /// loses at least `critical_min_alternative_loss`. Recovered: 0.35, 0.80, 0.10.
    pub critical_min_best_ep: f64,
    pub critical_max_alternative_ep: f64,
    pub critical_min_alternative_loss: f64,
    /// "Easy to find" proxy for the reference's position-difficulty model: a search
    /// this deep already picks the move as best. 0 turns it off (the default:
    /// on the calibration games depths 4 to 8 found nearly every Great move).
    pub easy_search_depth: u32,
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
    /// A missed free piece or tactic is a Miss only after the opponent's previous move lost at
    /// least this. Calibrated: 0.05. 0 turns it off.
    pub tactic_min_opponent_ep_lost: f64,
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
    /// Leave out ideas above the player's rating (each coach tag has a
    /// minimum rating). The rating is the player's Elo from the PGN when
    /// `player_side` is known, else the rating used for expected points.
    pub rating_gates: bool,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            precision_thresholds: [0.02, 0.05, 0.10, 0.20],
            outcome_thresholds: [0.10, 0.20, 0.35, 0.65, 0.80, 0.90],
            rating_min: 1200,
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
            min_threat_cp: 200,
            pawns: false,
            ignored_threats: true,
            left_en_prise: true,
            near_best: true,
            min_ep_after: 0.47,
            endgame_min_gain_cp: 200,
            opponent_lost_ep: 0.10,
            moved_net_loss: true,
            counter_threat_excludes: true,
            standing_trade_excludes: true,
            reference_safe_squares: true,
            ignored_needs_safe_square: true,
            reference_standing: true,
            reference_moved: true,
            ref_pinned_attacker: true,
            ref_overloaded: true,
            ref_promotion: true,
            ref_trapped: true,
            ref_deflection: true,
            ref_decoded_pins: true,
            ref_decoded_trapped: false,
            ref_relative_pin_defensible: false,
            ref_created_see_threats: true,
        }
    }
}

impl Default for GreatFindConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            high_rated_min: 1000,
            key_moment_min_ep: 0.47,
            wins_material_min_gain_cp: 100,
            forces_mate_max_moves: 4,
            forces_mate_max_previous_cp: 1499,
            critical_min_best_ep: 0.35,
            critical_max_alternative_ep: 0.80,
            critical_min_alternative_loss: 0.10,
            easy_search_depth: 0,
        }
    }
}

impl Default for CoachConfig {
    fn default() -> Self {
        Self {
            threat_search_depth: 10,
            best_threat_search: true,
            rating_gates: true,
        }
    }
}

impl Default for MissConfig {
    fn default() -> Self {
        Self {
            free_piece_min_value_cp: 300,
            tactic_min_gain_cp: 200,
            opportunity_min_opponent_ep_lost: 0.10,
            tactic_min_opponent_ep_lost: 0.05,
            brilliant_min_best_ep: 0.20,
        }
    }
}
