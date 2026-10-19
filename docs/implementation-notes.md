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
| Generated explanations (facts for the details drawer) | `review-server/src/analysis/explain.rs` |
| Coach: position facts, tag detectors, circumstances, prioritiser, realiser, English text | `review-server/src/coach/` (`text/en.toml`) |
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
  - *Confirmed by calibration data:* scoring a played move outside the lines with a search of the
    same position restricted to it (`go … searchmoves`) matched Chess.com worse (26 labels worse,
    12 better on the tuning set). The restricted search rates the move higher than the next
    position's own search, which finds the refutation at full depth. `searchmoves` is supported by
    the engine layer but not used by the pipeline.
- **Determinism.** Every search starts with `ucinewgame` and `setoption name Clear Hash`, and engines
  run with one thread, so a search depends only on its request. Without this, hash tables carried over
  between searches in the engine pool changed about 14% of labels between two runs of the same games.
- **Repetition.** Threefold repetition is tracked along the game (EPD counts). A position drawn by
  rule has EP 0.5, matching the reference `Position::draw()`.

## Fallback heuristics

All horizons and thresholds are in `ReviewConfig`.

### Material along a line (`walk_line`)

- Plays the engine PV from the position before the move, for `material_horizon_plies` (6) plies.
- It then **extends while captures, promotions or checks continue**, up to 6 more plies, so material
  is measured at a quiet point. Without this, a horizon that cut an exchange in half graded near-best
  moves as Mistakes. See `docs/parity.md` for the Opera Game case.
- **Reported quantities:** start, end, min and max balance, what the first move captured, and the
  *gain ply*: the first settled ply (one not followed by a capture) from which the balance stays at
  its final level. The coach looks for the tactic behind a gain at or before it.

### Static exchange evaluation (`analysis/see.rs`)

- The standard swap list on one square: each side captures with its least valuable piece and may stop
  when going on loses material. Attackers are recomputed as pieces leave the board, so x-ray
  attackers join in.
- Pieces pinned to their king capture only along the pin line, a king never captures a defended
  piece, and promotions during the exchange are ignored. The king is valued at 99999, as in the
  reference sacrifice gates.
- `capture_gain` also requires a legal first capture, so a piece the opponent cannot take (for
  example because they are in check) is not counted as threatened.

### Facts derived from the walks

| Reference fact | Fallback |
|---|---|
| `material_lost` | `max(0, start − end)` of the played line. It is reported as unknown when the played move's score is a forced mate, because the mate rules decide those positions and material is meaningless there. |
| `is_losing_material_or_getting_mated` | material lost ≥ `blunder_material_threshold_cp` (100), or the played score is a mate against the mover |
| `loses_more_material_than_best` | the played line ends at least `forgiving_material_lost_cp` (200) below the best line. The margin was added after parity runs: in lost positions every line sheds material, and tiny PV differences between runs flipped the verdict. |
| Guard A ("drops material") | the move is not the best move, it loses ≥ 100, and it loses more than the best line gains |
| Sacrifice (`is_sacrifice`) | static test, not a line walk: see Brilliant below |

The line-based material facts were kept after calibration. Replacing them with the material the
opponent can win immediately (static exchange) lowered agreement with Chess.com in two variants: it
softened Blunders that lose material through a later tactic, such as a fork. See
`docs/calibration/log.md`.

### Event detectors

- **Brilliant** follows Chess.com's published definition (help article "How are moves classified?":
  a Brilliant move is "a good piece sacrifice"; the mover must not be in a bad position after it,
  must not be completely winning even without it, and newer players get a more generous sacrifice
  test). It requires all of (`brilliant.*` in the config):
  - the played move is the engine's best move (line 1);
  - **no** gap to the second-best move is required (`min_gap_to_second_best`, 0 = off). Chess.com
    requires the opposite (a sacrifice that is the only good move is a Great move): 4 of its 7
    Brilliants in the 59-game dataset had a second-best move within 0.06 EP;
  - not completely winning anyway: the second-best line's EP is below `max_alternative_ep` (0.90,
    where Completely Winning starts). A forced mate on the second line does not count: Chess.com rated
    a mating bishop sacrifice Brilliant although other moves mated too;
  - the mover is not mated after the move and its EP after it is at least `min_ep_after` (0.50);
    mating moves are allowed (`exclude_mating_moves`, off);
  - a **static sacrifice** (`static_sacrifice`): the net material given up, what the opponent wins by
    exchange (SEE) on the piece's square minus what the move captured, is at least
    `min_sacrifice_cp` (100), so an even trade is not a sacrifice, and the exchange itself wins at
    least `min_exchange_gain_cp` (200), or the whole pawn when a pawn is given up. (The 200 drops
    rook "sacrifices" that lose only an exchange's worth; every Chess.com Brilliant in the dataset
    gives up at least a minor piece for a pawn.) The piece is
    - the moved knight, bishop, rook or queen. If it was already threatened before the move, it had a
      safe square, and no other own piece and no opponent piece of at least its value was threatened;
    - or a knight, bishop, rook or queen the move leaves newly en prise without moving it
      (`count_pieces_left_hanging`, on): the queen in Legal's trap (5.Nxe5), a bishop left behind by
      castling. A piece that was already en prise before the move does not count;
    - or, when both players are below `pawn_sacrifice_below_elo` (1000), the moved pawn
      (Chess.com: 15...d5 in game 185058622904, both players rated about 950);
    - never a promotion.
    The earlier "disguised trade" test (capture plus discovered attack) is gone: it rejected the
    Traxler sacrifice 8...Nxg3, whose discovered attack on c4 wins nothing;
  - the recovered gates: the mover's balance after the move is below the sacrificed piece's value,
    **or** the opponent's EP before the move is ≥ 0.10; in the endgame a gain of at least 200 along
    the line, unless the second-best move drops an outcome bucket.
  - Calibration (`docs/calibration/log.md`): the rule finds 7 of the 9 Chess.com Brilliants in 60
    games and gives 5 others. The misses: 21...gxf3 is not our engine's best move, and 20...Bxh2+
    fails the recovered material gate by equality. One identical position (the Traxler
    8...Nxg3 in games 185058622904 and 185438343298, same score) is Brilliant in one review and Best in
    the other, so part of Chess.com's decision is not visible in the review data.
