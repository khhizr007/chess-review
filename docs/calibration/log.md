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
| 2026-10-18 | (fix 1) | `fix1-brilliant` | Fix 1: Brilliant needs the engine's best move, a clearly worse second-best move, a static sacrifice (new SEE), the EP window and the recovered gates | 68.2% | 89.6% | 1.00 / 0.33 | 0.71 / 0.19 | 2.96 (+2.79) | Kept: 59 false Brilliants gone and Great Find no longer pre-empted. Of the app's 3, 1 matches; 5.Nxe5 fails the second-best gap (0.067 EP at rating 1010) and 21...gxf3 is not our engine's best move. |
| 2026-10-18 | (fix 1) | `fix1-brilliant-hanging` | variant: also count a piece left en prise (`count_pieces_left_hanging`) | 68.2% | 89.6% | 1.00 / 0.33 | 0.71 / 0.19 | 2.96 (+2.79) | No effect on the tuning set; left off (the spec's definition) and revisited in Fix 6. |
