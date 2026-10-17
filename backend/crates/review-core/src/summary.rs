//! Summary statistics derived from per-move classifications.
//! Functional spec §11.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::classification::Classification;

/// Display order used by the reference client's tally.
pub const TALLY_ORDER: [Classification; 10] = [
    Classification::Brilliant,
    Classification::GreatFind,
    Classification::Best,
    Classification::Excellent,
    Classification::Good,
    Classification::Book,
    Classification::Inaccuracy,
    Classification::Mistake,
    Classification::Miss,
    Classification::Blunder,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TallyEntry {
    pub classification: Classification,
    pub count: u32,
}

/// Counts classifications in display order. Forced moves are counted as Best,
/// as in the reference client; Tricky (if enabled) is counted as Best too.
pub fn tally<I: IntoIterator<Item = Classification>>(classes: I) -> Vec<TallyEntry> {
    let mut counts = [0u32; TALLY_ORDER.len()];
    for c in classes {
        let c = match c {
            Classification::Forced | Classification::Tricky => Classification::Best,
            other => other,
        };
        if let Some(i) = TALLY_ORDER.iter().position(|o| *o == c) {
            counts[i] += 1;
        }
    }
    TALLY_ORDER
        .iter()
        .zip(counts)
        .map(|(c, count)| TallyEntry {
            classification: *c,
            count,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_counts_as_best() {
        let t = tally([
            Classification::Forced,
            Classification::Best,
            Classification::Blunder,
        ]);
        let get = |c| t.iter().find(|e| e.classification == c).unwrap().count;
        assert_eq!(get(Classification::Best), 2);
        assert_eq!(get(Classification::Blunder), 1);
        assert_eq!(t.len(), TALLY_ORDER.len());
    }
}
