# Test corpus

These PGNs are used by this crate's tests (`backend/crates/review-server/tests/*` and one unit test in
`src/analysis/detectors.rs`) and are good for manual testing in the UI.

| File | Purpose |
|---|---|
| `scholars_mate.pgn` | Short ordinary game, blunder into mate, checkmate, full metadata |
| `opera_game.pgn` | Tactical game with sacrifices (Brilliant), castling queenside, forced move, mate (public domain, 1858) |
| `missed_mate.pgn` | A mate in 1 that was missed (Miss/mate), unfinished result `*` |
| `back_rank.pgn` | Non-standard FEN start, missed back-rank mate, forced king moves, ends drawn |
| `stalemate.pgn` | Loyd's ten-move stalemate: huge eval swings, stalemate instead of mate |
| `special_moves.pgn` | En passant, promotion, kingside castling; ends by resignation |
| `agreed_draw.pgn` | Draw by agreement; long book line |
| `sparse_metadata.pgn` | Missing and `?` headers, no ratings, no result |
| `multi_games.pgn` | Three games: fool's mate, one illegal game, one short game |
| `invalid.pgn` | An illegal move (error location reporting) |
| `chess960.pgn` | Chess960 start position with Shredder castling rights |
| `fake_maia.sh` | Stand-in for the Maia-2 sidecar (`tests/human_model.rs`) |
| `calibration-sample/` | 16 rated online games (the PGN as the site's public API returned it), read by `tests/coach_verify.rs` |
