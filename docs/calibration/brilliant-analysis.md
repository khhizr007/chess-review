# Brilliant: why we differ from Chess.com, and the plan

Analysis of commit `62704b1` (run `br-v3`) against Chess.com on all 575 calibration games
(42,049 scored plies, 98 Chess.com Brilliants). Written 2026-10-24, before any further change.

## 1. Summary

| | Count |
|---|---|
| Chess.com Brilliants | 98 |
| Ours | 151 |
| Both (right) | 77 |
| Missed | 21 |
| Extra (wrong) | 74 |

Precision 0.51, recall 0.79. The differences have four sources:

| Source | Missed | Extra | Fixable? |
|---|---|---|---|
| Our "threatened" is weaker than Chess.com's "strongly threatened" | 0 | 14 | Yes, decoded (§4.1) |
| Our safe-square test is stricter than Chess.com's | 6–8 | ? | Yes, decoded (§4.2) |
| Pieces left standing: `is_sacrifice_of_not_moved` and the walk-back are only approximated | 4 | 46 | Partly, needs decoding (§4.4) |
| High-rated branch, material gate, saved pieces: approximations | 7 | ? | Needs decoding (§4.5–4.7) |
| Engine differences (our Stockfish 18 at depth 18 vs Chess.com's engine) | 3 | 7 | No (§5) |

The engine-noise floor is about 10 of 172 disagreeing or shared cases. Everything else is a rule
difference that the decompile can settle. No rule below was chosen by fitting thresholds to labels.

## 2. Evidence used

1. **Our trace.** A copy of the backend (never committed, `~/work/brtrace`) logs every Brilliant gate
   and every sacrifice candidate with the reason it was rejected. The traced run reproduces `br-v3`
   exactly (P 0.51, R 0.79).
2. **Chess.com's labels and scores** (`tests/calibration/cc-reviews/*.sf.txt`).
3. **Chess.com's coach tags** (local oracle, `~/work/chesscom/oracle/captures/*.keys.json`, never
   committed). Each Brilliant carries the base tag that made it one, with the piece, e.g.
   `IgnoresThreat/rook[Brilliant]`. This tells us which sacrifice Chess.com saw.
4. **The libcee 4.10.15 decompile** (Ghidra project `~/work/chesscom/ghidra`, scripts
   `~/work/re-great`). New this round: `is_sacrifice`, `is_safe`, `Piece::safe_squares`,
   `ignored_threats`, the `StaticAnalysis` layout and `collect_eval_terms`.

## 3. What the data shows

### 3.1 Which sacrifice Chess.com saw, and whether we found it

| Chess.com tag | Found | Missed |
|---|---|---|
| DirectSacrificeCapture (moved piece, captures) | 41 | 8 |
| DirectSacrificeEnprise (moved piece, no capture) | 15 | 3 |
| IgnoresThreat | 15 | 4 |
| IndirectSacrifice | 3 | 1 |
| …Mate variants | 1 | 2 |
| ForkIfCaptured | 1 | 0 |
| no tag decoded | 1 | 3 |

Our sacrifice kind agrees with Chess.com's tag on every found Brilliant except one.

### 3.2 Chess.com Brilliants are not always its best move

Chess.com hides the best move for a Brilliant. Using its previous-ply score instead: 80 of 98
Brilliants lose nothing, 12 lose up to 0.02 and 6 lose 0.02–0.05. Its "Best" moves always lose
nothing. So the decompile's "Good or better" rule (`near_best`) is real.

### 3.3 Why we missed 21 (from the trace)

| Reason | Count | Cases |
|---|---|---|
| Moved piece "had no safe square" | 6 | Qxc7, Rxg6+, Nd6+, Rxf6, Nxf6+, Rxd7 |
| Ignored threat rejected by our `counter_threat` exclusion | 2 | Rac1, Re8 |
| Ignored threat "could not be saved" (no safe square) | 2 | Bxc3, Bxf3 |
| High-rated "disguised trade": discovered attack counted | 2 | Nfg4+, Qxd4 |
| Material gate | 3 | Rxc6, Bxh2+, O-O-O |
| Moved piece: mutual threat / saves pieces | 2 | Bxf7+, Bxe3+ |
| Endgame gate | 1 | Bxe4 |
| Engine: we grade the move Inaccuracy | 2 | Nf3, gxf3 |
| Engine: our EP after is just under 0.47 | 1 | Rxd5 |

### 3.4 Why we gave 74 extras

Chess.com's label for the move: 32 Best, 15 Excellent, 10 Great, 9 Good, 6 Inaccuracy, 2 Book.
So in 66 of 74 cases Chess.com also judged the move good enough. It just did not see a sacrifice.

| Our sacrifice kind | Right | Wrong |
|---|---|---|
| Moved piece | 58 | 28 |
| Ignored threat | 15 | 39 |
| Left en prise | 4 | 7 |

Ignored threats are the weakest part. Many wrong ones repeat the same piece over several moves
(five in game 185339193636, two in 185743721678).

### 3.5 Label consistency

