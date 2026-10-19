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
- **Threat search (coach only).** A null-move search (`coach.threat_search_depth`, default depth 10,
  MultiPV 1) runs on positions with the side to move flipped and the en passant square cleared
  (`board::null_move`). It is skipped when the side to move is in check or the flipped position has no
  legal move. One search per position serves two moves: the position after move *i* with its mover to
  move again is N for move *i* (THREATENS), and the same position is N_prev for move *i + 1*
  (PREVENTS). Only positions next to a move that is neither Book nor Forced are searched. The
  searches share the engine pool and the progress count with the main searches, and never change a
  classification. A failed threat search is logged and skipped, not fatal. Depth 0 turns them off.

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

## Coach

`review-server/src/coach/` implements phases 1 and 2 of [`coach-analysis.md`](coach-analysis.md)
§6–7. It explains Inaccuracies, Mistakes, Blunders and Misses by naming a tactic and showing the line
(phase 1), and Brilliant, Great Find, Best, Excellent and Good moves by what they do, threaten or stop
(phase 2). It is pure: it reads the engine lines the review already has, including the null-move
threat searches (see "Search model"), and makes no engine calls. The output is `PlyReview.coach`;
`explanation` stays for the details drawer. Book and Forced moves get an empty note.

- **Nodes.** P is the played move, with its own line (the MultiPV line that starts with it, or P
  followed by the next position's line 1). R is the opponent's best reply: line 1 of the next position
  and its PV. B is the best alternative: line 1 of this position. B is skipped when it is P. N is the
  null-move threat search after P (the mover to move again), and N_prev the one before P (the opponent
  to move). Their lines start from the flipped positions, so their show-lines carry that FEN.
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
- **Defence tags** (`detect/defence.rs`, group 8, after the bad-move tags): `DefendsPiece` (a
  threatened piece stays and is safe because the moved piece guards it or blocks the attacker),
  `EscapesThreat` (the threatened moved piece is safe on its new square; captures are left to the
  capture tags, so a recapture is not an escape), `BlocksCheck` (a piece other than the king steps
  between a single line checker and the king) and `CounterThreat` (a threatened piece is left, and the
  moved piece attacks a new target it can win for at least as much). "Threatened" is SEE before the
  move; when N_prev is known, its line must also take that piece in its first two moves.
- **Circumstances** (`circ.rs`):
  - BASE: for a bad move, the bad-move tags on P. For a good move, the event of P's own line (as for
    PERMITS below: a mate, the tactic behind its gain, or a tactic of P itself) and the defence tags.
  - PERMITS / MISSES: R's or B's line. If its first mover mates, the event is a mate tag:
    `BackRankMate` when the mating move fits the pattern, else `Checkmate` (mate in one) or
    `ForcedMate`. If the line wins at least a pawn, the highest-priority tactic among that side's moves
    up to the gain ply is chosen: WINS_MATERIAL when it is the first move, LEADS_TO when it comes later.
    If no tactic is found, the fallback is `MaterialGain`. A line that wins nothing reports a tactic of
    its first move, if any. The gain is placed on the winning side's last capture before the material
    settles, never on the other side's move after it (10...cxb5 Bxb5+ "cxb5 wins a pawn").
  - THREATENS: N's line, read the same way, kept only when it is a threat: its first move mates or
    carries out the tactic, and the material falls by the mover's next move (an idea that needs a
    quiet move first is a plan, not a threat). It is also dropped
    when R's first move takes the moved piece (a trade, or a piece given up: 4...Bxf3 "threatens"
    nothing that 5.Qxf3 does not answer), and when the material threat is on a piece the mover could
    already win before P.
  - PREVENTS: N_prev's line, read the same way, when it is a threat (as above) and is gone after P: R
    no longer mates, or R wins less than the threat and does not take on the threatened square.
  - Gains are counted from **before** the move that led to the line, when that move captured. So a
    recapture is not a gain: after 4...Bxf3, 5.Qxf3 wins nothing. Null-move lines have no such move.
  - MISSES_SAME: B takes, for free, the piece on the square the opponent's last move went to.
