# Chess Game Review

A chess game-review application. Import a PGN, have every move searched with Stockfish and graded,
then step through the game with move classifications, best moves, alternative lines, accuracy, an
evaluation graph and a summary.

The grading rules reproduce the review logic recovered from a reference application. That logic is
documented in [`docs/functional-spec.md`](docs/functional-spec.md):
- the expected-points curve and its rating rule;
- the precision thresholds;
- the classification decision tree and mate rules;
- CAPS2 accuracy.

Where the reference behaviour could not be recovered, documented and configurable fallbacks are
used ([`docs/open-questions.md`](docs/open-questions.md)).

- **Backend:** Rust (axum, tokio, shakmaty, pgn-reader). The `review-core` crate holds the recovered
  rules with no I/O. `review-server` holds the API, PGN import, the Stockfish pool and the pipeline.
- **Frontend:** React + TypeScript (Vite, react-chessboard). Its API types are generated from the
  Rust types.

## Requirements

- Rust 1.88+ (`rustup`)
- Node.js 20+ and npm
- A UCI engine, normally [Stockfish](https://stockfishchess.org/download/). It is not bundled; see
  [`docs/configuration.md`](docs/configuration.md#installing-stockfish).

## Quick start (development)

```bash
# 1. Backend (http://127.0.0.1:8080)
cd backend
STOCKFISH_PATH=/path/to/stockfish cargo run --release -p review-server

# 2. Frontend (http://localhost:5173, proxies /api to the backend)
cd frontend
npm install
npm run dev
```

Open http://localhost:5173, then click **Load sample** (or paste or open a PGN) and **Start review**.
At the Standard preset (depth 18) a 40-move game takes about a minute on a 4-core machine. **Fast** is
much quicker.

**Board keys:** `←`/`→` move, `Home`/`End` first or last move, `f` flip, `Esc` leave a line preview.

## Single-process deployment

```bash
cd frontend && npm ci && npm run build
cd ../backend && cargo build --release -p review-server
STOCKFISH_PATH=/path/to/stockfish STATIC_DIR=../frontend/dist ./target/release/review-server
# → http://127.0.0.1:8080 serves both the UI and the API
```

All settings are environment variables; see [`docs/configuration.md`](docs/configuration.md).

## Tests and checks

```bash
cd backend
cargo test                       # unit + integration tests (fake engine, no Stockfish needed)
STOCKFISH_PATH=… cargo test --release --test stockfish_e2e -- --nocapture   # real-engine review
cargo clippy --all-targets && cargo fmt --all -- --check

cd frontend
npm test && npm run typecheck && npm run lint && npm run build
```

`cargo test` also regenerates `frontend/src/api/generated/*.ts`.

## Documentation

| Document | Content |
|---|---|
| [`docs/functional-spec.md`](docs/functional-spec.md) | Recovered behaviour, tagged Confirmed / Inferred / Unknown |
| [`docs/open-questions.md`](docs/open-questions.md) | Unresolved items and the fallback used for each |
| [`docs/architecture.md`](docs/architecture.md) | Components, data flow, concurrency |
| [`docs/api.md`](docs/api.md) | HTTP API |
| [`docs/implementation-notes.md`](docs/implementation-notes.md) | How the spec maps to code; fallback heuristics |
| [`docs/parity.md`](docs/parity.md) | Parity evidence and expected differences |
| [`docs/configuration.md`](docs/configuration.md) | Environment variables, engine installation |
| [`tests/fixtures/`](tests/fixtures/README.md) | Sample PGNs and the test corpus |

## License

GPL-3.0-or-later (see [`LICENSE`](LICENSE) and [`THIRD_PARTY.md`](THIRD_PARTY.md)). The project
contains no code or assets from the reference application.