- **Great Find** requires all of:
  - precision Best or Excellent, and the played move is line 1;
  - an EP gap to line 2 of at least `min_gap_to_second_best` (0.20): a proxy for the unrecovered
    difficulty and move-probability models. 0.12–0.14 matched the tuning games better but gave
    mostly wrong Great Finds on the holdout games, so the default stays;
  - the recovered gates: `max(Elo) ≥ 1000` (but see `docs/parity.md`), not simply taking a free
    piece, not a recapture;
  - the recovered exclusions: not easy to find (the moved piece's only safe square; a free capture
    by static exchange; a pawn reaching the seventh rank or promoting) and not an obvious quiet retreat
    (a backward, non-capturing, non-checking piece move that wins less than 200 along the line);
  - the mover was not already completely winning;
  - the same endgame gate as Brilliant.
  - An optional proxy for the move-probability model, `easy_search_depth`, excludes moves that a
    shallow search already picks. It is **off** (0): at depths 4, 6 and 8 it removed every Great
    Find on the tuning set.
- **Tricky** is not implemented (it needs ML models, and the reference client drops it).

### Miss subtypes

These are only consulted when precision is Mistake or Blunder, under the recovered A/B guards and order.

| Subtype | Fallback test |
|---|---|
| free piece | the best move's first capture is worth ≥ 300 and is not won back |
| tactic | the best line gains ≥ 200 |
| win | best outcome bucket ≥ Winning and played ≤ Equal |
| comeback | mover ≤ Worse before the move, best ≥ Equal, played ≤ Worse |
| brilliant | the best move is a sacrifice (the Brilliant test above) and EP(best) ≥ 0.20 (`miss.brilliant_min_best_ep`) |
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
- **Book.** A move is Book when the position after it appears in the Lichess openings data (any
  position along a named line), even after earlier non-book moves: Chess.com marks moves that
  transpose back into the book as Book (calibration: +0.5 points on tuning, +0.7 on holdout).
  `book_requires_unbroken` restores the stricter "every earlier move was Book" rule. Chess.com's
  book is also larger than this one: in several games it continues 3–8 plies longer.
- **Explanations.** Only computed facts are stated: mate distance, the best move, material along the
  engine line, and expected score. They are labelled "generated" in the UI.

## Coach (phase 1: bad moves)

`review-server/src/coach/` implements phase 1 of [`coach-analysis.md`](coach-analysis.md) §6–7. It
explains Inaccuracies, Mistakes, Blunders and Misses by naming a tactic and showing the line. It is
pure: it reads the engine lines the review already has and makes no engine calls (no null-move
search yet, so no THREATENS or PREVENTS). The output is `PlyReview.coach`; `explanation` stays for
the details drawer. Good moves get an empty note.

- **Nodes.** P is the played move. R is the opponent's best reply: line 1 of the next position and its
  PV. B is the best alternative: line 1 of this position. B is skipped when it is P.
- **Facts** (`facts.rs`): attackers, SEE (`analysis/see.rs`), hanging pieces, absolute and relative
  pins, x-rays, pinned guards, and trapped pieces. A piece is trapped when the opponent wins it where
  it stands and on every square it can reach, unless it captures something at least as valuable.
