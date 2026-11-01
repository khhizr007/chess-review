//! Positional coach detectors (groups 13 and 14) on hand-built positions:
//! development, castling, the centre, activity and pawn structure.

use review_core::Phase;
use review_server::board;
use review_server::coach::detect::{MoveCtx, Tag, positional_errors, positional_tags};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Move, Position};

fn pos(fen: &str) -> Chess {
    fen.parse::<Fen>()
        .unwrap()
        .into_position(CastlingMode::Standard)
        .unwrap()
}

/// Runs `detect` on `uci` from `fen`, with `rest` as the line after it.
fn run(fen: &str, uci: &str, rest: &[&str], detect: impl Fn(&MoveCtx<'_>) -> Vec<Tag>) -> Vec<Tag> {
    let before = pos(fen);
    let m = board::parse_uci(&before, uci).unwrap_or_else(|| panic!("{uci} is legal"));
    let mut after = before.clone();
    after.play_unchecked(m);
    let mut p = after.clone();
    let mut moves: Vec<Move> = Vec::new();
    for u in rest {
        let mv = board::parse_uci(&p, u).unwrap_or_else(|| panic!("{u} is legal"));
        p.play_unchecked(mv);
        moves.push(mv);
    }
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &moves,
        first: true,
    };
    detect(&ctx)
}

fn ideas(fen: &str, uci: &str, phase: Phase) -> Vec<Tag> {
    ideas_with(fen, uci, &[], phase)
}

fn ideas_with(fen: &str, uci: &str, rest: &[&str], phase: Phase) -> Vec<Tag> {
    run(fen, uci, rest, |ctx| {
        positional_tags(ctx, phase)
            .into_iter()
            .map(|f| f.tag)
            .collect()
    })
}

fn errors(fen: &str, uci: &str, phase: Phase) -> Vec<Tag> {
    run(fen, uci, &[], |ctx| {
        positional_errors(ctx, phase)
            .into_iter()
            .map(|f| f.tag)
            .collect()
    })
}

fn first(tags: &[Tag]) -> Option<Tag> {
    tags.first().copied()
}

const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const AFTER_E4: &str = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
const AFTER_E4_E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";

// ---------------------------------------------------------------- development

#[test]
fn develops_toward_the_centre_in_the_opening() {
    // Nf3 eyes d4 and e5.
    assert_eq!(
        first(&ideas(AFTER_E4_E5, "g1f3", Phase::Opening)),
        Some(Tag::DevelopsCenter)
    );
}

#[test]
fn develops_away_from_the_centre_in_the_opening() {
    // Bg5 pins the knight but reaches no centre square.
    let fen = "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/3P4/PPP2PPP/RNBQKBNR w KQkq - 1 3";
    assert_eq!(
        first(&ideas(fen, "c1g5", Phase::Opening)),
        Some(Tag::DevelopsOpening)
    );
}

#[test]
fn develops_a_rook_in_the_middlegame() {
    let fen = "r2q1rk1/ppp2ppp/2n2n2/4p3/8/2NP1N2/PPP2PPP/R4RK1 w - - 0 10";
    assert_eq!(
        first(&ideas(fen, "a1d1", Phase::Middlegame)),
        Some(Tag::DevelopsMiddlegame)
    );
    // A piece that has already moved does not develop.
    assert!(!ideas(fen, "f3d2", Phase::Middlegame).contains(&Tag::DevelopsMiddlegame));
}

#[test]
fn clearing_the_way_to_castle_comes_before_developing() {
    // Bc4 also develops toward d5, but what it is for is O-O.
    let fen = "rnbqk2r/pppp1ppp/5n2/2b1p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 4 4";
    let tags = ideas(fen, "f1c4", Phase::Opening);
    assert_eq!(first(&tags), Some(Tag::PreparesToCastleKingside));
    assert!(tags.contains(&Tag::DevelopsCenter));
    // Without the right to castle there is nothing to prepare.
    let fen = "rnbqk2r/pppp1ppp/5n2/2b1p3/4P3/5N2/PPPP1PPP/RNBQKB1R w Qkq - 4 4";
    assert_eq!(
        first(&ideas(fen, "f1c4", Phase::Opening)),
        Some(Tag::DevelopsCenter)
    );
}

