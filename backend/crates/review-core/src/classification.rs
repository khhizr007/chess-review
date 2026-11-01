//! Move classification (classification v3). Functional spec §5.3–§5.7.
//!
//! The decision tree, precision thresholds and mate rules are recovered.
//! Board-event facts (brilliant, great find, material loss, missed
//! opportunities) are supplied by the caller through [`MoveFacts`], because
//! computing them needs a board and engine lines.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::config::ReviewConfig;
use crate::expected_points::{ExpectedOutcome, expected_outcome, node_expected_points};
use crate::value::Value;

/// Classification shown to users (the reference client's set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Classification {
    Brilliant,
    GreatFind,
    Best,
    Excellent,
    Good,
    Book,
    Inaccuracy,
    Mistake,
    Blunder,
    Miss,
    Forced,
    Tricky,
}

impl Classification {
    /// `api::should_attach_best_alternative_move`: show the engine's best move?
    pub fn shows_best_move(self) -> bool {
        !matches!(
            self,
            Classification::Book
                | Classification::Brilliant
                | Classification::GreatFind
                | Classification::Best
                | Classification::Forced
        )
    }

    /// The client's "good enough" set; anything else needs attention.
    pub fn is_good_enough(self) -> bool {
        matches!(
            self,
            Classification::Brilliant
                | Classification::GreatFind
                | Classification::Best
                | Classification::Book
                | Classification::Excellent
                | Classification::Good
                | Classification::Forced
        )
    }

    /// Whether the classification counts as a bad move.
    pub fn is_bad(self) -> bool {
        matches!(
            self,
            Classification::Inaccuracy
                | Classification::Mistake
                | Classification::Blunder
                | Classification::Miss
        )
    }
}

/// Subtype of a Miss (the reference engine's internal classes 10, 12–18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MissKind {
    Win,
    FasterMate,
    Mate,
    FreePiece,
    Tactic,
    Comeback,
    Brilliant,
    Opportunity,
}

/// Precision grade from expected points lost (`MovePrecisionV3Registry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Precision {
    Best,
    Excellent,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
}

/// Internal classification, numbered as in the reference engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Internal {
    Book = 0,
    Brilliant = 1,
    GreatFind = 2,
    Tricky = 3,
    Best = 4,
    Excellent = 5,
    Good = 6,
    Inaccuracy = 7,
    Mistake = 8,
    Blunder = 9,
    MissedWin = 10,
    Forced = 11,
    MissedFasterMate = 12,
    MissedMate = 13,
    MissedFreePiece = 14,
    MissedTactic = 15,
    MissedComeback = 16,
    MissedBrilliant = 17,
    MissedOpportunity = 18,
}

impl Internal {
    fn from_precision(p: Precision) -> Internal {
        match p {
            Precision::Best => Internal::Best,
            Precision::Excellent => Internal::Excellent,
            Precision::Good => Internal::Good,
            Precision::Inaccuracy => Internal::Inaccuracy,
            Precision::Mistake => Internal::Mistake,
            Precision::Blunder => Internal::Blunder,
        }
    }

    fn external(self) -> (Classification, Option<MissKind>) {
        use Classification as C;
        match self {
            Internal::Book => (C::Book, None),
            Internal::Brilliant => (C::Brilliant, None),
            Internal::GreatFind => (C::GreatFind, None),
            Internal::Tricky => (C::Tricky, None),
            Internal::Best => (C::Best, None),
            Internal::Excellent => (C::Excellent, None),
            Internal::Good => (C::Good, None),
            Internal::Inaccuracy => (C::Inaccuracy, None),
            Internal::Mistake => (C::Mistake, None),
            Internal::Blunder => (C::Blunder, None),
            Internal::Forced => (C::Forced, None),
            Internal::MissedWin => (C::Miss, Some(MissKind::Win)),
            Internal::MissedFasterMate => (C::Miss, Some(MissKind::FasterMate)),
            Internal::MissedMate => (C::Miss, Some(MissKind::Mate)),
            Internal::MissedFreePiece => (C::Miss, Some(MissKind::FreePiece)),
            Internal::MissedTactic => (C::Miss, Some(MissKind::Tactic)),
            Internal::MissedComeback => (C::Miss, Some(MissKind::Comeback)),
            Internal::MissedBrilliant => (C::Miss, Some(MissKind::Brilliant)),
            Internal::MissedOpportunity => (C::Miss, Some(MissKind::Opportunity)),
        }
    }
}

