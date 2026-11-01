# Third-party components

This project is licensed GPL-3.0-or-later (see `LICENSE`). This was chosen because the chess
rules and PGN libraries below are GPL-3.0.

| Component | Use | License | Distributed with this project? |
|---|---|---|---|
| [shakmaty](https://crates.io/crates/shakmaty) | Chess rules, FEN, SAN, UCI moves | GPL-3.0-or-later | Compiled into the backend |
| [pgn-reader](https://crates.io/crates/pgn-reader) | PGN parsing | GPL-3.0-or-later | Compiled into the backend |
| axum, tokio, tower-http, serde, tracing, uuid, lru, ts-rs, futures, async-trait, thiserror, toml | Web server, runtime, serialisation, coach text | MIT and/or Apache-2.0 | Compiled into the backend |
| [Stockfish](https://stockfishchess.org/) | Position search over UCI | GPL-3.0 | **No.** It is installed separately and launched as its own process (`STOCKFISH_PATH`). |
| [Maia-2](https://github.com/CSSLab/maia2) (Tang et al., NeurIPS 2024) | Optional human-move model for the coach's difficulty wording (`tools/maia/maia_sidecar.py`, `MAIA_COMMAND`) | MIT | **Only in the Docker image.** The repository holds only the sidecar script. The `Dockerfile` installs the `maia2` package and PyTorch and downloads the model weights from the location the authors publish; outside Docker they are installed separately (the weights are downloaded by `maia2` on first use). The server talks to it as its own process. |
| [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings) | Opening names and book positions (`backend/data/openings/*.tsv`) | CC0 (public domain) | Yes (data files) |
| React, React DOM | UI | MIT | Bundled into the frontend build |
| [chessground](https://github.com/lichess-org/chessground) 9.2.1 (Lichess's board) | Pieces, dragging and move animation on the board (`frontend/src/components/BoardView.tsx`); its base stylesheet | GPL-3.0-or-later | Bundled into the frontend build |
| [Stockfish.js](https://github.com/nmrugg/stockfish.js) 19.0.1, build `stockfish-19-lite-single` (Stockfish 19 with the `nn-61e7af4bb97d` net by Chris Bao) | Live analysis of sideline positions in the reader's browser (`frontend/public/engine/`) | GPL-3.0 | **Yes**, unmodified: the `.js` and `.wasm` files from the npm package `stockfish@19.0.1`. The corresponding source is the upstream repository at [tag v19.0.1](https://github.com/nmrugg/stockfish.js/tree/v19.0.1). |
| [chessops](https://github.com/niklasf/chessops) | Legal moves, SAN and FEN for the reader's own sideline moves (`frontend/src/lib/chess.ts`) | GPL-3.0-or-later | Bundled into the frontend build |
| [@badrap/result](https://github.com/badrap/result) (dependency of chessops) | Result type | MIT | Bundled into the frontend build |
| Piece outlines by Colin M.L. Burnett ("cburnett", as shipped in react-chessboard and Lichess) | Piece geometry in `frontend/src/lib/pieceShapes.ts`; the shading and colours are this project's own | GPL-2.0-or-later / BSD / GFDL | Bundled into the frontend build |

The sample game in the frontend and `backend/crates/review-server/tests/fixtures/opera_game.pgn`
(Morphy, 1858) and `stalemate.pgn` (Loyd's composition) are public domain. The games in
`tests/fixtures/calibration-sample/` are rated online games as the site's public API returned them.
The other fixtures are hand-written.
