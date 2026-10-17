//! Board-event detectors (fallback heuristics; see `docs/implementation-notes.md`).
//!
//! The reference engine derives Brilliant / Great Find / Miss facts from about
//! 250 tactical tags and machine-learned models that were not recovered. These
//! detectors use material changes along engine principal variations instead.
//! Their outputs feed the recovered decision tree in `review-core`.

use review_core::config::BrilliantConfig;
use shakmaty::{Chess, Color, Move, Position, Role, Square};

use super::see;
use crate::board;

/// Material profile of a line from the mover's point of view (cp).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineMaterial {
    /// Balance before the first move of the line.
    pub start: i32,
    /// Balance after the last inspected ply.
    pub end: i32,
    /// Lowest / highest balance after any inspected ply.
    pub min: i32,
    pub max: i32,
    /// Value captured by the first move of the line, if it is a capture.
    pub first_capture_value: Option<i32>,
    pub plies: u32,
}

impl LineMaterial {
    pub fn net(&self) -> i32 {
        self.end - self.start
    }

    pub fn lost(&self) -> i32 {
        (self.start - self.end).max(0)
    }
}

/// Extra plies allowed past the horizon to finish an exchange or check sequence.
const QUIESCENCE_EXTENSION: u32 = 6;

/// Plays `pv` (UCI, starting with the mover's move) from `start` for
/// `horizon` plies, extended while captures, promotions or checks continue,
/// and records the material balance for `mover`.
pub fn walk_line(start: &Chess, mover: Color, pv: &[String], horizon: u32) -> LineMaterial {
    let mut pos = start.clone();
    let initial = board::balance(pos.board(), mover);
    let mut walk = LineMaterial {
        start: initial,
        end: initial,
        min: initial,
        max: initial,
        first_capture_value: None,
        plies: 0,
    };
    let mut last_was_forcing = false;
    for (i, text) in pv.iter().enumerate() {
        // Stop at the horizon, but only at a quiet point (not mid-exchange or in check).
        let past_horizon = i >= horizon as usize;
        if (past_horizon && !last_was_forcing) || i >= (horizon + QUIESCENCE_EXTENSION) as usize {
            break;
        }
        let Some(m) = board::parse_uci(&pos, text) else {
            break;
        };
        last_was_forcing = m.is_capture() || m.is_promotion();
        if i == 0 {
            walk.first_capture_value = m.capture().map(board::role_value);
        }
        pos.play_unchecked(m);
        last_was_forcing = last_was_forcing || pos.is_check();
        let b = board::balance(pos.board(), mover);
        walk.end = b;
        walk.min = walk.min.min(b);
        walk.max = walk.max.max(b);
        walk.plies += 1;
    }
    walk
}

/// A piece the mover gives up with a move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sacrifice {
    pub square: Square,
    pub role: Role,
    /// Material the opponent wins by capturing it (static exchange, cp).
    pub see_gain: i32,
}

/// Static sacrifice test (functional spec §5.5, after the reference
/// `node_lib::is_sacrifice`). The moved piece (knight, bishop, rook or queen)
/// is given up when, on its destination square:
///
/// - the opponent wins at least `min_sacrifice_cp` by capturing it (SEE);
/// - it is not a disguised trade: the value the move captures plus the largest
///   value hit by a discovered attack is less than the piece's value;
/// - if it was already threatened before the move, it had a safe square, and
///   no other own piece and no opponent piece of at least its value was
///   threatened (a doomed piece or a mutual threat is not a sacrifice).
///
/// With `count_pieces_left_hanging`, a piece that the move leaves newly
/// en prise without moving it also counts (the trade test applies to it too).
/// The most valuable piece given up is returned.
pub fn static_sacrifice(before: &Chess, m: Move, cfg: &BrilliantConfig) -> Option<Sacrifice> {
    if m.is_castle() {
        return None;
    }
    let mover = before.turn();
    let opponent = mover.other();
    let mut after = before.clone();
    after.play_unchecked(m);
    let given_up = |role: Role| see::value(role) >= cfg.min_piece_value_cp && role != Role::King;
    let trade = m.capture().map_or(0, see::value) + discovered_attack_value(before, &after, m);

    let to = m.to();
    if let Some(role) = after.board().role_at(to).filter(|r| given_up(*r)) {
        let gain = see::capture_gain(&after, to);
        if gain >= cfg.min_sacrifice_cp
            && trade < see::value(role)
            && not_doomed_or_mutual(before, m, role)
        {
            return Some(Sacrifice {
                square: to,
                role,
                see_gain: gain,
            });
        }
    }
    if !cfg.count_pieces_left_hanging {
        return None;
    }
    let mut best: Option<Sacrifice> = None;
    for sq in after.board().by_color(mover).without(to) {
        let Some(role) = after.board().role_at(sq).filter(|r| given_up(*r)) else {
            continue;
        };
        let gain = see::capture_gain(&after, sq);
        let was_threatened = see::see(before.board(), sq, opponent) > 0;
        if gain >= cfg.min_sacrifice_cp
            && !was_threatened
            && trade < see::value(role)
            && best.is_none_or(|b| see::value(role) > see::value(b.role))
        {
            best = Some(Sacrifice {
                square: sq,
                role,
                see_gain: gain,
            });
        }
    }
    best
}

