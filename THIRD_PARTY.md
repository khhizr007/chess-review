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
| React, React DOM | UI | MIT | Bundled into the frontend build |
| [chessground](https://github.com/lichess-org/chessground) 9.2.1 (Lichess's board) | Pieces, dragging and move animation on the board (`frontend/src/components/BoardView.tsx`); its base stylesheet | GPL-3.0-or-later | Bundled into the frontend build |
| [Stockfish.js](https://github.com/nmrugg/stockfish.js) 19.0.1, build `stockfish-19-lite-single` (Stockfish 19 with the `nn-61e7af4bb97d` net by Chris Bao) | Live analysis of sideline positions in the reader's browser (`frontend/public/engine/`) | GPL-3.0 | **Yes**, unmodified: the `.js` and `.wasm` files from the npm package `stockfish@19.0.1`. The corresponding source is the upstream repository at [tag v19.0.1](https://github.com/nmrugg/stockfish.js/tree/v19.0.1). |
| [chessops](https://github.com/niklasf/chessops) | Legal moves, SAN and FEN for the reader's own sideline moves (`frontend/src/lib/chess.ts`) | GPL-3.0-or-later | Bundled into the frontend build |
| [@badrap/result](https://github.com/badrap/result) (dependency of chessops) | Result type | MIT | Bundled into the frontend build |
| Piece outlines by Colin M.L. Burnett ("cburnett", as shipped in react-chessboard and Lichess) | Piece geometry in `frontend/src/lib/pieceShapes.ts`; the shading and colours are this project's own | GPL-2.0-or-later / BSD / GFDL | Bundled into the frontend build |

## Reference application

The review rules were recovered by analysing a commercial Android application. The project contains
**no** code, assets, data files or credentials from that application.

- The formulas and thresholds were reimplemented independently in `backend/crates/review-core`.
- Book moves use the public-domain opening list above, not the reference app's embedded book.
- The UI uses its own styling and symbols.
- Sounds play on the reference client's events, but each one is synthesised in the browser by
  this project's code (`frontend/src/lib/synth.ts`). No audio files are included. A developer may install
  recordings of their own into the gitignored `frontend/public/sounds/` (`tools/sounds/install-sound-pack.sh`).
  Those recordings are not part of the project and must not be committed or distributed.
- The web UI's colour values and spacing follow the reference website's published dark theme, observed in its
  public stylesheets. No images, piece sets, logos, fonts or code from it are included.

The sample game in the frontend and `backend/crates/review-server/tests/fixtures/opera_game.pgn`
(Morphy, 1858) and `stalemate.pgn` (Loyd's composition) are public domain. The games in
`tests/fixtures/calibration-sample/` are rated online games as the site's public API returned them.
The other fixtures are hand-written.