#[test]
fn prepares_to_castle_queenside() {
    let fen = "rn2kbnr/ppp1pppp/8/3q4/3P2b1/2N5/PPP1QPPP/R1B1KBNR w KQkq - 0 1";
    assert!(ideas(fen, "c1e3", Phase::Opening).contains(&Tag::PreparesToCastleQueenside));
}

#[test]
fn castling_by_where_the_other_king_is() {
    let fen = "rnbqk2r/pppp1ppp/5n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
    assert_eq!(
        first(&ideas(fen, "e1g1", Phase::Opening)),
        Some(Tag::CastlesKingside)
    );
    let fen = "rnbq1rk1/pppp1ppp/5n2/2b1p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQ - 6 5";
    assert_eq!(
        first(&ideas(fen, "e1g1", Phase::Opening)),
        Some(Tag::CastlesSameSide)
    );
    let fen = "rn1q1rk1/pbpp1ppp/1p2pn2/8/3P4/2NQB3/PPP2PPP/R3KBNR w KQ - 0 7";
    assert_eq!(
        first(&ideas(fen, "e1c1", Phase::Opening)),
        Some(Tag::CastlesOppositeSide)
    );
}

#[test]
fn connects_rooks() {
    // The queen was the last piece between the rooks.
    let fen = "4k3/8/8/8/8/8/5PPP/R2Q1RK1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "d1d3", Phase::Middlegame)),
        Some(Tag::ConnectsRooks)
    );
}

#[test]
fn fianchetto() {
    let fen = "rnbqkbnr/pppppp1p/6p1/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 2";
    assert_eq!(
        first(&ideas(fen, "f8g7", Phase::Opening)),
        Some(Tag::Fianchetto)
    );
}

#[test]
fn prepares_bishop_development() {
    // ...e6 frees the bishop on f8.
    let fen = "rnbqkb1r/pppppppp/5n2/8/3P4/5N2/PPP1PPPP/RNBQKB1R b KQkq - 2 2";
    assert_eq!(
        first(&ideas(fen, "e7e6", Phase::Opening)),
        Some(Tag::PreparesBishopDevelopment)
    );
    // ...a6 frees nothing.
    assert!(ideas(fen, "a7a6", Phase::Opening).is_empty());
}

// ---------------------------------------------------------------- centre

#[test]
fn puts_a_pawn_in_the_centre() {
    assert_eq!(
        first(&ideas(START, "e2e4", Phase::Opening)),
        Some(Tag::PutsPawnInCenter)
    );
    // Only in the opening.
    assert!(!ideas(START, "e2e4", Phase::Middlegame).contains(&Tag::PutsPawnInCenter));
}

#[test]
fn strikes_at_the_centre() {
    // ...d5 hits the pawn on e4.
    assert_eq!(
        first(&ideas(AFTER_E4, "d7d5", Phase::Opening)),
        Some(Tag::StrikesAtCenter)
    );
}

#[test]
fn central_break_against_a_locked_pawn() {
    // French Advance: ...c5 hits d4, a centre pawn: a strike at the centre.
    let fen = "rnbqkbnr/ppp2ppp/4p3/3pP3/3P4/8/PPP2PPP/RNBQKBNR b KQkq - 0 3";
    assert_eq!(
        first(&ideas(fen, "c7c5", Phase::Opening)),
        Some(Tag::StrikesAtCenter)
    );
    // Later, ...e6 hits the pawn on f5, which our pawn on f6 locks: a break.
    let fen = "4k3/4p3/5p2/5P2/8/8/8/4K3 b - - 0 1";
    assert_eq!(
        first(&ideas(fen, "e7e6", Phase::Middlegame)),
        Some(Tag::CentralBreak)
    );
    assert_ne!(
        first(&ideas(fen, "e7e6", Phase::Opening)),
        Some(Tag::CentralBreak)
    );
    // 3.d4 against e5 (Philidor): a strike, though e4 and e5 face each other.
    let fen = "rnbqkbnr/ppp2ppp/3p4/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 3";
    assert_eq!(
        first(&ideas(fen, "d2d4", Phase::Opening)),
        Some(Tag::StrikesAtCenter)
    );
}

