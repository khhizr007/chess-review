//! Coach detectors on hand-built positions: one or more positions per tag.

use review_server::board;
use review_server::coach::detect::{MoveCtx, Tag, bad_tags, defence_tags, tactics};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Move, Position, Square};

fn pos(fen: &str) -> Chess {
    fen.parse::<Fen>()
        .unwrap()
        .into_position(CastlingMode::Standard)
        .unwrap()
}

/// Plays `uci` and then `rest` from `fen`; returns the start, the move, the
/// position after it and the parsed rest.
fn setup(fen: &str, uci: &str, rest: &[&str]) -> (Chess, Move, Chess, Vec<Move>) {
    let before = pos(fen);
    let m = board::parse_uci(&before, uci).unwrap_or_else(|| panic!("{uci} is legal"));
    let mut after = before.clone();
    after.play_unchecked(m);
    let mut p = after.clone();
    let mut moves = Vec::new();
    for u in rest {
        let mv = board::parse_uci(&p, u).unwrap_or_else(|| panic!("{u} is legal"));
        p.play_unchecked(mv);
        moves.push(mv);
    }
    (before, m, after, moves)
}

fn tactic_tags(fen: &str, uci: &str, rest: &[&str]) -> Vec<Tag> {
    let (before, m, after, rest) = setup(fen, uci, rest);
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &rest,
        first: true,
    };
    tactics(&ctx).into_iter().map(|f| f.tag).collect()
}

fn top_tactic(fen: &str, uci: &str) -> Option<Tag> {
    tactic_tags(fen, uci, &[]).first().copied()
}

fn bad(fen: &str, uci: &str, reply: &[&str], best: Option<&str>) -> Vec<Tag> {
    bad_with_previous(fen, uci, reply, best, None)
}

fn bad_with_previous(
    fen: &str,
    uci: &str,
    reply: &[&str],
    best: Option<&str>,
    previous: Option<Move>,
) -> Vec<Tag> {
    let (before, m, after, rest) = setup(fen, uci, reply);
    let best = best.map(|b| board::parse_uci(&before, b).expect("best move is legal"));
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous,
        rest: &rest,
        first: true,
    };
    bad_tags(&ctx, best).into_iter().map(|f| f.tag).collect()
}

// ---------------------------------------------------------------- mates

#[test]
fn checkmate() {
    // Scholar's mate.
    let fen = "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4";
    assert_eq!(top_tactic(fen, "h5f7"), Some(Tag::Checkmate));
}

#[test]
fn back_rank_mate() {
    let fen = "6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1";
    assert_eq!(top_tactic(fen, "a1a8"), Some(Tag::BackRankMate));
    // The same rook check with an escape square is no mate at all.
    let fen = "6k1/5p1p/8/8/8/8/8/R5K1 w - - 0 1";
    assert_eq!(top_tactic(fen, "a1a8"), None);
}

// ---------------------------------------------------------------- material

#[test]
fn free_piece() {
    assert_eq!(
        top_tactic("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", "d1d5"),
        Some(Tag::FreePiece)
    );
    // Defended by a pawn: the rook would be lost.
    assert!(
        !tactic_tags("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1", "d1d5", &[]).contains(&Tag::FreePiece)
    );
}

#[test]
fn free_piece_counts_only_on_the_first_move_of_a_line() {
    let (before, m, after, rest) = setup("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", "d1d5", &[]);
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &rest,
        first: false,
    };
    assert!(tactics(&ctx).iter().all(|f| f.tag != Tag::FreePiece));
}

#[test]
fn taking_back_is_not_a_free_piece() {
    // White has just played Rxd7 (a rook for the knight); ...Nxd7 takes back the
    // rook. That wins the exchange, not a hanging rook.
    let prior = pos("4k3/3n4/5n2/8/8/8/8/3RK3 w - - 0 1");
    let previous = board::parse_uci(&prior, "d1d7").unwrap();
    let mut before = prior.clone();
    before.play_unchecked(previous);
    let m = board::parse_uci(&before, "f6d7").unwrap();
    let mut after = before.clone();
    after.play_unchecked(m);
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: Some(previous),
        rest: &[],
        first: true,
    };
    assert!(tactics(&ctx).iter().all(|f| f.tag != Tag::FreePiece));
    // The same capture with no capture before it is a free rook.
    let ctx = MoveCtx {
        previous: None,
        ..ctx
    };
    assert_eq!(tactics(&ctx).first().map(|f| f.tag), Some(Tag::FreePiece));
}

