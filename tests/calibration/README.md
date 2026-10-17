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

All 20 games were reviewed on 2026-10-17/18 with Game Review set to engine **Stockfish**
(`stockfish16 nnue` in the site's settings) at strength **Fast** (depth 18). Games whose review
already existed were shown again with the same accuracy after the engine switch; the rest were
reviewed fresh. Labels were read move by move from the review panel (label text and icon), and the
per-move labels reproduce the counts table for both players in every game.

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