#[test]
fn attacks_the_pawn_chain() {
    // ...b5 hits c4, which d3 guards.
    let fen = "4k3/1p6/8/8/2P5/3P4/8/4K3 b - - 0 1";
    assert_eq!(
        first(&ideas(fen, "b7b5", Phase::Middlegame)),
        Some(Tag::AttacksPawnChain)
    );
}

#[test]
fn prepares_a_central_push() {
    // ...c6 guards d5 for the d-pawn.
    let fen = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq - 0 1";
    assert_eq!(
        first(&ideas(fen, "c7c6", Phase::Opening)),
        Some(Tag::PreparesCentralPush)
    );
}

#[test]
fn creates_luft() {
    let fen = "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "h2h3", Phase::Middlegame)),
        Some(Tag::CreatesLuft)
    );
}

// ---------------------------------------------------------------- activity

#[test]
fn rook_on_the_seventh() {
    let fen = "6k1/pp3ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "d1d7", Phase::Middlegame)),
        Some(Tag::RookOnSeventh)
    );
}

#[test]
fn outpost() {
    // Nd5, guarded by e4; no c- or e-pawn of Black can ever chase it.
    let fen = "4k3/pp3ppp/3p4/8/4P3/2N5/PP3PPP/4K3 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "c3d5", Phase::Middlegame)),
        Some(Tag::Outpost)
    );
    // With a black c-pawn on c7, ...c6 would chase it.
    let fen = "4k3/ppp2ppp/3p4/8/4P3/2N5/PP3PPP/4K3 w - - 0 1";
    assert!(!ideas(fen, "c3d5", Phase::Middlegame).contains(&Tag::Outpost));
}

#[test]
fn takes_the_open_file() {
    let fen = "6k1/pp3ppp/8/8/8/8/PP3PPP/5RK1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "f1d1", Phase::Middlegame)),
        Some(Tag::TakesOpenFile)
    );
    // The c-file is not open.
    let fen = "6k1/ppp2ppp/8/8/8/8/PPP2PPP/5RK1 w - - 0 1";
    assert!(!ideas(fen, "f1c1", Phase::Middlegame).contains(&Tag::TakesOpenFile));
}

#[test]
fn battery() {
    // Qc2 behind the bishop on d3, aimed at h7.
    let fen = "6k1/5ppp/8/8/8/3B4/5PPP/3Q2K1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "d1c2", Phase::Middlegame)),
        Some(Tag::Battery)
    );
}

#[test]
fn centralizes_a_knight() {
    let fen = "4k3/8/8/8/8/7N/8/4K3 w - - 0 1";
    let tags = ideas(fen, "h3f4", Phase::Middlegame);
    assert_eq!(first(&tags), Some(Tag::CentralizesKnight));
    assert!(tags.contains(&Tag::MovesToActiveSquare));
}

#[test]
fn bishop_gains_scope() {
    // From b1, hemmed in by c2, to the open a2-g8 diagonal.
    let fen = "4k3/8/8/8/8/8/2P5/1B2K3 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "b1a2", Phase::Middlegame)),
        Some(Tag::BishopGainsScope)
    );
}

#[test]
fn king_activity_in_the_endgame() {
    let fen = "8/8/8/8/8/8/k7/6K1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "g1f2", Phase::Endgame)),
        Some(Tag::KingActivity)
    );
    assert!(ideas(fen, "g1f2", Phase::Middlegame).is_empty());
}

// ---------------------------------------------------------------- structure

#[test]
fn pushes_a_passed_pawn() {
    let fen = "8/8/8/8/P7/8/k7/6K1 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "a4a5", Phase::Endgame)),
        Some(Tag::PushesPassedPawnEndgame)
    );
    assert_eq!(
        first(&ideas(fen, "a4a5", Phase::Middlegame)),
        Some(Tag::PushesPassedPawn)
    );
}

#[test]
fn makes_a_passed_pawn() {
    // bxc5 removes the only pawn in the way.
    let fen = "4k3/8/8/2p5/1P6/8/8/4K3 w - - 0 1";
    assert_eq!(
        first(&ideas(fen, "b4c5", Phase::Endgame)),
        Some(Tag::MakesPassedPawn)
    );
}

