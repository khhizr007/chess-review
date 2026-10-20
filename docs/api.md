# HTTP API

The base path is `/api`. Everything is JSON. The TypeScript types for every request and response are
generated from the Rust definitions into `frontend/src/api/generated/` (run `cargo test` in
`backend/` to regenerate them).

## Conventions

**Errors.** Every error response has this shape:

```json
{ "error": { "code": "pgn_invalid", "message": "game 1, ply 7 (4.): \"Bxe8\" is not a legal move: …", "details": { … } } }
```

| Status | Codes |
|---|---|
| 400 | `pgn_empty`, `pgn_invalid`, `invalid_settings`, `game_not_found`, `no_moves`, `invalid_fen` |
| 404 | `not_found` |
| 409 | `review_not_ready` (`details` holds the job status) |
| 422 / 415 | `invalid_request_body` |
| 502 | `engine_error` |
| 503 | `engine_unavailable` |

**Evaluations (`Eval`).** These are always from **White's point of view**:
- `{ "kind": "cp", "cp": 34 }`
- `{ "kind": "mate", "winner": "white", "moves": 3 }`. `moves: 0` means the position is checkmate.

**Expected points.** Values in `[0, 1]`. Per-move fields (`ep_before`, `ep_after`, `ep_best`) are from
the **mover's** point of view. Graph points carry White's.

## Endpoints

### `GET /api/health`
Returns the engine identity, or why it is unavailable.

```json
{ "status": "ok", "version": "0.1.0", "engine": "Stockfish 19", "engine_error": null, "opening_positions": 7982 }
```

### `GET /api/config/defaults`
Returns the recovered review constants (`ReviewConfig`) and the strength presets (fast 12, standard 18,
deep 22, maximum 26 plies).

### `POST /api/games/parse`
Request: `{ "pgn": "…" }`. Validates every game in a PGN without analysing it.

Each entry in `games` has either a `game` (players, ratings, event, date, result, `ply_count`,
`start_fen`, and `moves` with SAN, UCI and the FEN after each move), or an `error`. The error is a
tagged `PgnError` (`illegal_move` with game, ply and SAN; `invalid_start_position`;
`unsupported_variant`) plus `error_message`.

### `POST /api/reviews` → `202 Accepted`

```json
{ "pgn": "…", "game_index": 0, "strength": "standard", "multipv": 3 }
```

**Options:**
- Use either `depth` (1–40) or `movetime_ms` (50–60000) instead of `strength`.
- `multipv` is 1–5.
- `config` (a `ReviewConfig`) overrides the review rules.
- `player_side` (`"white"` or `"black"`) is the side the user played. The coach then talks to them
  ("you allow …", "your opponent missed …"); without it, it names the colours. It is echoed in the
  result's `settings.player_side`.

**Response:** a `ReviewStatus`:

```json
{ "id": "uuid", "state": "queued", "progress": { "done": 0, "total": 0 }, "error": null }
```

**Behaviour:**
- Reviews run in the background.
- At most `MAX_CONCURRENT_REVIEWS` run at once; the rest wait in `queued`.
- Results are kept in memory, for up to `MAX_STORED_REVIEWS` finished jobs.

### `GET /api/reviews/{id}`
Returns the status with `state` (`queued | running | completed | failed | cancelled`) and `progress`
(searches done out of the total: one per position, plus the coach's null-move threat searches; the
short threat searches after the best move run last and are not counted). Poll it every 0.5–1 s.

### `DELETE /api/reviews/{id}`
Requests cancellation. Running searches receive `stop`, and the job ends as `cancelled`.

### `GET /api/reviews/{id}/result`
Returns the full `ReviewResult` (or 409 until it is complete). Top-level fields:

| Field | Content |
|---|---|
| `game` | Headers, players, ratings, result, termination, start and final FEN, Chess960 flag, opening name |
| `engine`, `settings`, `rating_used`, `config` | The settings that produced the review |
| `plies[]` | One `PlyReview` per move (see below) |
| `white`, `black` | Accuracy (`overall`, `opening`, `middlegame`, `endgame`), the tally in display order (Forced counted as Best), and counts |
| `graph[]` | One point per position (ply 0 to N): `eval`, `white_expected_points`, and the classification of the move that led there |
| `key_moments`, `critical_moments`, `biggest_swings` | Ply numbers |
| `phase_starts` | First ply of the middlegame and the endgame |
| `notes[]` | Known deviations from the reference that apply to this review |

Each `PlyReview` contains:

| Field | Content |
|---|---|
| `ply`, `move_number`, `mover`, `san`, `uci`, `from`, `to`, `fen_before`, `fen_after`, `flags` | The move itself. `flags`: check, checkmate, stalemate, capture, castle, promotion, en passant. |
| `eval_before`, `eval_after`, `eval_best` | Evaluations (White's view) |
| `ep_before`, `ep_after`, `ep_best`, `ep_lost` | Expected points (mover's view) |
| `classification`, `miss_kind`, `precision`, `rule` | The verdict. `rule` says which rule decided it, for traceability. |
| `best_move`, `played_is_best`, `show_best_move` | The engine's best move, and the reference predicate for showing it |
| `lines[]` | MultiPV lines from the position before the move. Each move carries SAN, UCI and the FEN after it, so the UI can preview lines without chess logic. |
| `accuracy` | CAPS2 move score |
| `phase`, `is_book`, `is_key_moment`, `is_critical`, `material_lost_cp` | Context flags |
| `explanation[]` | Generated facts (shown in the details drawer) |
| `coach` | The coach note for every classified move except Book and Forced (empty for those): `sentences[]` (each a list of `fragments` with `text`, `arrows` and `squares`, plus the index of the `event` it says), `show_lines[]` (`label`, `start_fen`, `moves`, one `narration` per move) and `events[]` (`chain` of circumstances, `tag`, `piece`, `square`, `actor`, `geometry`, `line` in UCI, `ply`, `gain_cp`, `mate_in`). See `docs/implementation-notes.md`, "Coach". |
| `pgn_comment` | The comment from the imported PGN |

### `GET /api/reviews/{id}/plies/{ply}`
Returns one `PlyReview` (1-based ply).

### `POST /api/positions/analyze`
Request: `{ "fen": "…", "depth": 18, "multipv": 3 }`. Analyses any position on demand and returns
`lines` (same shape as above), or `terminal: "checkmate" | "stalemate"`.