#[test]
fn trapped_piece() {
    // Rb1 attacks the knight on a1; Nb3 runs into axb3 and Nc2 into Kxc2.
    let fen = "1R6/7k/8/8/8/8/P2K4/n7 w - - 0 1";
    assert_eq!(top_tactic(fen, "b8b1"), Some(Tag::TrappedPiece));
}

// ---------------------------------------------------------------- double attacks

#[test]
fn fork_and_royal_fork() {
    assert_eq!(
        top_tactic("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7"),
        Some(Tag::Fork)
    );
    assert_eq!(
        top_tactic("2q1k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5d6"),
        Some(Tag::RoyalFork)
    );
}

#[test]
fn fork_on_a_defended_square_is_not_a_fork() {
    // The bishop on b6 covers c7 and takes the knight.
    assert!(!tactic_tags("r3k3/8/1b6/1N6/8/8/8/4K3 w - - 0 1", "b5c7", &[]).contains(&Tag::Fork));
}

#[test]
fn double_check() {
    let tags = tactic_tags("4k3/8/8/8/4N3/8/8/4RK2 w - - 0 1", "e4d6", &[]);
    assert_eq!(tags.first(), Some(&Tag::DoubleCheck));
    assert!(tags.contains(&Tag::DiscoveredCheck));
}

#[test]
fn discovered_check() {
    assert_eq!(
        top_tactic("4k3/8/8/8/4N3/8/8/4RK2 w - - 0 1", "e4c5"),
        Some(Tag::DiscoveredCheck)
    );
}

#[test]
fn discovered_attack() {
    // The knight steps away and the rook hits the queen.
    assert_eq!(
        top_tactic("3qk3/8/8/8/3N4/8/8/3RK3 w - - 0 1", "d4b3"),
        Some(Tag::DiscoveredAttack)
    );
}

// ---------------------------------------------------------------- line tactics

#[test]
fn pin() {
    // Bg5 pins the knight to the queen.
    assert_eq!(
        top_tactic("3qk3/8/5n2/8/8/8/8/2B1K3 w - - 0 1", "c1g5"),
        Some(Tag::Pin)
    );
}

#[test]
fn exploits_pin() {
    // The pinned knight on f6 cannot step away from e5.
    assert_eq!(
        top_tactic("3qk3/8/5n2/6B1/4P3/8/8/4K3 w - - 0 1", "e4e5"),
        Some(Tag::ExploitsPin)
    );
}

#[test]
fn capture_with_a_pinned_defender_exploits_the_pin() {
    // The knight on c6 guards e5 but is pinned to the king by Bb5: Rxe5 wins a pawn.
    let fen = "4k3/8/2n5/1B2p3/8/8/8/4R1K1 w - - 0 1";
    assert_eq!(top_tactic(fen, "e1e5"), Some(Tag::ExploitsPin));
}

#[test]
fn skewer() {
    // Re1+ skewers the king and the queen behind it.
    assert_eq!(
        top_tactic("4q3/8/4k3/8/8/8/8/R5K1 w - - 0 1", "a1e1"),
        Some(Tag::Skewer)
    );
}

// ---------------------------------------------------------------- defender tactics

#[test]
fn removes_defender() {
    // Bxc6 removes the knight that guarded the bishop on e5.
    let fen = "6k1/1p6/2n5/1B2b3/8/8/8/4R1K1 w - - 0 1";
    assert_eq!(top_tactic(fen, "b5c6"), Some(Tag::RemovesDefender));
}

#[test]
fn overloaded() {
    // The queen guards both d5 and h4: after Rxd5 Qxd5, Nxh4.
    let fen = "3q2k1/8/8/3n4/7b/5N2/8/3R2K1 w - - 0 1";
    assert_eq!(top_tactic(fen, "d1d5"), Some(Tag::Overloaded));
}

#[test]
fn attraction() {
    // Rf8+ Kxf8 Nd7+ forks the king and the queen.
    let fen = "1q4k1/8/8/4N3/8/8/8/5RK1 w - - 0 1";
    let tags = tactic_tags(fen, "f1f8", &["g8f8", "e5d7", "f8e7", "d7b8"]);
    assert_eq!(tags.first(), Some(&Tag::Attraction));
    // Without the follow-up it is just a lost rook.
    assert!(!tactic_tags(fen, "f1f8", &["g8f8"]).contains(&Tag::Attraction));
}