Only one position and move appears twice: the Traxler 8...Nxg3. Chess.com calls it Brilliant at
max Elo 949 and Best at 1034. That is no noise: 1000 is the `is_high_rated` boundary, and above it
the discovered attack on c4 makes the move a "disguised trade". The decompile predicts both labels.

## 4. What the decompile says

### 4.1 "Strongly threatened" means the exchange wins at least 200

`collect_eval_terms` tries every capture of every piece, scores it with `Position::see_gain`, and
writes three sets:

| SEE of the capture | Set | StaticAnalysis field |
|---|---|---|
| ≥ 0 | the attacker's safe squares | `safe_squares` (+0x4e0) |
| ≥ 100 | "SEE threatened" | `see_threats_against` (+0xb0) |
| ≥ 200 | "strongly SEE threatened" | `strong_see_threats_against` (+0x4d0) |

`is_sacrifice`, `ignored_threats` and `is_indirect_sacrifice` all use the strong set. Our
`min_threat_cp` is 100. With 200, 14 extras go and no right Brilliant is lost. This also explains
why pawns never count: capturing a pawn can never win 200.

### 4.2 A safe square is a move whose own exchange does not lose

`Piece::safe_squares` reads the same table: destinations where the move's own SEE is ≥ 0. An even
trade (rook takes a defended rook) is therefore safe. Ours asks whether the opponent can win material
after the move, ignoring what the move captured, so an even trade counts as unsafe. That is the
likely cause of the six "no safe square" misses, and of the two ignored-threat ones.

`is_safe(n)` (the move itself) is the same test on the played destination. Our `moved_net_loss`
(gain > captured) already approximates it, so it is now decompile-backed.

### 4.3 `is_sacrifice` (moved piece), confirmed

In order: not castling, not game over, `!is_safe(n)`, the moved piece is strongly threatened after
the move, and it had at least one safe square before. If it was strongly threatened before, no
other own strongly threatened piece and no strongly threatened opponent piece may be worth at least
as much. Then `saved_pieces` is computed (its use is still unclear). Then, for high-rated games
only: the piece has a non-pinned threatener, the move is no deflection, and captured value plus the
greatest discovered attack is below the piece's value. Low-rated games return true straight away.

### 4.4 Pieces left standing (`ignored_threats`, `is_indirect_sacrifice`)

- No sacrifice when in check, after a check evasion, at ply 1, with a PROMOTION event, or for a
  mate evasion.
- **Ignored threat:** the most valuable piece strongly threatened both before and after the move.
  There is no safe-square requirement here: our "could have been saved" test is ours, not
  Chess.com's.
- **Walk-back:** go back two plies at a time while the same piece (by piece id) stays SEE-threatened.
  If an earlier node already holds a base IGNORES_THREAT event on that piece, there is no new event.
  Ours checks only the previous own move.
- Then `is_sacrifice_of_not_moved(n, square, true)`: not trapped, not absolutely pinned, relatively
  pinned only if `defensible_by_alternative`, plus checks on pinned or overloaded attackers, safe
  promotion squares, newly created strong threats and the most valuable piece the move threatens.
  Still only partly decoded. Our `counter_threat_excludes` and `standing_trade_excludes` are
  data-driven stand-ins for these checks, and `counter_threat` wrongly rejects two right Brilliants.
- **Indirect sacrifice:** pieces strongly threatened after the move and not SEE-threatened before;
  the most valuable goes through `is_sacrifice_of_not_moved(n, square, false)`.

### 4.5 High-rated branch

`piece_value_of_greatest_discovered_attack` reads `discovered_attack_profile`, not "any piece newly
attacked". Ours counts any newly attacked piece, which wrongly turned Nfg4+ and Qxd4 into disguised
trades. To decode: `discovered_attack_profile`, `has_non_pinned_threatener`, `is_deflection`.

### 4.6 Material gate