- **Tags** (`detect/`): our own `Tag` enum, whose declaration order is the priority order. Groups 1–7
  of §6.4 are written from chess semantics and the lichess tagger *description*
  (`coach-research-notes.md` §2.1), not from its code. Choices that differ from that description:
  - fork targets include pawns only when they are hanging;
  - a line piece also forks through a friendly piece in front of it (a battery) when the piece
    behind becomes newly lost (6...Nf6 7.Qb3 hits b7, and f7 through the c4 bishop);
  - `FreePiece` counts only on the first move of a line (later captures collect what a tactic won),
    never on a recapture (taking back on the square the opponent just captured on; what the exchange
    nets is reported as the line's gain, e.g. 13.Rxd7 Nxd7 "wins the exchange", not "wins White's
    rook"), and gives way to `ExploitsPin` when the only defender is pinned to its king;
  - the bad-move tags (`HangsPiece`, `IgnoresThreat`, ...) also need R's line to take the piece in
    its first two moves, so a poisoned "hanging" piece is not called hanging.
- **Circumstances** (`circ.rs`):
  - BASE: the bad-move tags on P.
  - PERMITS / MISSES: R's or B's line. If its first mover mates, the event is a mate tag:
    `BackRankMate` when the mating move fits the pattern, else `Checkmate` (mate in one) or
    `ForcedMate`. If the line wins at least a pawn, the highest-priority tactic among that side's moves
    up to the gain ply is chosen: WINS_MATERIAL when it is the first move, LEADS_TO when it comes later.
    If no tactic is found, the fallback is `MaterialGain`. A line that wins nothing reports a tactic of
    its first move, if any.
  - Gains are counted from **before** the move that led to the line, when that move captured. So a
    recapture is not a gain: after 4...Bxf3, 5.Qxf3 wins nothing.
  - MISSES_SAME: B takes, for free, the piece on the square the opponent's last move went to.
- **Prioritiser** (`priority.rs`, §6.5):
  - The Permits block: mate; a bad BASE tag (unless R's tactic wins 200 cp more than it puts at
    stake); a named tactic with a gain; a free piece; `MaterialGain`; any PERMITS event.
  - The Misses block: mate; MISSES_SAME; a named tactic with a gain; a free piece; `MaterialGain`; any
    MISSES event.
  - A Miss runs the Misses block first, every other bad move the Permits block first. When the main
    event comes from the Permits side, the Misses block gives the best-move sentence. Without one, the
    sentence is a plain "Better was …".
  - A non-Miss explained only by B is phrased as "Better was …", never "missed".
- **Realiser** (`realise.rs`, `text/en.toml`):
  - A sentence is a frame (by circumstance chain and style) with the tag's phrase spliced in.
  - Styles `you`, `opponent` and `neutral`. The pipeline uses `neutral` ("White", "Black's knight"),
    because the API does not know which side the user played.
  - Placeholders: `{move} {reply} {line} {best} {mover} {piece} {piece_role} {square} {targets}
    {gain} {n}`.
  - Fragments for moves and pieces carry the event's arrows and squares. Moves keep their SAN
    capitalisation.
  - The wording varies by ply (the seed).
  - Lead-in lines longer than four plies are not spelled out in the sentence.
  - Show-lines replay the event's line through the gain (or the mate), plus two plies, with one
    narration per move: the move's best tag, or a capture, recapture, check, forced or quiet move.
- **Text.** Every sentence in `en.toml` was written for this project. None of it is the reference
  app's text.
- **Frontend.** `Coach.tsx` shows the sentences in place of the generic advice, with the named moves
  and pieces in bold. The board draws the arrows of the main sentence's first marked fragment.
  Show-line buttons and fragment hover are phase 3.
- **Tests.** `tests/coach_detect.rs` has hand-built positions per tag. `tests/coach_note.rs` checks
  circumstances, priority, styles and wording. `tests/coach_pipeline.rs` runs the fake and scripted
  engines. `cargo run --release --example coach_report -- game.pgn [depth]` (with `STOCKFISH_PATH`)
  prints the coach's notes for a real game.
- **Sample game** (Morphy, Paris 1858; Stockfish 16.1, depth 18). Five of the seven non-book bad
  moves get an event with geometry and a line:

  | Move | Coach sentence |
  |---|---|
  | 6...Nf6 | After Nf6, Qb3 forks Black's pawns on b7 and f7, winning a pawn. |
  | 9...b5 | After b5, Nxb5 wins a pawn. |
  | 10...cxb5 | cxb5 allows a sequence that ends with Bxd7+, which uncovers an attack by White's queen on Black's rook on b8, winning two pawns. |
  | 14...Qe6 | After Qe6, Bxd7+ wins Black's rook on d7. |
  | 15...Nxd7 | After Nxd7, Qb8+ starts a back-rank mate in 2. |

  - 9...b5 gets only the `MaterialGain` fallback, not a named motif.
  - 4...Bxf3 and 6.Bc4 are positional inaccuracies. All three engine lines keep material level within
    the horizon, so there is no tactic to name, and the bubble falls back to the old text. They need
    phase 2: a null-move THREATENS (6.Bc4: "Qb3 attacks b7") and positional tags.