#[test]
fn promotion() {
    assert_eq!(
        top_tactic("8/4P1k1/8/8/8/8/8/4K3 w - - 0 1", "e7e8q"),
        Some(Tag::Promotion)
    );
    // Promoting where the king takes the new queen is no gain.
    assert_eq!(top_tactic("8/3kP3/8/8/8/8/8/4K3 w - - 0 1", "e7e8q"), None);
}

// ---------------------------------------------------------------- bad-move bases

#[test]
fn hangs_piece_and_pawn() {
    assert_eq!(
        bad("4k3/8/5n2/8/8/8/8/3RK3 b - - 0 1", "f6d5", &["d1d5"], None),
        vec![Tag::HangsPiece]
    );
    assert_eq!(
        bad("4k3/1p6/8/8/8/3B4/8/4K3 b - - 0 1", "b7b5", &["d3b5"], None),
        vec![Tag::HangsPawn]
    );
    // When the opponent's best line does not take it, it is not called hanging.
    assert!(bad("4k3/8/5n2/8/8/8/8/3RK3 b - - 0 1", "f6d5", &["e1e2"], None).is_empty());
}

#[test]
fn ignores_threat() {
    assert_eq!(
        bad("4k3/8/8/3n4/8/8/8/3RK3 b - - 0 1", "e8f8", &["d1d5"], None),
        vec![Tag::IgnoresThreat]
    );
}

#[test]
fn removes_own_defender() {
    // The knight stops guarding the bishop on e5.
    assert_eq!(
        bad(
            "4k3/8/2n5/4b3/8/8/8/4R1K1 b - - 0 1",
            "c6b4",
            &["e1e5"],
            None
        ),
        vec![Tag::RemovesOwnDefender]
    );
}

#[test]
fn bad_trade() {
    // Rxd5 exd5: a rook for a pawn.
    assert_eq!(
        bad(
            "4k3/8/4p3/3p4/8/8/8/3RK3 w - - 0 1",
            "d1d5",
            &["e6d5"],
            None
        ),
        vec![Tag::BadTrade]
    );
}

#[test]
fn moves_pinned_piece() {
    // The knight leaves the pin and Bxd8 wins the queen.
    let tags = bad(
        "3qk3/8/5n2/6B1/8/8/8/4K3 b - - 0 1",
        "f6h5",
        &["g5d8"],
        None,
    );
    assert_eq!(tags.first(), Some(&Tag::MovesPinnedPiece));
}

#[test]
fn did_not_recapture() {
    // White has just taken on f6 with the bishop; ...gxf6 was the move.
    let prior = pos("4k3/6p1/5n2/6B1/8/8/8/4K3 w - - 0 1");
    let previous = board::parse_uci(&prior, "g5f6").unwrap();
    let mut before = prior.clone();
    before.play_unchecked(previous);
    let fen = board::fen(&before);
    let tags = bad_with_previous(&fen, "e8d7", &["f6d4"], Some("g7f6"), Some(previous));
    assert_eq!(tags, vec![Tag::DidNotRecapture]);
    // Without knowing the best move recaptures, nothing is claimed.
    assert!(bad_with_previous(&fen, "e8d7", &["f6d4"], None, Some(previous)).is_empty());
}

#[test]
fn quiet_safe_moves_have_no_bad_tags() {
    assert!(
        bad(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
            "e7e5",
            &["g1f3"],
            None
        )
        .is_empty()
    );
}

#[test]
fn geometry_points_at_the_targets() {
    use review_server::coach::geometry::Geometry;
    let (before, m, after, rest) = setup("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7", &[]);
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &rest,
        first: true,
    };
    let fork = tactics(&ctx).remove(0);
    assert_eq!(
        fork.geometry,
        Geometry::Fan {
            source: Square::C7,
            targets: vec![Square::E8, Square::A8]
        }
    );
    assert_eq!(fork.stake, 500);
}

// ---------------------------------------------------------------- defence