Three misses fail it, including a mating sacrifice (Bxh2+). The earlier reading ("opponent's EP
before the move below 0.10 at rating 2000") may have the perspective or the material term wrong.
To decode: the gate's instructions in `compute_board_event<13>` and `material_difference`.

### 4.7 Not yet looked at

`saved_pieces`, the endgame gate's `max_material_gain`, `ForkIfCaptured`.

## 5. What cannot be matched

Ten cases depend on the engine: 3 misses (we grade the move Inaccuracy, or EP after is 0.469) and
7 extras (Chess.com grades the move Inaccuracy, or calls it Book). Chess.com's engine and depth
differ from ours, so these stay. With every rule matched, the best reachable is about 95 of 98 found
with about 7 extras. "Exactly the same as Chess.com" is not reachable while the engines differ.

## 6. Plan

Each step is a rule read from the decompile, not a threshold fitted to labels. Each is measured
alone, on tuning and holdout separately.

1. **Strong threats at 200** for all three sacrifice kinds (§4.1). Expected: −14 extras, no loss.
2. **Chess.com's safe squares** (§4.2): a destination is safe when the move's own SEE is ≥ 0. Use it
   for "had a safe square" and drop the ignored-threat safe-square test. Expected: up to +8 found.
3. **Walk-back by piece** (§4.4): no new ignored threat if an earlier node, while the piece stayed
   threatened, already had one on it.
4. **Decode `is_sacrifice_of_not_moved`** fully, implement it, and remove `counter_threat_excludes`
   and `standing_trade_excludes`. This targets the 39 wrong ignored threats.
5. **Decode the high-rated branch and the material gate** (§4.5, §4.6).
6. Re-check `pawns`: with strong threats it should make no difference, so drop the flag.

## 7. Guarding against overfitting

- **Source of each rule.** Only decompiled logic goes in. A data-driven rule is allowed only if it
  improves both tuning and holdout, and it is named as such in the docs. The two current ones
  (`counter_threat_excludes`, `standing_trade_excludes`) are to be replaced, not tuned.
- **No threshold search.** Constants come from the binary (100, 200, 0.47, 0.10).
- **Holdout.** Tuning and holdout are reported for every step; holdout is never looked at to choose a
  rule.
- **A second, independent test set.** Chess.com also explains its own best move when the player
  missed it. In 255 positions that best move is tagged as a Brilliant sacrifice, and in thousands of
  others it is not. None of these moves was played, so they were never used for tuning. Our rule,
  applied to Chess.com's best move (the same as ours in 225 of the 255), must agree there too.
- **Per-case check.** Every remaining difference is listed with its trace reason, so a change that
  only moves numbers without fixing a named cause shows up.


## 8. Result (2026-10-24, after the plan)

Each step was a rule read from the decompile (reports 2 and 3 in WSL `~/work/claude-scratchpad`),
measured alone on top of the previous one. No threshold was searched.

| Step | Rule | All P / R | Tuning P / R | Holdout P / R | Best-move set, tuning P / R | holdout P / R |
|---|---|---|---|---|---|---|
| start (`62704b1`) | | 0.51 / 0.79 | 0.53 / 0.81 | 0.45 / 0.73 | 0.61 / 0.88 | 0.49 / 0.87 |
| 1 | strong threats, SEE ≥ 200 | 0.56 / 0.79 | 0.60 / 0.81 | 0.47 / 0.73 | 0.63 / 0.88 | 0.54 / 0.87 |
| 2 | reference safe squares | 0.56 / 0.82 | 0.59 / 0.83 | 0.49 / 0.77 | 0.63 / 0.90 | 0.54 / 0.87 |
| 3+4 | `ignored_threats` with walk-back, `is_indirect_sacrifice`, `is_sacrifice_of_not_moved` | 0.60 / 0.83 | 0.62 / 0.85 | 0.56 / 0.77 | 0.66 / 0.94 | 0.59 / 0.91 |
| 5 | moved piece: strong mutual and saved-piece tests, reference discovered attack | **0.61 / 0.85** | 0.62 / 0.88 | 0.56 / 0.77 | 0.65 / 0.95 | 0.59 / 0.93 |

Every step moved tuning, holdout and the independent best-move set (§7) the same way. Exact
agreement (68.9%) and Great (0.84 / 0.76) are unchanged.

**The best-move set** holds Chess.com's own best move in positions where the player did not play it,
its EP after that move is at least 0.47, and Chess.com explained it: 255 tagged as a Brilliant
sacrifice, 11,284 with no sacrifice tag. It was never used to choose a rule. Our sacrifice kind
agrees with Chess.com's tag on all but 4 of the 240 we find.

**New facts from the code.**
- `ignored_threats` picks the single most valuable piece strongly threatened both before and after
  the move; with a tie of equal value there is no ignored threat. It has no safe-square test. Its
  walk-back steps back two plies at a time while that piece stays SEE-threatened on its square, and
  stops the event if an earlier move already carried IGNORES_THREAT on the same piece.
- `is_sacrifice_of_not_moved` rejects the piece when the move saved a piece worth as much, when the
  moved piece was itself threatened and worth as much, when what the piece loses is no more than what
  the move captured plus the best new strong threat it creates, and when the moved piece hits back
  at something at least as valuable that is not the piece's own attacker. Only when ignoring a
  threat: the piece must not be trapped or pinned to its king.
- A discovered attack is a new strong threat by a slider the move uncovers.
- The material gate (§4.6) was re-read in the assembly: it rejects when the mover is ahead by at
  least the piece's value and the parent node's `score()` is below `EP_to_cp(0.10, 2000)`. Both
  decompile reports agree on the comparisons; they disagree on the score's point of view. The
  opponent's view rejects 34 moves Chess.com did not call Brilliant and 3 it did; the mover's view
  rejects none. We keep the opponent's view. The 4 Brilliants it still rejects all have the mover
  ahead by exactly the piece's value.

**Left (15 missed, 54 extra).** Missed: 4 by the material gate at equality (above), 3 engine grade or
EP differences, 1 endgame gate, 7 where we find no sacrifice (three have no Chess.com tag decoded).
Extras: Chess.com called 20 of them Best, 13 Great, 12 Excellent; it saw no sacrifice in any. The
parts still not modelled are `has_pinned_attacker`, `has_overloaded_attacker` (deflection),
`defensible_by_alternative`, the safe-promotion test, `ForkIfCaptured` and the endgame gate's
`max_material_gain` (`moves_better_than(n, 6)`).