/// Board-event facts for the played move (computed by the caller).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoveEvents {
    pub brilliant: bool,
    pub tricky: bool,
    pub great_find: bool,
}

/// Material facts about the played move and the engine's best move.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MaterialFacts {
    /// `is_losing_material_or_getting_mated` for the played move.
    pub loses_material_or_mated: bool,
    /// `loses_more_material_than_best`.
    pub loses_more_material_than_best: bool,
    /// `material_lost` (cp) along the played line, if known.
    pub material_lost_cp: Option<i32>,
    /// Guard "A": the played move itself drops material (hangs a piece or loses
    /// more than the best line gains).
    pub drops_material: bool,
}

/// What the best line would have achieved (Miss detectors).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MissedChances {
    pub free_piece: bool,
    pub tactic: bool,
    pub win: bool,
    pub comeback: bool,
    pub brilliant: bool,
    pub opportunity: bool,
}

/// Everything needed to classify one move.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveFacts {
    /// Mover-perspective value after the played move.
    pub played: Value,
    /// Mover-perspective value after the engine's best move.
    pub best: Value,
    pub played_is_draw: bool,
    pub best_is_draw: bool,
    /// The played move is the engine's best move (forces equality).
    pub played_is_best_move: bool,
    pub rating: i32,
    pub is_book: bool,
    pub is_forced: bool,
    /// Both the move and its parent position have reliable engine scores.
    pub scored: bool,
    /// Other moves were analysed from the same position.
    pub has_siblings: bool,
    pub events: MoveEvents,
    pub material: MaterialFacts,
    /// Expected points the opponent lost on their previous move, if known.
    pub opponent_previous_ep_lost: Option<f64>,
    pub missed: MissedChances,
}

/// Classification result with the rule that produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassificationOutcome {
    pub classification: Option<Classification>,
    pub miss_kind: Option<MissKind>,
    pub precision: Option<Precision>,
    /// Expected points the move lost relative to the best move (≥ 0).
    pub ep_lost: f64,
    pub rule: Rule,
}

/// Rule that decided a classification, for traceability in the UI and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Rule {
    Book,
    Forced,
    Unscored,
    BrilliantEvent,
    TrickyEvent,
    GreatFindEvent,
    Precision,
    BlunderSoftened,
    LosingMaterialHardened,
    MissDetector,
    MateOverride,
}

/// Expected points lost by the played move relative to the best move.
pub fn expected_points_lost(f: &MoveFacts) -> f64 {
    let best = node_expected_points(f.best, f.best_is_draw, f.rating);
    let played = node_expected_points(f.played, f.played_is_draw, f.rating);
    (best - played).max(0.0)
}

/// Precision grade (§5.3), including the mate override. `None` when unscored.
pub fn precision(f: &MoveFacts, cfg: &ReviewConfig) -> Option<Precision> {
    if !f.scored {
        return None;
    }
    if f.played_is_best_move || f.played == f.best {
        return Some(Precision::Best);
    }
    let lost = expected_points_lost(f);
    let t = cfg.precision_thresholds;
    let mut grade = if lost <= t[0] {
        Precision::Excellent
    } else if lost <= t[1] {
        Precision::Good
    } else if lost <= t[2] {
        Precision::Inaccuracy
    } else if lost <= t[3] {
        Precision::Mistake
    } else {
        Precision::Blunder
    };
    if let Some(over) = mate_override(f.played, f.best, f.rating, cfg) {
        grade = match over {
            Internal::Good => Precision::Good,
            Internal::Inaccuracy => Precision::Inaccuracy,
            Internal::Mistake => Precision::Mistake,
            // Missed-mate overrides keep the threshold grade at the precision level.
            _ => grade,
        };
    }
    Some(grade)
}