/// Defence tags of `uci`. `threat` is the opponent's null-move line before the
/// move (played from `fen` with the other side to move), when known.
fn defence(fen: &str, uci: &str, threat: Option<&[&str]>) -> Vec<Tag> {
    let (before, m, after, _) = setup(fen, uci, &[]);
    let threat: Option<Vec<Move>> = threat.map(|line| {
        let mut p = board::null_move(&before).expect("the opponent can move");
        line.iter()
            .map(|u| {
                let mv = board::parse_uci(&p, u).unwrap_or_else(|| panic!("{u} is legal"));
                p.play_unchecked(mv);
                mv
            })
            .collect()
    });
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &[],
        first: true,
    };
    defence_tags(&ctx, threat.as_deref())
        .into_iter()
        .map(|f| f.tag)
        .collect()
}

// The rook on d8 attacks the undefended knight on d4.
const KNIGHT_ATTACKED: &str = "3rk3/8/8/8/3N4/8/2P5/4K3 w - - 0 1";

#[test]
fn defends_piece() {
    // c3 guards the knight: Rxd4 cxd4 loses the exchange.
    assert_eq!(
        defence(KNIGHT_ATTACKED, "c2c3", None),
        vec![Tag::DefendsPiece]
    );
    // Confirmed by the opponent's threat line...
    assert_eq!(
        defence(KNIGHT_ATTACKED, "c2c3", Some(&["d8d4"])),
        vec![Tag::DefendsPiece]
    );
    // ...and not claimed when that line shows no interest in the knight.
    assert!(defence(KNIGHT_ATTACKED, "c2c3", Some(&["e8e7"])).is_empty());
    // A pawn move that does not guard it defends nothing.
    assert!(defence(KNIGHT_ATTACKED, "c2c4", None).is_empty());
}

#[test]
fn defends_piece_by_blocking() {
    // The bishop steps between the rook on d8 and the knight on d4, where the
    // e4 pawn guards it.
    let fen = "3rk3/8/8/8/3NP3/1B6/8/4K3 w - - 0 1";
    assert_eq!(defence(fen, "b3d5", None), vec![Tag::DefendsPiece]);
    // Behind the knight, the bishop neither guards nor blocks.
    let fen = "3rk3/8/8/8/3N4/8/8/4KB2 w - - 0 1";
    assert!(!defence(fen, "f1d3", None).contains(&Tag::DefendsPiece));
}

#[test]
fn escapes_threat() {
    assert_eq!(
        defence("3rk3/8/8/8/3N4/8/8/4K3 w - - 0 1", "d4b5", None),
        vec![Tag::EscapesThreat]
    );
    // Away from the rook on a4 but into the b7 pawn's reach, it is still lost.
    let fen = "4k3/1p6/8/8/r2N4/8/8/4K3 w - - 0 1";
    assert!(defence(fen, "d4c6", None).is_empty());
    assert_eq!(defence(fen, "d4f5", None), vec![Tag::EscapesThreat]);
    // A capture is not called an escape.
    assert!(defence("3rk3/8/8/5p2/3N4/8/8/4K3 w - - 0 1", "d4f5", None).is_empty());
}

#[test]
fn blocks_check() {
    // The rook on a1 checks along the first rank; the bishop steps in on c1.
    let fen = "4k3/8/8/8/8/8/3B4/r3K3 w - - 0 1";
    assert_eq!(defence(fen, "d2c1", None), vec![Tag::BlocksCheck]);
    // Stepping out of check is no block.
    assert!(defence(fen, "e1e2", None).is_empty());
}

#[test]
fn counter_threat() {
    // The knight on d4 stays attacked; Bd3 hits the undefended queen on h7.
    let fen = "3rk3/7q/8/8/3N4/8/8/4KB2 w - - 0 1";
    let (before, m, after, _) = setup(fen, "f1d3", &[]);
    let ctx = MoveCtx {
        before: &before,
        m,
        after: &after,
        previous: None,
        rest: &[],
        first: true,
    };
    let found = defence_tags(&ctx, None);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].tag, Tag::CounterThreat);
    assert_eq!(found[0].focus.map(|p| p.square), Some(Square::H7));
    assert_eq!(found[0].targets[0].square, Square::D4);
    // A threat smaller than what is left hanging is no answer.
    let fen = "3rk3/7p/8/8/3N4/8/8/4KB2 w - - 0 1";
    assert!(defence(fen, "f1d3", None).is_empty());
}
