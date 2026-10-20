# Configuration

## Backend (environment variables)

| Variable | Default | Meaning |
|---|---|---|
| `STOCKFISH_PATH` | `stockfish` found on `PATH` | UCI engine executable. Without one the server starts in "degraded" mode: import and parsing work, reviews return 503. |
| `BIND_ADDR` | `127.0.0.1:8080` | Listen address |
| `ENGINE_POOL_SIZE` | half the CPUs, 1–4 | Engine processes, which is also the maximum number of parallel searches |
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
4-core machine the defaults (2 × 1) keep the desktop responsive. Increase them on bigger machines for
faster reviews.

## Review rules

The defaults are the recovered constants (`GET /api/config/defaults`). A review request may pass
`config` to override them, for example `{"great_find": {"min_gap_to_second_best": 0.15}}` merged over
the defaults. The fields are documented in `backend/crates/review-core/src/config.rs`.

`coach.threat_search_depth` (default 10) sets the depth of the coach's null-move threat search, which
finds what a move threatens and what threat it stops. `{"coach": {"threat_search_depth": 0}}` turns it
off; classifications do not change, only the coach's notes. `coach.best_threat_search` (default
`true`) also runs it after the engine's best move wherever the played move was not that move, so the
coach can say what the better move would have threatened.

`coach.rating_gates` (default `true`) leaves out ideas above the player's level: each coach tag has a
`min_rating` in `backend/crates/review-server/src/coach/text/en.toml` (tactics and development are
said to everyone, activity from about 600–900, pawn structure from about 900–1100). The rating is the
player's Elo from the PGN when the request names `player_side`, else the rating the review uses for
expected points (the higher Elo plus 200, at least 1000). When the only idea of a move is gated, the
coach says nothing about it.

## Frontend

| Variable | Default | Meaning |
|---|---|---|
| `VITE_BACKEND_URL` | `http://127.0.0.1:8080` | Where the Vite dev server proxies `/api` |

## Maia-2 (optional)

[Maia-2](https://github.com/CSSLab/maia2) (MIT) predicts the moves human players of a given rating
make. With it, the coach says when the best move was hard to find at the player's level (P(best) below
0.10) and leads with the safety check when a Mistake or Blunder was a natural move (P(played) at least
0.30). It runs as a separate Python process, like Stockfish, and is not bundled:

```
python3 -m venv ~/maia && ~/maia/bin/pip install maia2
MAIA_COMMAND="$HOME/maia/bin/python3 tools/maia/maia_sidecar.py --type rapid" ./target/release/review-server
```

The `maia2` package downloads its model weights on first start. If the sidecar fails to start or
answer, reviews go on without it (a warning is logged).

## Installing Stockfish

Stockfish is GPL-3.0 and is **not** bundled. Install it from your package manager
(`apt install stockfish`, `brew install stockfish`) or from https://stockfishchess.org/download/.
Then point `STOCKFISH_PATH` at the binary. Any UCI engine with `MultiPV` support works. The
reference application used Stockfish 16.1.