#[test]
fn forces_doubled_pawns() {
    // Bxc6 bxc6 doubles Black's c-pawns.
    let fen = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
    let tags = ideas_with(fen, "b5c6", &["b7c6"], Phase::Opening);
    assert_eq!(first(&tags), Some(Tag::ForcesDoubledPawns));
    // Taken back with the queen: no weakness.
    let fen = "r1b1kbnr/ppppqppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
    let tags = ideas_with(fen, "b5c6", &["e7c5"], Phase::Opening);
    assert!(!tags.iter().any(|t| t.forces_weakness()));
}

#[test]
fn forces_an_isolated_pawn() {
    // Bxc6 bxc6 leaves Black with isolated pawns on a7 and c6.
    let fen = "4k3/pp6/2n5/1B6/8/8/8/4K3 w - - 0 1";
    assert_eq!(
        first(&ideas_with(fen, "b5c6", &["b7c6"], Phase::Middlegame)),
        Some(Tag::ForcesIsolatedPawn)
    );
}

// ---------------------------------------------------------------- errors

#[test]
fn too_early_queen() {
    assert_eq!(
        first(&errors(AFTER_E4_E5, "d1h5", Phase::Opening)),
        Some(Tag::TooEarlyQueen)
    );
    assert!(errors(AFTER_E4_E5, "d1h5", Phase::Middlegame).is_empty());
}

#[test]
fn moves_the_same_piece_twice() {
    let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3";
    assert_eq!(
        first(&errors(fen, "f3g5", Phase::Opening)),
        Some(Tag::MovesSamePiece)
    );
    // Developing another piece is no error.
    assert!(errors(fen, "f1c4", Phase::Opening).is_empty());
}

#[test]
fn weakens_the_king() {
    assert_eq!(
        first(&errors(AFTER_E4, "f7f6", Phase::Opening)),
        Some(Tag::WeakensKing)
    );
}

#[test]
fn moves_a_pawn_in_front_of_the_king() {
    let fen = "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";
    assert_eq!(
        first(&errors(fen, "g2g4", Phase::Middlegame)),
        Some(Tag::MovesPawnInFrontOfKing)
    );
    // A pawn far from the king is not part of its shelter.
    let fen = "6k1/5ppp/8/8/8/8/P4PPP/3R2K1 w - - 0 1";
    assert!(errors(fen, "a2a4", Phase::Middlegame).is_empty());
}

#[test]
fn loses_castling() {
    let fen = "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1";
    assert_eq!(
        first(&errors(fen, "e1e2", Phase::Middlegame)),
        Some(Tag::LosesCastling)
    );
    assert_eq!(
        first(&errors(fen, "h1h4", Phase::Middlegame)),
        Some(Tag::LosesCastling)
    );
}

#[test]
fn doubles_and_isolates_its_own_pawns() {
    // bxc3 doubles the c-pawns (and isolates them).
    let fen = "4k3/8/8/8/8/2n5/1PP5/4K3 w - - 0 1";
    let tags = errors(fen, "b2c3", Phase::Middlegame);
    assert_eq!(first(&tags), Some(Tag::DoubledPawns));
    assert!(tags.contains(&Tag::IsolatedPawn));
    // bxc3 with a pawn on a2: both pawns are isolated, none doubled.
    let fen = "4k3/8/8/8/8/2n5/PP6/4K3 w - - 0 1";
    assert_eq!(
        first(&errors(fen, "b2c3", Phase::Middlegame)),
        Some(Tag::IsolatedPawn)
    );
}

#[test]
fn leaves_a_backward_pawn() {
    // After c4, the d3 pawn has no support left and e5 guards d4.
    let fen = "4k3/8/8/4p3/8/2PP4/8/4K3 w - - 0 1";
    assert_eq!(
        first(&errors(fen, "c3c4", Phase::Middlegame)),
        Some(Tag::BackwardPawn)
    );
}

#[test]
fn moves_to_a_passive_square() {
    let fen = "4k3/8/8/8/8/2N5/8/4K3 w - - 0 1";
    assert_eq!(
        first(&errors(fen, "c3a2", Phase::Middlegame)),
        Some(Tag::MovesToPassiveSquare)
    );
}

#[test]
fn every_new_tag_is_in_its_group() {
    for tag in Tag::ALL {
        assert!(
            !(tag.is_positional() && tag.is_positional_error()),
            "{tag:?} in both groups"
        );
        if tag.is_development() || tag.forces_weakness() {
            assert!(tag.is_positional(), "{tag:?}");
        }
    }
}