/// Mate rules (§5.6). Values are mover-perspective after the move.
fn mate_override(played: Value, best: Value, rating: i32, cfg: &ReviewConfig) -> Option<Internal> {
    if best.is_mate() {
        let md_best = best.mate_moves().unwrap_or(0);
        let n_best = md_best.abs();
        if !played.is_mate() || (played.0 > 0) != (best.0 > 0) {
            let still_completely_winning =
                expected_outcome(played, rating, cfg) == ExpectedOutcome::CompletelyWinning;
            return Some(if n_best > 2 && still_completely_winning {
                Internal::Good
            } else {
                Internal::MissedMate
            });
        }
        let n_played = played.mate_moves().unwrap_or(0).abs();
        if played.0 < 1 {
            if (n_best > 2 && n_played == 1) || (n_best > 4 && n_played == 2) {
                return Some(Internal::Inaccuracy);
            }
        } else {
            if (md_best == 0 && n_played > 1) || (n_best == 1 && n_played > 3) {
                return Some(Internal::MissedFasterMate);
            }
            if (n_best == 1 && n_played > 2)
                || (n_best == 2 && n_played > 4)
                || (n_best == 3 && n_played > 6)
            {
                return Some(Internal::Good);
            }
        }
        return None;
    }
    if played.is_mate() && expected_outcome(best, rating, cfg) == ExpectedOutcome::Lost {
        let n_played = played.mate_moves().unwrap_or(0).abs();
        if n_played < 4 {
            return Some(Internal::Mistake);
        }
        return Some(if best.0 > -1501 {
            Internal::Inaccuracy
        } else {
            Internal::Good
        });
    }
    None
}