/// For a moved piece that was already threatened on its origin square: it had
/// a safe square to go to, and no other piece of at least its value (own or
/// the opponent's) was threatened.
fn not_doomed_or_mutual(before: &Chess, m: Move, role: Role) -> bool {
    let Some(from) = m.from() else {
        return true;
    };
    let mover = before.turn();
    let opponent = mover.other();
    let board = before.board();
    if see::see(board, from, opponent) <= 0 {
        return true;
    }
    let has_safe_square = before
        .legal_moves()
        .iter()
        .filter(|mv| mv.from() == Some(from))
        .any(|mv| {
            let mut p = before.clone();
            p.play_unchecked(*mv);
            see::capture_gain(&p, mv.to()) <= 0
        });
    if !has_safe_square {
        return false;
    }
    let value = see::value(role);
    let threatened = |sq: Square, by: Color| {
        board
            .role_at(sq)
            .is_some_and(|r| r != Role::King && see::value(r) >= value)
            && see::see(board, sq, by) > 0
    };
    let own_threatened = board
        .by_color(mover)
        .without(from)
        .into_iter()
        .any(|sq| threatened(sq, opponent));
    let theirs_threatened = board
        .by_color(opponent)
        .into_iter()
        .any(|sq| threatened(sq, mover));
    !own_threatened && !theirs_threatened
}

/// Largest value (king 99999) newly attacked by the mover's other sliders
/// because the move cleared their line.
fn discovered_attack_value(before: &Chess, after: &Chess, m: Move) -> i32 {
    let mover = before.turn();
    let board = after.board();
    let them = board.by_color(mover.other());
    let mut largest = 0;
    for sq in (board.by_color(mover) & board.sliders()).without(m.to()) {
        let new_targets = board.attacks_from(sq) & !before.board().attacks_from(sq) & them;
        for target in new_targets {
            if let Some(role) = board.role_at(target) {
                largest = largest.max(see::value(role));
            }
        }
    }
    largest
}

/// Recovered gate for Brilliant/Great Find: the mover is not already ahead in
/// material by at least the value of the piece involved.
pub fn material_gate(balance_after_move: i32, piece_value: i32) -> bool {
    balance_after_move < piece_value
}

/// The first move captures a piece that is not won back within the horizon.
pub fn takes_free_piece(line: &LineMaterial, min_value: i32) -> bool {
    match line.first_capture_value {
        Some(value) if value >= min_value => line.net() >= value - 100,
        _ => false,
    }
}

