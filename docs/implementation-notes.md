# Implementation notes

This file records how each part of the functional spec is realised in code. It focuses on the
**fallback heuristics**: the places where the reference behaviour was not recovered and something
else is used instead.

## Where things live

| Concern | Module |
|---|---|
| Recovered rules (pure, no I/O): expected points, outcome buckets, precision, classification tree, mate rules, CAPS2, tallies | `backend/crates/review-core/src/*` |
| Engine-facing score model (White-relative `Eval`, mover-relative reference `Value`) | `review-core/src/value.rs` |
| Facts computed from board + engine lines (material walks, sacrifices, free pieces, recaptures) | `review-server/src/analysis/detectors.rs` |
| Orchestration: searches, perspective conversion, facts → `MoveFacts`, CAPS2, summary | `review-server/src/analysis/mod.rs` |
| Generated explanations | `review-server/src/analysis/explain.rs` |
| Phase heuristic, material values, FEN helpers | `review-server/src/board.rs` |

Every recovered constant is a default in `ReviewConfig` and is pinned by a unit test in `review-core`.
`GET /api/config/defaults` returns them, and `POST /api/reviews` accepts an override for
experiments.

## Search model

- **One search per position.** Each position before a move gets one MultiPV search. The final position
  is searched as well, unless it is checkmate, stalemate or a drawn final position.
- **Best move.** The best move's resulting score is line 1 of the search *before* the move. A mate
  distance is reduced by one move when the mover is the one mating (`eval_after_first_move`).
- **Played move.** If the played move appears among the MultiPV lines, the score of that line is used.
  This keeps both scores from one search, as the reference compares sibling nodes. Otherwise the
  played move is scored from the next position's search. If the played move is line 1, its value
  equals the best value by construction, which gives **Best**.
- **Repetition.** Threefold repetition is tracked along the game (EPD counts). A position drawn by
  rule has EP 0.5, matching the reference `Position::draw()`.

## Fallback heuristics

All horizons and thresholds are in `ReviewConfig`.

### Material along a line (`walk_line`)

- Plays the engine PV from the position before the move, for `material_horizon_plies` (6) plies.
- It then **extends while captures, promotions or checks continue**, up to 6 more plies, so material
  is measured at a quiet point. Without this, a horizon that cut an exchange in half graded near-best
  moves as Mistakes. See `docs/parity.md` for the Opera Game case.
- **Reported quantities:** start, end, min and max balance; the most valuable mover piece captured by
  the opponent; and what the first move captured.

### Facts derived from the walks

| Reference fact | Fallback |
|---|---|
| `material_lost` | `max(0, start − end)` of the played line. It is reported as unknown when the played move's score is a forced mate, because the mate rules decide those positions and material is meaningless there. |
| `is_losing_material_or_getting_mated` | material lost ≥ `blunder_material_threshold_cp` (100), or the played score is a mate against the mover |
| `loses_more_material_than_best` | the played line ends at least `forgiving_material_lost_cp` (200) below the best line. The margin was added after parity runs: in lost positions every line sheds material, and tiny PV differences between runs flipped the verdict. |
| Guard A ("drops material") | the move is not the best move, it loses ≥ 100, and it loses more than the best line gains |
| Sacrifice (`is_sacrifice`) | a mover piece worth ≥ 300 is captured, and the balance dips ≥ 200 below the start |

### Event detectors

- **Brilliant** requires all of:
  - precision Best or Excellent;
  - a sacrifice on the played line;
  - mover EP after the move ≥ 0.20;
  - the recovered gate: the mover's balance after the move is below the sacrificed piece's value, **or**
    the opponent's EP before the move is ≥ 0.10;
  - the recovered endgame gate: a gain of at least 200 along the line.
- **Great Find** requires all of:
  - precision Best or Excellent, and the played move is line 1;
  - an EP gap to line 2 of at least 0.20 (a proxy for the unrecovered difficulty and
    move-probability models);
  - the recovered gates: `max(Elo) ≥ 1000`, not simply taking a free piece, not a recapture;
  - the mover was not already completely winning;
  - the same endgame gate as Brilliant.
- **Tricky** is not implemented (it needs ML models, and the reference client drops it).

### Miss subtypes

These are only consulted when precision is Mistake or Blunder, under the recovered A/B guards and order.

| Subtype | Fallback test |
|---|---|
| free piece | the best move's first capture is worth ≥ 300 and is not won back |
| tactic | the best line gains ≥ 200 |
| win | best outcome bucket ≥ Winning and played ≤ Equal |
| comeback | mover ≤ Worse before the move, best ≥ Equal, played ≤ Worse |
| brilliant | the best line is a sacrifice and EP(best) ≥ 0.20 |
| opportunity | the opponent's previous move lost ≥ 0.10 EP |

### Other fallbacks

- **Phases.**
  - Endgame when there are ≤ 6 non-pawn, non-king pieces.
  - Middlegame when there are ≤ 10 such pieces, either side has fewer than 4 pieces on its back rank,
    or from ply 40.
  - Phases never go backwards.
- **Key moments.**
  - The recovered candidate filter: precision Best or Excellent, not forced, not book, mover not
    completely winning, not a recapture.
  - Plus a trigger: the line wins ≥ 200 material, the move keeps or delivers mate, it is a promotion,
    or it takes a free piece.
  - Critical moments are Blunder, Mistake, Miss, Brilliant and Great Find.
- **Book.** A move is Book while every earlier move was Book and the position after it appears in the
  Lichess openings data (any position along a named line). The lookup is position-based, so
  transpositions count.
- **Explanations.** Only computed facts are stated: mate distance, the best move, material along the
  engine line, and expected score. They are labelled "generated" in the UI.