- **Prioritiser** (`priority.rs`, §6.5):
  - The Permits block: mate; a bad BASE tag (unless R's tactic wins 200 cp more than it puts at
    stake); a named tactic with a gain; a free piece; `MaterialGain`; any PERMITS event.
  - The Misses block: mate; MISSES_SAME; a named tactic with a gain; a free piece; `MaterialGain`; any
    MISSES event.
  - A Miss runs the Misses block first, every other bad move the Permits block first. When the main
    event comes from the Permits side, the Misses block gives the best-move sentence. Without one, the
    sentence is a plain "Better was …". When neither block has an event, a THREATENS event is said,
    followed by "Better was …".
  - A non-Miss explained only by B is phrased as "Better was …", never "missed".
  - A good move: a mate on its own line; a tactic (or free piece) that wins material there;
    THREATENS a mate; PREVENTS a mate; a BASE tag without a gain (a tactic of P, or a defence tag);
    any THREATENS; any PREVENTS; `MaterialGain` on its own line. Good moves get no "better" sentence.
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
  - Good moves use the `good*`, `threatens*` and `prevents*` frames. Brilliant and Great Find moves use
    `brilliant_<frame>` / `great_find_<frame>` when the corpus has one; otherwise their own sentence
    (`brilliant`, `great_find`) comes first, and it is said alone when there is no event.
  - Show-lines replay the event's line through the gain (or the mate), plus two plies, with one
    narration per move: the move's best tag, or a capture, recapture, check, forced or quiet move.
    THREATENS and PREVENTS lines are labelled "Show the threat" and start from the null-move position.
- **Text.** Every sentence in `en.toml` was written for this project. None of it is the reference
  app's text.
- **Frontend.** `Coach.tsx` shows the sentences, with the named moves and pieces in bold. For a bad
  move they replace the generic advice; a good move keeps its one-line advice after them. The board
  draws the arrows of the main sentence's first marked fragment. Phase 3 makes it interactive (see
  "Coach UI (phase 3)").
- **Tests.** `tests/coach_detect.rs` has hand-built positions per tag, including the defence tags.
  `tests/coach_note.rs` checks circumstances (THREATENS and PREVENTS included), the bad-move and
  good-move priority, Brilliant and Great Find wording, styles and wording. `tests/coach_pipeline.rs`
  runs the fake and scripted engines, null-move searches included (scripted by FEN and depth).
  `cargo run --release --example coach_report -- game.pgn [depth] [threat depth]` (with
  `STOCKFISH_PATH`) prints the coach's notes for a real game and the review time.
- **Sample game** (Morphy, Paris 1858; Stockfish 18, depth 18, threat search depth 10). 26 of the 33
  moves are reviewed (the rest are Book). 21 get a sentence with geometry and a line (a selection
  below); 4...Bxf3, 9...b5, 11.Bxb5+, 12...Rd8 and 14...Qe6 get none.

  | Move | Class | Coach sentence |
  |---|---|---|
  | 5.Qxf3 | Best | Qxf3 stops Bxd1, which wins White's queen on d1. |
  | 6.Bc4 | Best | Bc4 threatens Qxf7#, which is checkmate. |
  | 6...Nf6 | Mistake | After Nf6, Qb3 forks Black's pawns on b7 and f7, winning a pawn. |
  | 7.Qb3 | Best | Qb3 forks Black's pawns on b7 and f7, winning a pawn. |
  | 7...Qe7 | Best | Qe7 defends against Bxf7+, which starts a forced mate in 2. |
  | 8.Nc3 | Good | Nc3 defends White's pawn on e4. |
  | 8...c6 | Best | c6 defends against Qxb7, which wins Black's pawn on b7. |
  | 9.Bg5 | Best | Bg5 pins Black's knight on f6 to Black's queen on e7, winning a pawn. |
  | 10.Nxb5 | Brilliant | Nxb5 is a sacrifice that works: it stops bxc4, which wins White's bishop on c4. |
  | 11...Nbd7 | Best | Nbd7 blocks the check. |
  | 13.Rxd7 | Best | After Rxd7 Nxd7, Bxe7 takes Black's queen on e7, which was guarding Black's rook on d8, winning material worth about 4 pawns. |
  | 13...Rxd7 | Mistake | After Rxd7, Rd1 makes use of the pin on Black's rook on d7. |
  | 15...Nxd7 | Mistake | After Nxd7, Qb8+ starts a back-rank mate in 2. |
  | 16.Qb8+ | Best | Qb8+ starts a back-rank mate in 2. |

  - Stockfish 18 rates the moves differently from 16.1 (used for the phase 1 table): 6.Bc4 is Best,
    9...b5 an Inaccuracy, 10...cxb5 and 14...Qe6 good moves. 6.Bc4's missing explanation in phase 1
    is now its threat.
  - 4...Bxf3 and 9...b5 are bad moves with nothing to name: their engine lines win no material within
    the horizon, and their threats (Bxd1, bxc4) are answered by taking the moved piece, so THREATENS
    is dropped. They need positional tags (phase 4).
  - 5.Qxf3 is really a recapture; it is said as the threat it stops, because there is no recapture
    tag yet.
