//! Game-review rules, independent of any chess engine or board library.
//!
//! Everything here reproduces behaviour recovered from the reference
//! application (see `docs/functional-spec.md`). Inputs are plain numbers and
//! facts; computing those facts from a board and engine output is the job of
//! the server crate.

pub mod caps2;
pub mod classification;
pub mod config;
pub mod expected_points;
pub mod summary;
pub mod value;

pub use caps2::{Caps2Move, Caps2State, Phase, ScoredMove, SideAccuracy};
pub use classification::{
    Classification, ClassificationOutcome, MaterialFacts, MissKind, MissedChances, MoveEvents,
    MoveFacts, Precision, Rule, classify,
};
pub use config::ReviewConfig;
pub use expected_points::{
    ExpectedOutcome, cp_to_expected_points, expected_outcome, rating_for_expected_points,
};
pub use value::{Eval, Side, UciScore, Value};
