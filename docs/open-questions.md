# Open Questions

These are items the reverse-engineering evidence did not settle. Each one has a fallback in this project.
All fallbacks are configurable in `ReviewConfig` (backend `review-core`) unless noted otherwise.

| # | Question | Evidence so far | Fallback used | Impact |
|---|---|---|---|---|
| 1 | Engine and depth used by the cloud review for each strength preset (Fast/Standard/Deep/Maximum) | Client sends `GameReviewSearchSettings(Strength)` and SF 16.1 NNUE or Torch. Local feedback uses depth 18. | Stockfish (any version) with presets 12/18/22/26 plies or a time budget | Scores near thresholds can differ |
| 2 | Exact `is_forced` definition | Name only | One legal move | Rare differences |
| 3 | Opening-book contents and the "book" rule | Embedded proprietary book (`init_embedded_book`) | Public-domain Lichess `chess-openings` positions: a move is Book while the resulting position is in that set and every earlier move was Book | Book length can differ |
| 4 | Sacrifice definition behind Brilliant (`node_lib::is_sacrifice`, base tags 24/25/209) | Gates recovered: material vs piece value, endgame gain ≥ 200, opponent EP ≥ 0.1 | Material dip of at least the piece value along the PV within 6 plies, and the move graded Best or Excellent | Brilliant frequency can differ |
| 5 | Great Find's difficulty model and human move-probability model | Calls to `PositionDifficultyRegistry` and `move_probability` (ML models) | MultiPV gap ≥ 0.20 EP between the best and second-best moves | Great Find frequency can differ |
| 6 | Tricky classification | Requires the ML models above. The Android client drops it. | Disabled | None in the UI |
| 7 | Detectors for the six Miss subtypes (free piece, tactic, win, comeback, brilliant, opportunity) | Decision order and A/B guards confirmed. The detectors inspect board-event tags on the best line. | PV-material and expected-outcome heuristics (see `docs/implementation-notes.md`) | The subtype label can differ. "Miss" itself is still gated by the confirmed guards. |
| 8 | `material_lost` / `loses_more_material_than_best` / `material_gain_in_sequence` exact horizon | Names and their use in the rules | Material balance along the PV, within a configurable horizon of 6 plies | Small |
| 9 | Game-phase heuristic weights | Structure: piece counts, piece-square material, castling | Piece-count heuristic, monotonic | Phase accuracy splits can differ |
| 10 | Key-moment final rule (`analysis::is_key_moment`) | Candidate filter recovered | Candidate + (material win ≥ 200 / forces mate / promotion) | Key-moment set can differ |
| 11 | "Effective rating" / game rating formula | Server field only | Not shown. Not invented. | The feature is absent |
| 12 | Coach explanations (about 250 tactical tags, speech templates) | Tag names and the prioritisation function names | Factual text generated from engine output, labelled as generated | Narrative depth |
| 13 | CAPS2 clamp for played eval `10 ≤ p < 30` raising scores to 80, but to 30 for `p ≥ 30` | Decompiled as written. It looks odd but it is what the code says. | Implemented exactly as decompiled, behind config | Only affects positions already ±10 pawns |
| 14 | Whether the server applies the "completely winning" exclusion flag when computing overall accuracy | The flag is an argument whose caller value is unknown | Exclusion enabled | Accuracy in lopsided games |
| 15 | Behaviour parity against the original app | The original cannot be run on this machine (Android arm64, server-side review) | Regression tests pin every recovered formula. Differences caused by the engine are documented. | A live comparison has not been done |