- **Cost of the threat search** (same game; the machine was shared, so wall-clock times varied from
  30 to 56 s for one setting and do not resolve the difference). The review makes 33 main searches
  (depth 18, MultiPV 3) and 23 threat searches (depth 10, MultiPV 1). Measured alone (main search at
  depth 1, three runs each), the threat searches add about 0.9 s of engine CPU at depth 10 and 1.6 s
  at depth 12, against 100–180 s of CPU for the depth-18 review: about 1%. That is far below the 25–35%
  estimated in `coach-analysis.md` §6.2, because a depth-10 MultiPV-1 search is much cheaper than a
  depth-18 MultiPV-3 one.

## Coach UI (phase 3)

Frontend only (`coach-analysis.md` §6.8). It reads `PlyReview.coach` and keeps working when a move
has no note, or when the backend adds fields or tags.

- **Fragments.** In `Coach.tsx`, fragments with arrows or squares are buttons. Hover (mouse) or focus
  (keyboard) draws that fragment's arrows and square rings on the board, in place of the default
  main-sentence arrows. A click or tap pins it; a second tap unpins it.
- **Show-lines.** Each `show_line` is a button with its `label`. It opens a `LinePreview` with source
  `"coach"`. The board starts at `start_fen` (move -1) and plays itself. The bubble shows each move's
  `narration` and the line's moves as chips. Prev, Play/Pause, Next and ← → step through it; Hide or
  Esc returns to the game.
- **Avatar mood** (`coachMood`): impressed for Brilliant and Great, happy for Best, concerned for
  Mistake and Miss, sad for Blunder, neutral otherwise. In Retry it is happy when solved and
  concerned after a wrong try. It is drawn with CSS only (tile colour, tilt, a short hop or slump).
- **Retry** (`lib/retry.ts`):
  - The hint ladder comes from one event: a MISSES / MISSES_SAME event of the mover when there is
    one, else the main sentence's event.
  - Rung 1 rings the geometry's target squares (fan targets, the x-ray's front and back, the
    relation's end, and so on). Rung 2 names the motif: "Look for a free piece", "Make sure your
    move doesn't allow a fork", or the error tag's own hint for BASE events. The last step reveals
    the move.
  - Without an event, the only rung is the old one: the piece to move.
  - `judgeRetry` accepts the best move, or the first move of any engine line within 0.04 expected
    points of it. It uses the backend's curve at `rating_used`. Moves outside the engine lines
    cannot be judged and count as wrong. In a lost position most engine lines pass, as on lichess.
- **Takeaways** (`lib/takeaways.ts`, report view): events said by a move's sentences, counted once per
  move under (side, kind, tag family).
  - Kinds: PERMITS gives "Allowed", MISSES gives "Missed", an error tag in BASE gives its own verb
    ("Left a piece hanging"), and BASE on a good move gives "Found".
  - The four most frequent are shown, then the most costly by expected points lost. Each links to
    its moves.
  - Tag wording for hints and takeaways is in `lib/tags.ts`. Unknown tags fall back to "a tactic".
