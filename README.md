# Chess Game Review

A chess game-review application. Import a PGN, have every move searched with Stockfish and graded,
then step through the game with move classifications, best moves, alternative lines, accuracy, an
evaluation graph and a summary. You can also play your own moves on the board to explore sidelines,
with a live Stockfish evaluation that runs in your browser.

The grading rules reproduce the review logic of a reference application:
- the expected-points curve and its rating rule;
- the precision thresholds;
- the classification decision tree and mate rules;
- CAPS2 accuracy.

Where the reference behaviour is not known, configurable fallbacks are used. The rules and their
defaults are documented in `backend/crates/review-core/src/config.rs`.

- **Backend:** Rust (axum, tokio, shakmaty, pgn-reader). The `review-core` crate holds the
  rules with no I/O. `review-server` holds the API, PGN import, the Stockfish pool and the pipeline.
- **Frontend:** React + TypeScript (Vite, chessground, chessops). Its API types are generated from the
  Rust types (`frontend/src/api/generated/`).

## Requirements

- Rust 1.88+ (`rustup`)
- Node.js 20+ and npm
- A UCI engine, normally [Stockfish](https://stockfishchess.org/download/). It is not bundled (see
  [Installing Stockfish](#installing-stockfish)).

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

Open http://localhost:5173. Then type your Chess.com or Lichess username under **Your games** and pick a game, or click **Load sample**, or paste or open a PGN. Then click **Start review**.
At the Standard preset (600,000 nodes per position, Chess.com's Game Review budget) a 40-move game
takes about 15 seconds on an 8-core machine. **Fast** is much quicker.

**Board keys:** `←`/`→` move, `Home`/`End` first or last move, `f` flip, `Esc` leave a line preview.
Moves and brilliant moves have sounds; the **Sound** switch turns them off.

## Docker

The image builds the frontend and the server, and bundles Stockfish 18 (official release binary)
and the Maia-2 sidecar with its rapid weights (CPU PyTorch; the image is about 2 GB, and Maia uses
about 400 MB of RAM). Run with `-e MAIA_COMMAND=` to turn Maia off.

```bash
docker build -t chess-review .
docker run --rm -p 127.0.0.1:8080:8080 -e ENGINE_POOL_SIZE=2 chess-review
# or: docker compose up --build
```

Then open http://127.0.0.1:8080. Every variable in [Configuration](#configuration) can be set with
`-e`. Build arguments: `STOCKFISH_VERSION` (default `sf_18`) and `STOCKFISH_ARCH` (default
`x86-64-avx2`; use `x86-64` on CPUs without AVX2). Reviews live in memory and are lost when the
container stops.

A recorded sound pack can be pulled from another GitHub repository at build time. Put
`SOUNDS_REPO=owner/name` (and, if needed, `SOUNDS_DIR` and `SOUNDS_REF`) in a `.env` file, then
build with a read-only token for a private repository:

```bash
GITHUB_TOKEN=<token> docker compose up -d --build
```

The token is a build secret and is not stored in the image. Without `SOUNDS_REPO` the app uses its
synthesised sounds.

## Single-process deployment

```bash
cd frontend && npm ci && npm run build
cd ../backend && cargo build --release -p review-server
STOCKFISH_PATH=/path/to/stockfish STATIC_DIR=../frontend/dist ./target/release/review-server
# → http://127.0.0.1:8080 serves both the UI and the API
```

All settings are environment variables (see [Configuration](#configuration)).

## Configuration

### Backend (environment variables)

| Variable | Default | Meaning |
|---|---|---|
| `STOCKFISH_PATH` | `stockfish` found on `PATH` | UCI engine executable. Without one the server starts in "degraded" mode: import and parsing work, reviews return 503. |
| `BIND_ADDR` | `127.0.0.1:8080` | Listen address |
| `ENGINE_POOL_SIZE` | half the CPUs, 1–8 | Engine processes, which is also the maximum number of parallel searches. Each Stockfish process uses about 370 MB of RAM. |
| `ENGINE_THREADS` | `1` | `Threads` per engine process |
| `ENGINE_HASH_MB` | `64` | `Hash` per engine process |
| `ENGINE_SEARCH_TIMEOUT_SECS` | `60` | Hard limit per position |
| `ANALYSIS_CACHE_ENTRIES` | `20000` | LRU cache of position analyses (0 disables it) |
| `OPENINGS_DIR` | `./data/openings` if present | Opening TSVs. Without them, no move is classified as Book. |
| `MAX_CONCURRENT_REVIEWS` | `2` | Reviews that run at the same time; others queue |
| `MAX_STORED_REVIEWS` | `100` | Finished reviews kept in memory |
| `STATIC_DIR` | unset | Serve a built frontend (`frontend/dist`) from the backend |
| `CORS_ORIGIN` | unset | Allowed browser origin. Only needed when the frontend is served from another origin without the Vite proxy. |
| `MAIA_COMMAND` | unset | Command that starts the optional Maia-2 sidecar (see below). Without it the coach does not judge how hard a move was to find. |
| `RUST_LOG` | `info,tower_http=warn` | Log filter |

**CPU budget.** CPU use is about `ENGINE_POOL_SIZE × ENGINE_THREADS` cores while a review runs. On a
4-core machine the defaults (2 × 1) keep the desktop responsive.

**Review rules.** The defaults are returned by `GET /api/config/defaults`. A review request may pass
`config` to override them; the fields are documented in `backend/crates/review-core/src/config.rs`.

### Frontend

| Variable | Default | Meaning |
|---|---|---|
| `VITE_BACKEND_URL` | `http://127.0.0.1:8080` | Where the Vite dev server proxies `/api` |

### Maia-2 (optional)

[Maia-2](https://github.com/CSSLab/maia2) (MIT) predicts the moves human players of a given rating
make; the coach uses it to say when the best move was hard to find. It runs as a separate Python
process; the Docker image bundles it, otherwise install it yourself. `maia2` 0.10.0 needs Python
3.12 or 3.13:

```bash
uv venv --python 3.12 ~/maia312
uv pip install --python ~/maia312/bin/python torch==2.8.0 --index-url https://download.pytorch.org/whl/cpu
uv pip install --python ~/maia312/bin/python maia2==0.10.0 --extra-index-url https://download.pytorch.org/whl/cpu --index-strategy unsafe-best-match
MAIA_COMMAND="$HOME/maia312/bin/python tools/maia/maia_sidecar.py --type rapid" ./target/release/review-server
```

On first start the sidecar downloads the weights (280 MB) into `~/.cache/maia2`. If it fails to
start or answer, reviews go on without it.

### Installing Stockfish

The server's Stockfish is GPL-3.0 and is **not** bundled. Install it from your package manager
(`apt install stockfish`, `brew install stockfish`) or from https://stockfishchess.org/download/,
then point `STOCKFISH_PATH` at the binary. Any UCI engine with `MultiPV` support works.

The live engine for sidelines is a separate, bundled WebAssembly build (Stockfish 19 Lite,
`frontend/public/engine/`, see `THIRD_PARTY.md`). It runs in the reader's browser and adds no work to
the server. Any server that serves `.wasm` as `application/wasm` can host it; with `STATIC_DIR` the
backend does.

## Tests and checks

```bash
cd backend
cargo test                       # unit + integration tests (fake engine, no Stockfish needed)
STOCKFISH_PATH=… cargo test --release --test stockfish_e2e -- --nocapture   # real-engine review
cargo clippy --all-targets && cargo fmt --all -- --check

cd frontend
npm test && npm run typecheck && npm run lint && npm run build
```

`cargo test` also regenerates `frontend/src/api/generated/*.ts`. The test PGNs are in
[`backend/crates/review-server/tests/fixtures/`](backend/crates/review-server/tests/fixtures/README.md).

## License

GPL-3.0-or-later (see [`LICENSE`](LICENSE) and [`THIRD_PARTY.md`](THIRD_PARTY.md)). The project
contains no code or assets from the reference application.