/// Classifies one move (§5.4).
pub fn classify(f: &MoveFacts, cfg: &ReviewConfig) -> ClassificationOutcome {
    let ep_lost = if f.scored {
        expected_points_lost(f)
    } else {
        0.0
    };
    let done = |internal: Internal, precision: Option<Precision>, rule: Rule| {
        let (classification, miss_kind) = internal.external();
        ClassificationOutcome {
            classification: Some(classification),
            miss_kind,
            precision,
            ep_lost,
            rule,
        }
    };

    if f.is_book {
        return done(Internal::Book, precision(f, cfg), Rule::Book);
    }
    if f.is_forced {
        return done(Internal::Forced, precision(f, cfg), Rule::Forced);
    }
    let Some(grade) = precision(f, cfg) else {
        if f.has_siblings {
            return ClassificationOutcome {
                classification: None,
                miss_kind: None,
                precision: None,
                ep_lost,
                rule: Rule::Unscored,
            };
        }
        return done(Internal::Best, None, Rule::Unscored);
    };

    if f.events.brilliant {
        return done(Internal::Brilliant, Some(grade), Rule::BrilliantEvent);
    }
    if cfg.allow_tricky && f.events.tricky {
        return done(Internal::Tricky, Some(grade), Rule::TrickyEvent);
    }
    if f.events.great_find {
        return done(Internal::GreatFind, Some(grade), Rule::GreatFindEvent);
    }

    let mut rule = Rule::Precision;
    let mut class = Internal::from_precision(grade);

    // Blunders that do not actually lose material (or allow mate) are softened.
    if class == Internal::Blunder && !f.material.loses_material_or_mated {
        class = Internal::Mistake;
        rule = Rule::BlunderSoftened;
    }

    // Small inaccuracies that throw material away while losing are hardened.
    if matches!(
        class,
        Internal::Excellent | Internal::Good | Internal::Inaccuracy
    ) {
        let losing = expected_outcome(f.played, f.rating, cfg) < ExpectedOutcome::Worse;
        let lost_enough = f
            .material
            .material_lost_cp
            .is_some_and(|cp| cp >= cfg.forgiving_material_lost_cp);
        if losing && f.material.loses_more_material_than_best && lost_enough {
            class = Internal::Mistake;
            rule = Rule::LosingMaterialHardened;
        }
    }

    // Missed opportunities, checked against the unadjusted precision grade. A move that walks
    // into a forced mate is never a Miss (calibration: 1 of the reference's 319 Misses allows
    // mate, 34 of its 228 Blunders do).
    let walks_into_mate = f.played.mate_moves().is_some_and(|n| n < 0);
    if grade >= Precision::Mistake && !walks_into_mate {
        let a = f.material.drops_material;
        let b = f
            .opponent_previous_ep_lost
            .is_some_and(|opp| ep_lost > 2.0 * opp);
        let m = &f.missed;
        let missed = if !a && m.free_piece {
            Some(Internal::MissedFreePiece)
        } else if !a && m.tactic {
            Some(Internal::MissedTactic)
        } else if !a && !b && m.win {
            Some(Internal::MissedWin)
        } else if !a && !b && m.comeback {
            Some(Internal::MissedComeback)
        } else if !b && m.brilliant {
            Some(Internal::MissedBrilliant)
        } else if !a && !b && m.opportunity {
            Some(Internal::MissedOpportunity)
        } else {
            None
        };
        if let Some(missed) = missed {
            class = missed;
            rule = Rule::MissDetector;
        }
    }

    if let Some(over) = mate_override(f.played, f.best, f.rating, cfg) {
        class = over;
        rule = Rule::MateOverride;
    }

    done(class, Some(grade), rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expected_points::expected_points_to_cp;
    use crate::value::{Eval, Side};

    const R: i32 = 1500;

    fn facts(played_cp: i32, best_cp: i32) -> MoveFacts {
        MoveFacts {
            played: Value(played_cp),
            best: Value(best_cp),
            played_is_draw: false,
            best_is_draw: false,
            played_is_best_move: false,
            rating: R,
            is_book: false,
            is_forced: false,
            scored: true,
            has_siblings: true,
            events: MoveEvents::default(),
            material: MaterialFacts {
                loses_material_or_mated: true,
                ..Default::default()
            },
            opponent_previous_ep_lost: None,
            missed: MissedChances::default(),
        }
    }

    /// Played value that loses exactly `lost` expected points from a best value of 0 cp.
    fn losing(lost: f64) -> i32 {
        expected_points_to_cp(0.5 - lost, R)
    }

    fn mate(winner: Side, moves: u32) -> Value {
        Value::after_move(Eval::Mate { winner, moves }, Side::White)
    }

    #[test]
    fn precision_thresholds_are_recovered_values() {
        let cfg = ReviewConfig::default();
        let grade = |lost: f64| precision(&facts(losing(lost), 0), &cfg).unwrap();
        assert_eq!(precision(&facts(30, 30), &cfg), Some(Precision::Best));
        assert_eq!(grade(0.015), Precision::Excellent);
        assert_eq!(grade(0.04), Precision::Good);
        assert_eq!(grade(0.09), Precision::Inaccuracy);
        assert_eq!(grade(0.19), Precision::Mistake);
        assert_eq!(grade(0.30), Precision::Blunder);
    }

    #[test]
    fn rating_changes_the_grade_of_the_same_centipawn_loss() {
        let cfg = ReviewConfig::default();
        let mut f = facts(0, 100);
        f.rating = 1000;
        assert_eq!(precision(&f, &cfg), Some(Precision::Inaccuracy)); // 0.056 lost
        f.rating = 2500;
        assert_eq!(precision(&f, &cfg), Some(Precision::Mistake)); // 0.154 lost
    }

    #[test]
    fn book_and_forced_take_priority() {
        let cfg = ReviewConfig::default();
        let mut f = facts(-900, 0);
        f.is_book = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Book)
        );
        f.is_book = false;
        f.is_forced = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Forced)
        );
    }

    #[test]
    fn blunder_without_material_loss_is_softened_to_mistake() {
        let cfg = ReviewConfig::default();
        let mut f = facts(losing(0.35), 0);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Blunder)
        );
        f.material.loses_material_or_mated = false;
        let out = classify(&f, &cfg);
        assert_eq!(out.classification, Some(Classification::Mistake));
        assert_eq!(out.rule, Rule::BlunderSoftened);
    }

    #[test]
    fn losing_side_throwing_material_is_hardened_to_mistake() {
        let cfg = ReviewConfig::default();
        // Already losing (EP < 0.2) and the move is only an Inaccuracy by EP.
        let best = expected_points_to_cp(0.15, R);
        let played = expected_points_to_cp(0.08, R);
        let mut f = facts(played, best);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Inaccuracy)
        );
        f.material.loses_more_material_than_best = true;
        f.material.material_lost_cp = Some(300);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Mistake)
        );
        f.material.material_lost_cp = Some(150);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Inaccuracy)
        );
    }

    #[test]
    fn miss_requires_mistake_precision_and_guards() {
        let cfg = ReviewConfig::default();
        let mut f = facts(losing(0.15), 0);
        f.missed.win = true;
        let out = classify(&f, &cfg);
        assert_eq!(out.classification, Some(Classification::Miss));
        assert_eq!(out.miss_kind, Some(MissKind::Win));

        // Guard A: the move itself dropped material.
        f.material.drops_material = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Mistake)
        );

        // Guard B: lost more than twice the opponent's previous loss (MissedWin needs ¬B).
        f.material.drops_material = false;
        f.opponent_previous_ep_lost = Some(0.05);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Mistake)
        );

        // MissedBrilliant only needs ¬B.
        f.opponent_previous_ep_lost = None;
        f.missed = MissedChances {
            brilliant: true,
            ..Default::default()
        };
        f.material.drops_material = true;
        assert_eq!(classify(&f, &cfg).miss_kind, Some(MissKind::Brilliant));

        // Inaccuracies never become misses.
        let mut g = facts(losing(0.08), 0);
        g.missed.win = true;
        assert_eq!(
            classify(&g, &cfg).classification,
            Some(Classification::Inaccuracy)
        );
    }

    #[test]
    fn miss_detectors_follow_recovered_order() {
        let cfg = ReviewConfig::default();
        let mut f = facts(losing(0.25), 0);
        f.missed = MissedChances {
            free_piece: true,
            tactic: true,
            win: true,
            ..Default::default()
        };
        assert_eq!(classify(&f, &cfg).miss_kind, Some(MissKind::FreePiece));
        f.missed.free_piece = false;
        assert_eq!(classify(&f, &cfg).miss_kind, Some(MissKind::Tactic));
    }

    #[test]
    fn events_take_priority_over_precision() {
        let cfg = ReviewConfig::default();
        let mut f = facts(10, 30);
        f.events.great_find = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::GreatFind)
        );
        f.events.brilliant = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Brilliant)
        );
        f.events = MoveEvents {
            tricky: true,
            ..Default::default()
        };
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Excellent)
        );
    }

    #[test]
    fn missing_a_mate_is_missed_mate_unless_still_crushing() {
        let cfg = ReviewConfig::default();
        let mut f = facts(250, 0);
        f.best = mate(Side::White, 1);
        let out = classify(&f, &cfg);
        assert_eq!(
            (out.classification, out.miss_kind),
            (Some(Classification::Miss), Some(MissKind::Mate))
        );

        // Mate in 4 missed, but the played move keeps a completely winning position.
        f.best = mate(Side::White, 4);
        f.played = Value(1500);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Good)
        );
    }

    #[test]
    fn slower_mates() {
        let cfg = ReviewConfig::default();
        let mut f = facts(0, 0);
        f.best = mate(Side::White, 0); // best move mates immediately
        f.played = mate(Side::White, 3);
        assert_eq!(classify(&f, &cfg).miss_kind, Some(MissKind::FasterMate));

        f.best = mate(Side::White, 1);
        f.played = mate(Side::White, 3);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Good)
        );
        f.played = mate(Side::White, 4);
        assert_eq!(classify(&f, &cfg).miss_kind, Some(MissKind::FasterMate));

        f.best = mate(Side::White, 3);
        f.played = mate(Side::White, 5);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Excellent)
        );
    }

    #[test]
    fn hastening_defeat_is_an_inaccuracy() {
        let cfg = ReviewConfig::default();
        let mut f = facts(0, 0);
        f.best = mate(Side::Black, 5);
        f.played = mate(Side::Black, 2);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Inaccuracy)
        );
    }

    #[test]
    fn walking_into_mate_when_already_lost_is_forgiven() {
        let cfg = ReviewConfig::default();
        let mut f = facts(0, -2000);
        f.played = mate(Side::Black, 2);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Mistake)
        );
        f.played = mate(Side::Black, 6);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Good)
        );
        f.best = Value(-1200);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Inaccuracy)
        );
    }

    #[test]
    fn walking_into_mate_from_a_playable_position_stays_a_blunder() {
        let cfg = ReviewConfig::default();
        let mut f = facts(0, 50);
        f.played = mate(Side::Black, 2);
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Blunder)
        );
        // Even when the best move would have won a piece: walking into mate is no Miss.
        f.missed.free_piece = true;
        assert_eq!(
            classify(&f, &cfg).classification,
            Some(Classification::Blunder)
        );
    }

    #[test]
    fn best_move_predicate_matches_reference() {
        use Classification as C;
        for c in [C::Book, C::Brilliant, C::GreatFind, C::Best, C::Forced] {
            assert!(!c.shows_best_move(), "{c:?}");
        }
        for c in [
            C::Excellent,
            C::Good,
            C::Inaccuracy,
            C::Mistake,
            C::Blunder,
            C::Miss,
            C::Tricky,
        ] {
            assert!(c.shows_best_move(), "{c:?}");
        }
    }
}
