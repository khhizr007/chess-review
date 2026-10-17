# Third-party components

This project is licensed GPL-3.0-or-later (see `LICENSE`). This was chosen because the chess
rules and PGN libraries below are GPL-3.0.

| Component | Use | License | Distributed with this project? |
|---|---|---|---|
| [shakmaty](https://crates.io/crates/shakmaty) | Chess rules, FEN, SAN, UCI moves | GPL-3.0-or-later | Compiled into the backend |
| [pgn-reader](https://crates.io/crates/pgn-reader) | PGN parsing | GPL-3.0-or-later | Compiled into the backend |
| axum, tokio, tower-http, serde, tracing, uuid, lru, ts-rs, futures, async-trait, thiserror | Web server, runtime, serialisation | MIT and/or Apache-2.0 | Compiled into the backend |
| [Stockfish](https://stockfishchess.org/) | Position search over UCI | GPL-3.0 | **No.** It is installed separately and launched as its own process (`STOCKFISH_PATH`). |
| [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings) | Opening names and book positions (`backend/data/openings/*.tsv`) | CC0 (public domain) | Yes (data files) |
| React, React DOM | UI | MIT | Bundled into the frontend build |
| [react-chessboard](https://github.com/Clariity/react-chessboard) | Board rendering, arrows, highlights | MIT | Bundled into the frontend build |

## Reference application

The review rules were recovered by analysing a commercial Android application. The project contains
**no** code, assets, data files or credentials from that application.

- The documented formulas and thresholds (`docs/functional-spec.md`) were reimplemented
  independently in `backend/crates/review-core`.
- Book moves use the public-domain opening list above, not the reference app's embedded book.
- The UI uses its own styling and symbols.

The sample game in the frontend and `tests/fixtures/opera_game.pgn` (Morphy, 1858) and
`stalemate.pgn` (Loyd's composition) are public domain. The other fixtures are hand-written.
