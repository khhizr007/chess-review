//! PGN import over the shared fixture corpus (`tests/fixtures`).

mod common;

use review_server::pgn::{PgnError, parse_pgn};
use shakmaty::{Color, Position};

#[test]
fn scholars_mate_parses_with_metadata() {
    let games = parse_pgn(&common::fixture("scholars_mate.pgn")).unwrap();
    let g = games[0].as_ref().unwrap();
    assert_eq!(g.header("White"), Some("Alice"));
    assert_eq!(g.elo(Color::Black), Some(1150));
    assert_eq!(g.moves.len(), 7);
    assert!(g.final_position().is_checkmate());
}

#[test]
fn special_moves_are_reconstructed() {
    let g = parse_pgn(&common::fixture("special_moves.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    let sans: Vec<&str> = g.moves.iter().map(|m| m.san.as_str()).collect();
    assert!(sans.contains(&"exf6"), "en passant: {sans:?}");
    assert!(g.moves.iter().any(|m| m.mv.is_en_passant()));
    assert!(sans.contains(&"gxh8=Q"), "promotion: {sans:?}");
    assert!(sans.contains(&"O-O"), "castling: {sans:?}");
    let castle = g.moves.iter().find(|m| m.san == "O-O").unwrap();
    assert_eq!(castle.uci, "e1g1");
}

#[test]
fn stalemate_game_ends_in_stalemate() {
    let g = parse_pgn(&common::fixture("stalemate.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    assert!(g.final_position().is_stalemate());
}

#[test]
fn fen_start_and_chess960_are_supported() {
    let g = parse_pgn(&common::fixture("back_rank.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    assert_eq!(g.start_fen(), "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1");
    let c = parse_pgn(&common::fixture("chess960.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    assert_eq!(c.castling_mode, shakmaty::CastlingMode::Chess960);
    assert_eq!(c.moves.len(), 4);
}

#[test]
fn sparse_metadata_is_tolerated() {
    let g = parse_pgn(&common::fixture("sparse_metadata.pgn"))
        .unwrap()
        .remove(0)
        .unwrap();
    assert_eq!(g.header("White"), None, "'?' is treated as missing");
    assert_eq!(g.header("Black"), None);
    assert_eq!(g.elo(Color::White), None);
    assert_eq!(g.moves.len(), 8);
}

#[test]
fn multi_game_file_keeps_good_games_and_reports_bad_ones() {
    let games = parse_pgn(&common::fixture("multi_games.pgn")).unwrap();
    assert_eq!(games.len(), 3);
    assert!(games[0].as_ref().unwrap().final_position().is_checkmate());
    assert!(matches!(
        games[1],
        Err(PgnError::IllegalMove {
            game: 2,
            ply: 3,
            ..
        })
    ));
    assert_eq!(games[2].as_ref().unwrap().moves.len(), 2);
}

#[test]
fn invalid_move_is_reported_with_location() {
    let err = parse_pgn(&common::fixture("invalid.pgn"))
        .unwrap()
        .remove(0)
        .unwrap_err();
    let text = err.to_string();
    assert!(text.contains("ply 7") && text.contains("Bxe8"), "{text}");
}
