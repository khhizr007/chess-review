# Coach log: agreement with Chess.com's coach (Phase 0)

This is Phase 0 of [`coach-analysis.md`](../coach-analysis.md) (§7 row 0, §8 item 2). It asks, per
ply, whether our coach picks the same explanation as Chess.com's coach: the same board-event tag in
the same circumstance. The label calibration in [`log.md`](log.md) asks the same question about
move labels.

**No Chess.com text is in this repository.** The captured speech and the decoded text asset stay in
WSL under `~/work/chesscom/oracle/` (see [`THIRD_PARTY.md`](../../THIRD_PARTY.md)). The comparison
reads tag and circumstance names only, which are generic chess terms. The descriptions below are
our own paraphrases.

## Runs

| date | measured commit | games | plies | engine (ours / theirs) | report |
|---|---|---:|---:|---|---|
| 2026-10-20 | `f5f0b35` (main, phase 1 coach) | 142 | 8,638 | Stockfish 18 depth 18 MultiPV 3 / Stockfish 18 (CEE 1.198.16) | [`runs/coach-phase0/coach-report.md`](../../tests/calibration/runs/coach-phase0/coach-report.md) |

| 2026-10-20 | `e4ead28` (main, phases 1–3) | 142 | 8,638 | same | [`runs/coach-main-e4ead28/coach-report.md`](../../tests/calibration/runs/coach-main-e4ead28/coach-report.md) |

The second run adds:

- good moves and the best-move sentence to the report;
- the Phase 2 tags to the mapping.

Its results and the remaining gaps are analysed in [`coach-gap-analysis.md`](../coach-gap-analysis.md).

Repeat with the same commands (below) and a new run name. The captures do not need to be taken
again.

## Method

### 1. Capture (local only)

- The review page receives the whole review from `/v2/game-review` as one `GameReview` message
  ([`chesscom-review-protocol.md`](chesscom-review-protocol.md)). Per ply it holds the played
  move's speech (sentences of fragments, each with its arrows and squares), the best alternative's
  speech, the show-moves buttons and the board markings.
- [`tools/calibrate/coach_capture.js`](../../tools/calibrate/coach_capture.js) runs on a signed-in
  analysis page. It opens each game's review page in a hidden same-origin iframe, wraps that frame's
  `WebSocket` before the app starts, and keeps the request and the response frames. Games are
  opened one at a time with a 3 s pause. Nothing is sent beyond what opening the review page sends.
- The account's Game Review engine must be **Stockfish**. This profile had Torch (the site default)
  and was switched with the owner's permission. The first two captures came back from Torch and
  were discarded. Every kept capture asks for engine 10, and all its scores come from engine 13
  (Stockfish 18).
- The browser pane blocks requests from chess.com to localhost. So `coachPull()` returns the frames
  as the tool result, which is saved to disk. `coach_oracle.py ingest` splits it into
  `captures/raw/<id>.json`.
- Coverage: **all 142 calibration games** (99 tuning + 43 holdout, the 59 of batches 1–3 included).
  2 games timed out on the first pass and were captured on a retry. As a sanity check, the decoded
  labels equal the earlier `cc-reviews/*.sf.txt` captures on 8,636 of 8,638 plies, and every move
  matches.

### 2. Sentences to keys

[`tools/calibrate/coach_oracle.py`](../../tools/calibrate/coach_oracle.py) runs `decode`, then
`match`.

**`decode`** reads the frames with a small protobuf wire decoder. The field numbers come from the
recovered schemas.

**`match`** maps each sentence to the key it came from in the embedded English asset
(`[Circumstance-[Child-]]Tag[Piece]-Style-N`):

- **Normalisation:** emphasis marks removed, quotes unified, case and final punctuation ignored, and
  the colour placeholders filled both ways. Where needed, piece names are generalised to "piece".
- **full:** the whole sentence equals an asset entry. When several keys share the text, the
  highest-priority tag wins and the record is marked ambiguous.
- **composed:** the sentence ends with an `InPlace<Tag><Piece>` verb phrase. The rest is a
  circumstance prefix, read with generic keyword rules:
  - miss / best / better / opportunity → Misses;
  - allow / permit / chance / able → Permits;
  - threat → Threatens;
  - force → Forces;
  - prevent / stop → Prevents.
- **pattern:** the live asset (CEE 1.198.16) is newer than the embedded one (app 4.10.15), so some
  verb phrases have no key. For these, a short list of generic chess words picks the tag, for example
  threaten plus capture → CaptureThreat, trapping → AttacksTrappedPiece and danger → EscapesThreat.

The output is `captures/<id>.keys.json`: keys and parsed tags only. Coverage on 142 games:

