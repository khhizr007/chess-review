# Third-party components

This project is licensed GPL-3.0-or-later (see `LICENSE`). This was chosen because the chess
rules and PGN libraries below are GPL-3.0.

| Component | Use | License | Distributed with this project? |
|---|---|---|---|
| [shakmaty](https://crates.io/crates/shakmaty) | Chess rules, FEN, SAN, UCI moves | GPL-3.0-or-later | Compiled into the backend |
| [pgn-reader](https://crates.io/crates/pgn-reader) | PGN parsing | GPL-3.0-or-later | Compiled into the backend |
| axum, tokio, tower-http, serde, tracing, uuid, lru, ts-rs, futures, async-trait, thiserror, toml | Web server, runtime, serialisation, coach text | MIT and/or Apache-2.0 | Compiled into the backend |
| [Stockfish](https://stockfishchess.org/) | Position search over UCI | GPL-3.0 | **No.** It is installed separately and launched as its own process (`STOCKFISH_PATH`). |
| [Maia-2](https://github.com/CSSLab/maia2) (Tang et al., NeurIPS 2024) | Optional human-move model for the coach's difficulty wording (`tools/maia/maia_sidecar.py`, `MAIA_COMMAND`) | MIT | **No.** The `maia2` Python package, PyTorch and the model weights are installed separately (the weights are downloaded by `maia2` on first use); the server talks to it as its own process. |
| [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings) | Opening names and book positions (`backend/data/openings/*.tsv`) | CC0 (public domain) | Yes (data files) |
| [lichess puzzle database](https://database.lichess.org/#puzzles) | Independent check of the coach's tactic detectors against the puzzle themes (`tools/calibrate/puzzle_check.py`, `docs/calibration/detector-check.md`) | CC0 (public domain) | Partly: the ids of the 14,190 sampled puzzles (`tools/calibrate/puzzle_ids.tsv`), and the FEN, moves and themes of the 110 puzzles quoted in `docs/calibration/detector-check.md`. The database itself is downloaded separately and not committed. |
| React, React DOM | UI | MIT | Bundled into the frontend build |
| [react-chessboard](https://github.com/Clariity/react-chessboard) | Board rendering, arrows, highlights | MIT | Bundled into the frontend build |
| Piece outlines by Colin M.L. Burnett ("cburnett", as shipped in react-chessboard) | Piece geometry in `frontend/src/components/pieces.tsx`; the shading and colours are this project's own | GPL-2.0-or-later / BSD / GFDL | Bundled into the frontend build |

## Reference application

The review rules were recovered by analysing a commercial Android application. The project contains
**no** code, assets, data files or credentials from that application.

- The documented formulas and thresholds (`docs/functional-spec.md`) were reimplemented
  independently in `backend/crates/review-core`.
- Book moves use the public-domain opening list above, not the reference app's embedded book.
- The UI uses its own styling and symbols.
- The web UI's colour values and spacing follow the reference website's published dark theme, observed in its
  public stylesheets. No images, piece sets, logos, fonts or code from it are included.

The sample game in the frontend and `tests/fixtures/opera_game.pgn` (Morphy, 1858) and
`stalemate.pgn` (Loyd's composition) are public domain. The other fixtures are hand-written.
