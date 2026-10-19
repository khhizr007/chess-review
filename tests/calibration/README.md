# Calibration dataset

Ground truth for comparing our review labels with Chess.com Game Review. See
`docs/calibration/agent-instructions.md` for the protocol and `docs/calibration/log.md` for results.

- `games/<id>.pgn`: the game exactly as the public API (`api.chess.com/pub/player/<user>/games/YYYY/MM`)
  returned it.
- `games/<id>.labels.json`: the reference app's per-move labels, in the format given in the
  instructions, plus the additions below.
- `tuning.txt` / `holdout.txt`: the split (see the header comment in each). Never tune on holdout games.
- `runs/<name>/`: harness output (`report.md`, `summary.json`; per-game review JSON is not committed).

## Collection

59 games in two batches, all reviewed with Game Review set to engine **Stockfish**
(`stockfish16 nnue` in the site's settings) at strength **Fast** (depth 18):

- Batch 1 (2026-10-17/18): 20 games. Games whose review already existed were shown again with the
  same accuracy after the engine switch; the rest were reviewed fresh.
- Batch 2 (2026-10-18): 39 games drawn at random (seed 2026) from the October archives of khhizr7,
  boy1956, PeterSouth1954, Hikaru, GothamChess and IMRosen, by source and time control, plus three
  draws; 2,915 moves.
- Batch 3 (2026-10-18): game 185058622904 (khhizr7, a Traxler counter-attack with two Brilliants),
  reported by the owner when our review gave it none. Its labels come straight from the captured
  `/v2/game-review` response (`label_source: "chesscom-capture"`); it is in the tuning split because the
  Brilliant rule was designed on it.
- Batch 4 (2026-10-19): 82 of khhizr7's rapid games (30+ plies, not already in the dataset) in
  `random.Random(2027)` order from the archives 2025/09 to 2026/10, reviewed one after another until
  the batch held 16 Brilliants (15 games). Labels, best moves and evaluations come straight from the
  captured `/v2/game-review` responses (`cc-reviews/<id>.sf.txt`, `label_source: "chesscom-capture"`);
  every played move was checked against the PGN's SAN and every best move for legality. The page
  reviewed each game with engine 10 (Stockfish setting) and every score came back from engine 13
  (Stockfish 18).

Labels were read move by move from the review panel (label text and icon), and the per-move labels
reproduce the counts table for both players in every game.

## Format additions

| Field | Meaning |
|---|---|
| `label: "forced"` | The app showed "Forced" (only legal or only sensible move). It is not a row of the counts table; the harness scores it as `best`. |
| `label: "uncertain:a/b"` | The label was hidden (the review showed a "Skill" note instead) and the counts table leaves more than one possibility. Excluded from scoring. |
| `hidden` | The note that hid the label, for example `"Skill"`. |
| `label_source: "counts"` | A hidden label recovered from the counts table because only one label was short. |
| `app_best_uci` / `app_best_san` | The green "best move" arrow, when the app drew one. `app_best_san` is filled by `tools/calibrate/fill_best_san.py`. |
| `app_eval` | The evaluation shown after the played move, from White's point of view (`+0.25`, `-1.40`, `M3`, `-M2`). `null` on the final move, where the app shows the result instead. |

Rebuild from raw captures with `tools/calibrate/build_dataset.py`, then run `fill_best_san.py` again
(the builder writes `app_best_san: null`).