| speech | sentences | full key | composed | pattern | unmatched |
|---|---:|---:|---:|---:|---:|
| played move | 9,150 | 78.3% | 15.0% | 4.8% | 1.9% |
| best alternative | 3,718 | 95.5% | 1.8% | 1.6% | 1.1% |

The unmatched sentences are mostly encouragement, evaluation-swing remarks and opening comments for
positions the embedded asset does not have.

**Their main event** is the first sentence of the played move's speech that names a board event. The
following are skipped:

- classification remarks (keys `<Class>-Classification`);
- opening remarks (FEN-keyed);
- "only good move" framing (`Critical`), unless nothing else is said.

### 3. Our tags to theirs

[`tools/calibrate/coach_tags.py`](../../tools/calibrate/coach_tags.py) maps each of our `Tag`s to
the Chess.com tag names that say the same thing, and to a family:

- the nine families of §6.4 of `coach-analysis.md`;
- plus sacrifice, trade, evaluation, game and other, for Chess.com tags we have no counterpart for.

Chess.com tags that we do not have get a family by hand, or else from their priority range.

| our tag | family | Chess.com tags counted as the same |
|---|---|---|
| `back_rank_mate` | mate | BackRankMate |
| `checkmate` | mate | Checkmate, LadderMate, KingQueenMate, KingRookMate, DoubleCheckMate, StillMate, BackRankMate |
| `forced_mate` | mate | ForcesMate, MateThreat, MaintainedMate, NmpMateInXThreat, SlowerMate, KeymomentGoodFasterMate, KeymomentBadFasterMate |
| `free_piece` | free material | FreePiece, FreePieceFromDistance, QuasiFreePiece |
| `trapped_piece` | free material | AttacksTrappedPiece |
| `double_check` | double attack | DoubleCheck |
| `royal_fork` | double attack | RoyalFork, Fork, ForkWithMate |
| `fork` | double attack | Fork, ForkWithMate, PseudoFork, MultiAttack, RoyalFork |
| `discovered_check` | double attack | DiscoveredCheck, DiscoveredCheckAttack, DiscoveredAttackCheck, DiscoveredCheckCapture |
| `discovered_attack` | double attack | DiscoveredAttack, DiscoveredAttackCapture, DiscoveredAttackCheck, DiscoveredCheckAttack |
| `skewer` | line tactic | Skewer |
| `exploits_pin` | line tactic | ExploitsPinnedDefender, ThreatensPinned, PressuresPinnedPiece |
| `pin` | line tactic | Pin, RelativePin, PinsDefender, PinsAttacker |
| `removes_defender` | defender tactic | RemovesDefender, CapturesMateDefender, RemovesMateDefender, KicksDefender, KicksMateDefender, ChecksAwayDefender |
| `overloaded` | defender tactic | ExploitsOverloadedDefender |
| `attraction` | defender tactic | Attraction, Decoy |
| `promotion` | promotion | Promotion, Underpromotion |
| `moves_pinned_piece` | bad-move base | MovesPinnedPiece |
| `did_not_recapture` | bad-move base | DidNotRecapture |
| `ignores_threat` | bad-move base | IgnoresAttackedPiece, IgnoresAttackedPawn |
| `removes_own_defender` | bad-move base | RemovesOwnDefender |
| `bad_trade` | bad-move base | InferiorTrade, BadTrade, BadCaptureChoice |
| `hangs_piece` | bad-move base | HangsPiece |
| `hangs_pawn` | bad-move base | HangsPawn |
| `material_gain` | free material | WinsMaterial, WinsMaterialNotFree, LosesMaterial, PickUpPieceInTactic, Loses/MissesMaterial{Losing,Winning}Position |

**Circumstances.** Ours map one to one: BASE is the empty chain, and MISSES_SAME counts as MISSES.
Two normalisations make chains comparable:

- Chess.com's `LosesMaterial` is said about the move itself, but it means "allows winning material
  through no named tactic", so it counts as PERMITS.
- A step the tag already implies is dropped on both sides: "wins material" for free and trapped
  pieces; "wins material" and "leads to" for the bare material tags.

### 4. Our review and the comparison

- **Our review:** `compare.py` on all 142 games, run `coach-phase0` (Stockfish 18, depth 18,
  MultiPV 3, default rules).
- **Label agreement:** 68.7% exact and 88.7% within one step, in line with `log.md`.
- **The comparison:**
  [`tools/calibrate/coach_compare.py`](../../tools/calibrate/coach_compare.py) aligns plies by index,
  checks every UCI move and puts each ply into one category:
  - exact (tag and chain);
  - same tag, other circumstance;
  - same family;
  - different;
  - they explain and we don't, we explain and they don't, or neither.