/// `played` recaptures on the square where the opponent just captured.
pub fn is_recapture(previous: Option<Move>, played: Move) -> bool {
    match previous {
        Some(prev) => prev.is_capture() && played.is_capture() && prev.to() == played.to(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::CastlingMode;
    use shakmaty::fen::Fen;

    fn pos(fen: &str) -> Chess {
        fen.parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap()
    }

    fn line(moves: &[&str]) -> Vec<String> {
        moves.iter().map(|m| (*m).to_owned()).collect()
    }

    fn sacrifice(fen: &str, uci: &str, cfg: &BrilliantConfig) -> Option<Sacrifice> {
        let p = pos(fen);
        let m = board::parse_uci(&p, uci).expect("legal move");
        static_sacrifice(&p, m, cfg)
    }

    #[test]
    fn bishop_sacrifice_on_f7_is_detected() {
        // Italian-style position: Bxf7+ Kxf7 gives a bishop for a pawn.
        let fen = "r1bqk2r/pppp1ppp/2n2n2/2b1p3/2B1P3/3P1N2/PPP2PPP/RNBQK2R w KQkq - 0 5";
        let walk = walk_line(&pos(fen), Color::White, &line(&["c4f7", "e8f7"]), 6);
        assert_eq!(walk.net(), -200);
        assert_eq!(
            sacrifice(fen, "c4f7", &BrilliantConfig::default()),
            Some(Sacrifice {
                square: Square::F7,
                role: Role::Bishop,
                see_gain: 300
            })
        );
    }

    #[test]
    fn quiet_moves_are_never_sacrifices() {
        let cfg = BrilliantConfig {
            count_pieces_left_hanging: true,
            ..Default::default()
        };
        // Be2, castling, and a king move.
        let fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 2";
        assert_eq!(sacrifice(fen, "f1e2", &cfg), None);
        let fen = "r1bqkbnr/pppp1ppp/2n5/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";
        assert_eq!(sacrifice(fen, "e1g1", &cfg), None);
        assert_eq!(
            sacrifice("8/8/4k3/8/8/8/5P1P/6K1 w - - 0 1", "g1g2", &cfg),
            None
        );
    }

    #[test]
    fn pawn_grab_and_later_trades_are_not_sacrifices() {
        // Nxe5 leaves nothing en prise, whatever the engine line does later
        // (Nxe5 d6 Nf3 Nxe4 dips nowhere; ...Bxc3 bxc3 lines trade evenly).
        let fen = "rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3";
        let walk = walk_line(
            &pos(fen),
            Color::White,
            &line(&["f3e5", "d7d6", "e5f3", "f6e4"]),
            6,
        );
        assert_eq!(walk.net(), 0);
        assert_eq!(sacrifice(fen, "f3e5", &BrilliantConfig::default()), None);
        // a3 attacks the bishop; the line a3 Bxc3+ bxc3 gives up nothing.
        let fen = "r1bqk2r/pppp1ppp/2n2n2/4p3/1b2P3/2N2N2/PPPP1PPP/R1BQKB1R w KQkq - 4 4";
        assert_eq!(sacrifice(fen, "a2a3", &BrilliantConfig::default()), None);
    }

    #[test]
    fn disguised_trade_is_not_a_sacrifice() {
        // Legal's trap, 5.Nxe5: the knight hangs, but it captures a pawn and uncovers
        // the queen's attack on g4 (100 + 300 >= 300). Counting pieces left hanging,
        // the queen on d1 is the piece given up.
        let fen = "rn1qkbnr/ppp2p1p/3p2p1/4p3/2B1P1b1/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 5";
        assert_eq!(sacrifice(fen, "f3e5", &BrilliantConfig::default()), None);
        let cfg = BrilliantConfig {
            count_pieces_left_hanging: true,
            ..Default::default()
        };
        assert_eq!(
            sacrifice(fen, "f3e5", &cfg),
            Some(Sacrifice {
                square: Square::D1,
                role: Role::Queen,
                see_gain: 600
            })
        );
    }

    #[test]
    fn doomed_piece_is_not_a_sacrifice() {
        // The bishop on a7 is lost wherever it goes, so Bxb6 gives nothing up.
        let fen = "r3k3/B1p5/1p6/8/8/8/8/4K3 w - - 0 1";
        assert_eq!(sacrifice(fen, "a7b6", &BrilliantConfig::default()), None);
    }

    #[test]
    fn opera_game_sacrifices() {
        let pgn = include_str!("../../../../../tests/fixtures/opera_game.pgn");
        let game = crate::pgn::parse_pgn(pgn).unwrap().remove(0).unwrap();
        let cfg = BrilliantConfig::default();
        let at = |ply: usize| {
            let parsed = &game.moves[ply - 1];
            (
                parsed.san.as_str(),
                static_sacrifice(&game.positions[ply - 1], parsed.mv, &cfg).map(|s| s.role),
            )
        };
        assert_eq!(at(19), ("Nxb5", Some(Role::Knight)));
        assert_eq!(at(21), ("Bxb5+", None));
        assert_eq!(at(25), ("Rxd7", Some(Role::Rook)));
        assert_eq!(at(29), ("Bxd7+", None), "rook for bishop is a trade");
        assert_eq!(at(31), ("Qb8+", Some(Role::Queen)));
    }

    #[test]
    fn walk_finishes_exchanges_past_the_horizon() {
        let p = pos("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3");
        // Horizon 1 would stop right after Nxe5 (+1); the capture extends the walk to d6.
        let walk = walk_line(&p, Color::White, &line(&["f3e5", "d7d6", "e5f3"]), 1);
        assert_eq!(walk.plies, 2);
        assert_eq!(walk.net(), 100);
    }

    #[test]
    fn free_piece_capture() {
        // Undefended queen on d4.
        let p = pos("4k3/8/8/8/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e8e7"]), 6);
        assert_eq!(walk.plies, 2);
        assert!(takes_free_piece(&walk, 300));

        // Defended by a pawn: Rxd4 exd4 wins material but the queen was not free.
        let p = pos("4k3/8/8/4p3/3q4/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d4", "e5d4"]), 6);
        assert_eq!(walk.net(), 400);
        assert!(!takes_free_piece(&walk, 300));
    }
}
