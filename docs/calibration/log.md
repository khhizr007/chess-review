# Calibration log

Each row is one harness run on the **tuning** set (14 games, 856 scored plies) unless marked
holdout. Settings for every run: Stockfish 17.1 (our engine), depth 18, MultiPV 3. The reference
labels come from Chess.com Game Review with engine Stockfish (`stockfish16 nnue`) at Fast (depth 18);
see `tests/calibration/README.md`. Reports are in `tests/calibration/runs/<run>/report.md`.

Exact = labels equal after normalisation; within-one = at most one step apart on
best > excellent > good > inaccuracy > mistake > blunder (book, brilliant, great and miss must match).
P / R = precision / recall. Acc MAD = mean absolute difference from the app's per-player accuracy
(bias in brackets, ours minus app).

| date | commit | run | change | exact | within-one | Brilliant P / R | Great P / R | acc MAD (bias) | conclusion |
|---|---|---|---|---:|---:|---|---|---|---|
| 2026-10-18 | `9ddf4a5` | `baseline` | none (code as of `5bc590e`) | 64.6% | 84.1% | 0.03 / 0.67 | 0.75 / 0.12 | 3.24 (+2.73) | Brilliant fires 60 times against the app's 3; Great Find almost never fires (4 vs 26). |
| 2026-10-18 | `c76a4ce` | `fix1-brilliant` | Fix 1: Brilliant needs the engine's best move, a clearly worse second-best move, a static sacrifice (new SEE), the EP window and the recovered gates | 68.2% | 89.6% | 1.00 / 0.33 | 0.71 / 0.19 | 2.96 (+2.79) | Kept: 59 false Brilliants gone and Great Find no longer pre-empted. Of the app's 3, 1 matches; 5.Nxe5 fails the second-best gap (0.067 EP at rating 1010) and 21...gxf3 is not our engine's best move. |
| 2026-10-18 | `c76a4ce` | `fix1-brilliant-hanging` | variant: also count a piece left en prise (`count_pieces_left_hanging`) | 68.2% | 89.6% | 1.00 / 0.33 | 0.71 / 0.19 | 2.96 (+2.79) | No effect on the tuning set; left off (the spec's definition) and revisited in Fix 6. |
| 2026-10-18 | (not kept) | `fix2-material` | Fix 2 as specified: material loss = immediate SEE exposure | 68.0% | 88.7% | 0.50 / 0.33 | 0.67 / 0.15 | 3.16 (+3.03) | Inconclusive: between two server runs the engine numbers changed on 799 of 865 plies and about 120 labels flipped each way, far above the effect being measured. Set aside until searches are deterministic; it also softened 4 real Blunders to Mistake. |
| 2026-10-18 | `b9f8aad` | `fix3a-deterministic` | Fix 3, determinism part, done before Fix 2: `ucinewgame` + Clear Hash before every search (Threads=1) | 70.4% | 89.1% | 1.00 / 0.33 | 0.56 / 0.19 | 3.19 (+2.79) | Kept. Labelling rules are as in Fix 1, so the change from `fix1-brilliant` is engine noise, not an improvement. This is the reference run for later changes. |
| 2026-10-18 | `b9f8aad` | `fix3a-deterministic-repeat` | the same code on a fresh server | 70.4% | 89.1% | 1.00 / 0.33 | 0.56 / 0.19 | 3.19 (+2.79) | Identical ply for ply (0 of 865 plies differ), so later runs can be compared exactly. |
| 2026-10-18 | (reverted) | `fix2-material-det` | Fix 2 as specified, now on the deterministic engine | 69.9% | 89.1% | 1.00 / 0.33 | 0.56 / 0.19 | 3.19 (+2.79) | 13 labels change, 4 better and 7 worse. Blunder softening hurts most: 4 of the app's Blunders lose material through a later tactic, not an immediate exchange, and become Mistakes. |
| 2026-10-18 | (reverted) | `fix2-material-adj` | adjusted once: blunder softening also counts the engine line; the rest exchange-based | 70.2% | 89.1% | 1.00 / 0.33 | 0.56 / 0.19 | 3.19 (+2.79) | Reverted: 2 better, 4 worse. Hardening of Best/Excellent/Good moves, the problem this fix targets, occurs once on the tuning set after Fix 1 (vs 16 of 25 Mistakes in the original audit), so there is little left to gain. |
| 2026-10-18 | (reverted) | `fix3b-searchmoves` | Fix 3, scoring part: a played move outside the MultiPV lines is scored by a search of the same position restricted to it (`searchmoves`) instead of the next position's search | 68.8% | 89.0% | 1.00 / 0.33 | 0.56 / 0.19 | 3.25 (+2.82) | Reverted: 212 plies get new scores, 12 labels better, 26 worse. The restricted search rates the played move higher (many app Inaccuracies become our Good), so the app's grades match the next-position search, which finds the refutation at full depth. `searchmoves` stays available in the engine layer. |