```
# WSL; ~/work/chesscom/oracle holds coach_en_us.raw, tag_names.txt and captures/
python3 tools/calibrate/compare.py --url http://127.0.0.1:8095 --run-name <run> --games <all ids>
python3 tools/calibrate/coach_oracle.py decode
python3 tools/calibrate/coach_oracle.py match --report
python3 tools/calibrate/coach_compare.py --run tests/calibration/runs/<run> \
    --out tests/calibration/runs/<run>/coach-report.md
```

## Results (`f5f0b35`, 142 games)

Phase 1 explains only bad moves, so the headline is the 1,988 plies that Chess.com labels
Inaccuracy, Mistake, Blunder or Miss.

| category | all 1,988 bad plies | 1,641 plies both apps label bad |
|---|---:|---:|
| exact | 376 (18.9%) | 376 (22.9%) |
| same tag, other circumstance | 19 (1.0%) | 19 (1.2%) |
| same family | 76 (3.8%) | 76 (4.6%) |
| different | 687 (34.6%) | 687 (41.9%) |
| they explain, we don't | 644 (32.4%) | 351 (21.4%) |
| we explain, they don't | 81 (4.1%) | 81 (4.9%) |
| neither | 105 (5.3%) | 51 (3.1%) |

Where both name an event (1,158 plies):

- **32.5%** are exact;
- **34.1%** have the same tag;
- **40.7%** are in the same family or better.

Of the 347 bad plies we do not label bad, 245 are our Good and 65 our Excellent. The label
calibration covers those.

**By Chess.com classification (all 1,988 bad plies).** Bigger mistakes agree better, because they
are explained by material and Phase 1 is about material:

| class | plies | exact | same tag, other circ. | same family | different | they only | we only | neither |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Inaccuracy | 825 | 7.0% | 0.5% | 2.1% | 24.5% | 49.9% | 5.0% | 11.0% |
| Mistake | 616 | 18.5% | 0.2% | 4.7% | 41.6% | 28.4% | 4.5% | 2.1% |
| Blunder | 228 | 46.5% | 1.8% | 6.1% | 41.7% | 2.6% | 1.3% | 0.0% |
| Miss | 319 | 30.7% | 3.1% | 5.0% | 42.0% | 16.0% | 2.8% | 0.3% |

**By phase.** Agreement is lowest in the opening (14.6% exact):

- Chess.com explains 43% of the opening's bad moves with something we say nothing about;
- its most common family there is positional (development and structure, 164 of 602 plies), which
  we do not have yet.

| phase | plies | exact | same tag, other circ. | same family | different | they only | we only | neither |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| opening | 602 | 14.6% | 0.2% | 2.7% | 29.7% | 43.2% | 3.7% | 6.0% |
| middlegame | 921 | 21.7% | 1.0% | 5.1% | 36.9% | 27.5% | 3.9% | 3.9% |
| endgame | 465 | 18.9% | 1.9% | 2.8% | 36.1% | 28.2% | 4.9% | 7.1% |

**Good moves.** Chess.com explains:

- 89% of its Best moves;
- 82% of its Excellent moves;
- 79% of its Good moves;
- every Great, Forced and (bar one) Brilliant move.

We explain none of them: Phase 2 adds good moves.

**What Chess.com says about bad moves.** Its main events, by family:

| family | share |
|---|---:|
| bad-move base | 18.8% |
| positional | 17.3% |
| defence | 17.0% |
| free material | 11.0% |
| nothing matched | 9.4% |
| double attack | 6.8% |
| mate | 5.9% |
| trade | 3.4% |
| line tactic | 3.2% |
| other | rest |

Its most frequent main events are:

- PERMITS LosesMaterial (235);
- MISSES CaptureThreat (206);
- PERMITS FreePiece (97);
- MISSES FreePiece (89);
- PERMITS WinsMaterial Fork (50);
- PERMITS ForcesMate (49);
- HangsPiece (48).

On the plies both apps label bad, **44% (719) of Chess.com's main events are about what the best
move would have done (MISSES)**. Most of them are not about material:

| family of those MISSES events | plies |
|---|---:|
| defence | 247 |
| positional | 155 |
| free material | 89 |

### Top 5 actionable disagreements

These are counted on the 1,641 plies both apps label bad. The full pattern table, with ply
references, is in the report.

1. **A piece left en prise is "allowed free piece", not "ignored the threat".**
   - *Counts:* 56 plies where Chess.com says PERMITS FreePiece and we say `ignores_threat` (we
     detected their event on 55). Another 22 where it says PERMITS LosesMaterial (the piece is lost
     to an exchange) and we say `ignores_threat`.
   - *Also:* 13 more where a capture gives up the capturing piece. It says PERMITS FreePiece and we
     say `bad_trade`.
   - *Ratio:* across all 1,988 bad plies, Chess.com leads with IgnoresAttackedPiece/Pawn 18 times;
     we lead with `ignores_threat` 142 times.
   - *Fix:* in `priority.rs`, drop `ignores_threat` and `bad_trade` below the PERMITS free-piece
     event, or keep them only when the piece is lost to an exchange and then word it as losing
     material. Examples: 1041474766 12...Nxd5, 142873206408 12...O-O-O, 142849919300 7...Nxd5.
   - *Potential:* about +4 points of exact agreement.
2. **Missed defensive and threat ideas are not explained.**
   - *Counts:* Chess.com explains a bad move through the best move's threat to capture 155 times
     (MISSES CaptureThreat). Other frequent ideas are defending a piece, stepping out of an attack,
     answering a check, retreating or recapturing (MISSES DefendsPiece / EscapesThreat / EvadesCheck
     / ObviousRetreat / ObviousRecapture, about 100 more across all bad plies). On the first kind
     alone, we say nothing on 71 plies and name some material gain instead on 17.
   - *Why:* our MISSES only looks for material or mate on B's line.
   - *Fix:* Phase 2, with static threat and defence detectors on the best move. Examples:
     1011026868 9...b5, 142873206408 6.a3, 145833178628 3...Bc5.
3. **We name a tactic where Chess.com just says "loses material".**
   - *Counts:* PERMITS LosesMaterial is Chess.com's most frequent bad-move event (195 plies both
     label bad). On 81 of them we name a tactic in the reply line instead: fork 25,
     `removes_defender` 15, `discovered_attack` 9, `forced_mate` 7, `pin` 7, `skewer` 6,
     `exploits_pin` 6.
   - *Ratio:* across all bad plies, our main event is `removes_defender`, `pin` or `exploits_pin`
     183 times, while Chess.com's line and defender tactics together come to 86.
   - *Fix:* name a tactic only when it is the reply itself or a short forcing line. Otherwise fall
     back to `material_gain`, worded as losing material. Tighten Fork (targets each worth more than
     the forker, or undefended). Examples: 147786569400 19...Nf6, 168426135116 15.Bxf6,
     184110410320 15.Kb1.
4. **Hanging the moved piece loses to the PERMITS free-piece event.**
   - *Counts:* 8 plies where Chess.com says HangsPiece and we detect `hangs_piece`, but we say
     PERMITS FreePiece.
   - *Likely cause:* the "base outweighed" filter in `select_bad` compares the reply line's whole
     gain (which includes later material) with the base event's stake.
   - *Fix:* keep the base event when the PERMITS event wins that same piece. Examples:
     142849698970 18...Qg5+, 165327199736 19.Qc2.
5. **A mate further away is "forces mate", not a named pattern.**
   - *Counts:* 12 plies where Chess.com says PERMITS ForcesMate and we name `back_rank_mate`. A
     further 7 where we announce `forced_mate` and Chess.com says LosesMaterial, apparently for long
     mates.
   - *Fix:* name the mate pattern only for the mating move itself. Measure whether a mate-distance
     cap applies before mates are announced. Examples: 166096155098 13...Bh3, 167316688806
     22...Rfxb8.

**Next:**

- Opening and structure explanations are Phase 4 work:
  - moving the same piece twice (19);
  - central development (24);
  - castling (18);
  - early queen moves;
  - pawn moves in front of the king (15);
  - evaluation-swing remarks (18).
- 81 plies are explained only by us. On those, Chess.com's speech is generic or unmatched.

## Files

| path | committed | content |
|---|---|---|
| `tools/calibrate/coach_capture.js` | yes | in-page capture |
| `tools/calibrate/coach_oracle.py` | yes | ingest, decode, match (reads local paths) |
| `tools/calibrate/coach_tags.py` | yes | the mapping table and families |
| `tools/calibrate/coach_compare.py` | yes | the comparison and report |
| `tests/calibration/runs/coach-phase0/coach-report.md` | yes | the full report (tags and ply references only) |
| `~/work/chesscom/oracle/captures/raw/<id>.json` | **never** | captured frames (Chess.com text) |
| `~/work/chesscom/oracle/captures/<id>.coach.json` | **never** | decoded speech (Chess.com text) |
| `~/work/chesscom/oracle/captures/<id>.keys.json` | no | keys and tags per ply |

`.gitignore` excludes `captures/`, `*.coach.json`, `*.keys.json` and the oracle files, in case any
of them is copied into the checkout.
