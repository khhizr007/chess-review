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
    /// When the line ends up ahead (`end > start`): the ply (0-based) whose
    /// move wins that material. Only settled points count (a ply whose next
    /// move is not a capture, so an exchange in progress is not mistaken for
    /// a gain or a loss): it is the first settled ply from which the balance
    /// stays at or above `end`. The tactic behind the gain is at or before it.
    pub gain_ply: Option<u32>,
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
        gain_ply: None,
    };
    // (balance after the ply, whether the ply's move captured)
    let mut balances: Vec<(i32, bool)> = Vec::new();
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
        let captures = m.is_capture();
        pos.play_unchecked(m);
        last_was_forcing = last_was_forcing || pos.is_check();
        let b = board::balance(pos.board(), mover);
        walk.end = b;
        walk.min = walk.min.min(b);
        walk.max = walk.max.max(b);
        walk.plies += 1;
        balances.push((b, captures));
    }
    if walk.end > walk.start {
        let end = walk.end;
        let settled: Vec<(usize, i32)> = (0..balances.len())
            .filter(|&k| {
                balances
                    .get(k + 1)
                    .is_none_or(|(_, next_captures)| !next_captures)
            })
            .map(|k| (k, balances[k].0))
            .collect();
        let first_kept = settled
            .iter()
            .rposition(|(_, b)| *b < end)
            .map_or(0, |last_below| last_below + 1);
        walk.gain_ply = settled.get(first_kept).map(|(k, _)| *k as u32);
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
/// `node_lib::is_sacrifice`). The moved piece (knight, bishop, rook or queen,
/// or a pawn when `pawns` is set) is given up when, on its destination square:
///
/// - the opponent wins material by capturing it (SEE), and that gain minus
///   what the move captured is at least `min_sacrifice_cp` (so an even trade
///   is not a sacrifice), and the gain is at least `min_exchange_gain_cp` or
///   the piece's whole value, whichever is smaller;
/// - if it was already threatened before the move, it had a safe square, and
///   no other own piece and no opponent piece of at least its value was
///   threatened (a doomed piece or a mutual threat is not a sacrifice).
///
/// With `count_pieces_left_hanging`, a knight, bishop, rook or queen that the
/// move leaves newly en prise without moving it also counts, under the same
/// net-material test (this includes a piece left behind by castling).
/// Promotions are never sacrifices. The most valuable piece given up is
/// returned.
pub fn static_sacrifice(
    before: &Chess,
    m: Move,
    cfg: &BrilliantConfig,
    pawns: bool,
) -> Option<Sacrifice> {
    if m.is_promotion() {
        return None;
    }
    let mover = before.turn();
    let opponent = mover.other();
    let mut after = before.clone();
    after.play_unchecked(m);
    let given_up = |role: Role| see::value(role) >= cfg.min_piece_value_cp && role != Role::King;
    let captured = m.capture().map_or(0, see::value);
    let gives_up = |gain: i32, role: Role| {
        gain - captured >= cfg.min_sacrifice_cp
            && gain >= cfg.min_exchange_gain_cp.min(see::value(role))
            && gain > 0
    };

    let to = m.to();
    let moved = (!m.is_castle())
        .then(|| after.board().role_at(to))
        .flatten()
        .filter(|r| given_up(*r) || (pawns && *r == Role::Pawn));
    let mut best = moved.and_then(|role| {
        let gain = see::capture_gain(&after, to);
        (gives_up(gain, role) && not_doomed_or_mutual(before, m, role)).then_some(Sacrifice {
            square: to,
            role,
            see_gain: gain,
        })
    });
    if !cfg.count_pieces_left_hanging {
        return best;
    }
    for sq in after.board().by_color(mover) {
        if sq == to && !m.is_castle() {
            continue;
        }
        let Some(role) = after.board().role_at(sq).filter(|r| given_up(*r)) else {
            continue;
        };
        let gain = see::capture_gain(&after, sq);
        let was_threatened = before.board().role_at(sq) == Some(role)
            && before.board().color_at(sq) == Some(mover)
            && see::see(before.board(), sq, opponent) > 0;
        if gives_up(gain, role)
            && !was_threatened
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
    if safe_squares(before, from).is_empty() {
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

/// Legal destinations of the piece on `from` where the opponent cannot win it
/// by exchange.
fn safe_squares(before: &Chess, from: Square) -> Vec<Square> {
    before
        .legal_moves()
        .iter()
        .filter(|mv| mv.from() == Some(from))
        .filter(|mv| {
            let mut p = before.clone();
            p.play_unchecked(**mv);
            see::capture_gain(&p, mv.to()) <= 0
        })
        .map(|mv| mv.to())
        .collect()
}

/// Great exclusion (`is_easy_to_find`): a threatened piece (not a pawn) goes to the only square
/// where the opponent cannot win it by exchange. The reference does not say "threatened", but on
/// the calibration games the rule without it (or with pawns) removed more Great moves than wrong
/// ones.
pub fn is_only_safe_square(before: &Chess, m: Move) -> bool {
    match m.from() {
        Some(from) if !m.is_castle() && m.role() != Role::Pawn => {
            see::see(before.board(), from, before.turn().other()) > 0
                && safe_squares(before, from) == [m.to()]
        }
        _ => false,
    }
}

/// The moved piece cannot be won by exchange on its new square (`is_safe`).
pub fn lands_safely(before: &Chess, m: Move) -> bool {
    if m.is_castle() {
        return true;
    }
    let mut after = before.clone();
    after.play_unchecked(m);
    see::capture_gain(&after, m.to()) <= 0
}

/// The move captures the piece that was attacking the capturing piece (`removes_attacker_to_self`).
pub fn removes_attacker(before: &Chess, m: Move) -> bool {
    match m.from() {
        Some(from) if m.is_capture() && !m.is_castle() && !m.is_en_passant() => {
            before.board().attacks_from(m.to()).contains(from)
        }
        _ => false,
    }
}

/// The move captures a piece worth at least `min_value` that the opponent
/// cannot win back by exchange.
pub fn captures_free_piece(before: &Chess, m: Move, min_value: i32) -> bool {
    let Some(captured) = m.capture().map(board::role_value) else {
        return false;
    };
    let mut after = before.clone();
    after.play_unchecked(m);
    captured >= min_value && see::capture_gain(&after, m.to()) <= 0
}

/// Great exclusion (`is_easy_to_find`): a promotion the opponent cannot win back by exchange.
pub fn is_safe_promotion(before: &Chess, m: Move) -> bool {
    m.is_promotion() && lands_safely(before, m)
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

/// A quiet move of a passed pawn (`is_pushes_passed_pawn`): no opposing pawn ahead of it on its own
/// or a neighbouring file. Passed on its old square, it is passed on the new one too.
pub fn is_passed_pawn_push(before: &Chess, m: Move) -> bool {
    let Some(from) = m
        .from()
        .filter(|_| m.role() == Role::Pawn && !m.is_capture())
    else {
        return false;
    };
    let mover = before.turn();
    let theirs = before.board().pawns() & before.board().by_color(mover.other());
    !theirs.into_iter().any(|sq| {
        sq.file().distance(from.file()) <= 1
            && match mover {
                Color::White => sq.rank() > from.rank(),
                Color::Black => sq.rank() < from.rank(),
            }
    })
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
        static_sacrifice(&p, m, cfg, false)
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
    fn legal_trap_gives_up_the_queen() {
        // Legal's trap, 5.Nxe5 (Chess.com: Brilliant): the queen on d1 is left en
        // prise; it is worth more than the knight, so it is the piece given up.
        let fen = "rn1qkbnr/ppp2p1p/3p2p1/4p3/2B1P1b1/2N2N2/PPPP1PPP/R1BQK2R w KQkq - 0 5";
        assert_eq!(
            sacrifice(fen, "f3e5", &BrilliantConfig::default()),
            Some(Sacrifice {
                square: Square::D1,
                role: Role::Queen,
                see_gain: 600
            })
        );
        // Counting only the moved piece, the knight is given up for a pawn.
        let cfg = BrilliantConfig {
            count_pieces_left_hanging: false,
            ..Default::default()
        };
        assert_eq!(
            sacrifice(fen, "f3e5", &cfg).map(|s| s.role),
            Some(Role::Knight)
        );
    }

    #[test]
    fn traxler_knight_sacrifice_with_a_discovered_attack() {
        // 8...Nxg3 (Chess.com game 185058622904: Brilliant). Leaving e4 opens the
        // queen's line to c4, which no longer makes it a "disguised trade": hxg3
        // still wins the knight for a pawn.
        let fen = "r1b1k2r/pppp1Npp/2n5/4p3/2B1n2q/6P1/PPPP3P/RNBQK2R b kq - 0 8";
        assert_eq!(
            sacrifice(fen, "e4g3", &BrilliantConfig::default()),
            Some(Sacrifice {
                square: Square::G3,
                role: Role::Knight,
                see_gain: 200
            })
        );
        // The rook on h8, en prise since 5.Nxf7, is not something this move gives up.
        let fen = "r1b1k2r/pppp1Npp/8/4p3/2Bn3q/8/PPPP2K1/RNBQ4 b kq - 7 15";
        assert_eq!(sacrifice(fen, "d7d5", &BrilliantConfig::default()), None);
    }

    #[test]
    fn pawn_sacrifices_count_only_when_asked() {
        // 15...d5 (same game, both players below 1000: Brilliant): Bxd5 wins the pawn.
        let fen = "r1b1k2r/pppp1Npp/8/4p3/2Bn3q/8/PPPP2K1/RNBQ4 b kq - 7 15";
        let p = pos(fen);
        let m = board::parse_uci(&p, "d7d5").unwrap();
        let cfg = BrilliantConfig::default();
        assert_eq!(
            static_sacrifice(&p, m, &cfg, true),
            Some(Sacrifice {
                square: Square::D5,
                role: Role::Pawn,
                see_gain: 100
            })
        );
        assert_eq!(static_sacrifice(&p, m, &cfg, false), None);
    }

    #[test]
    fn castling_can_leave_a_piece_behind() {
        // 19.O-O-O (Chess.com game 186411389477: Brilliant): the king no longer
        // guards e2, so the queen on e3 can take the bishop.
        let fen = "r3k2r/p2b1pp1/8/1pp4p/2p5/Q1P1q1PP/PP1NB1P1/R3KR2 w Qkq - 0 19";
        assert_eq!(
            sacrifice(fen, "e1c1", &BrilliantConfig::default()).map(|s| (s.square, s.role)),
            Some((Square::E2, Role::Bishop))
        );
    }

    #[test]
    fn promotions_are_not_sacrifices() {
        // g8=Q+ Kxg8: the new queen is captured, but nothing was given up.
        let fen = "8/6Pk/8/4Q3/8/2P5/PP6/2K1R3 w - - 1 51";
        assert_eq!(sacrifice(fen, "g7g8q", &BrilliantConfig::default()), None);
    }

    fn mv(fen: &str, uci: &str) -> (Chess, Move) {
        let p = pos(fen);
        let m = board::parse_uci(&p, uci).expect("legal move");
        (p, m)
    }

    #[test]
    fn only_safe_square() {
        // The bishop on a2 is attacked by the pawn; b1 is its only safe square
        // (Bxb3 runs into the rook).
        let fen = "1r2k3/8/8/8/8/1p6/B7/4K3 w - - 0 1";
        let (p, m) = mv(fen, "a2b1");
        assert!(is_only_safe_square(&p, m));
        let (p, m) = mv(fen, "a2b3");
        assert!(!is_only_safe_square(&p, m), "not the safe square");
        assert!(!lands_safely(&p, m));
        // Without the rook, Bxb3 is safe too: two ways out.
        let (p, m) = mv("4k3/8/8/8/8/1p6/B7/4K3 w - - 0 1", "a2b1");
        assert!(!is_only_safe_square(&p, m));
    }

    #[test]
    fn capturing_the_attacker() {
        // The knight on b3 attacks c1, so Nxb3 removes the piece attacking the knight.
        let (p, m) = mv("4k3/8/8/8/8/1n6/8/2N1K3 w - - 0 1", "c1b3");
        assert!(removes_attacker(&p, m));
        // It does not attack d1, so Bxb3 only captures.
        let (p, m) = mv("4k3/8/8/8/8/1n6/8/3BK3 w - - 0 1", "d1b3");
        assert!(!removes_attacker(&p, m));
    }

    #[test]
    fn free_piece_by_exchange() {
        let (p, m) = mv("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", "d1d5");
        assert!(captures_free_piece(&p, m, 300));
        let (p, m) = mv("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1", "d1d5");
        assert!(
            !captures_free_piece(&p, m, 300),
            "the pawn wins the rook back"
        );
    }

    #[test]
    fn safe_promotions() {
        let (p, m) = mv("4k3/2P5/8/8/8/8/8/4K3 w - - 0 1", "c7c8q");
        assert!(is_safe_promotion(&p, m));
        let (p, m) = mv("1r2k3/2P5/8/8/8/8/8/4K3 w - - 0 1", "c7c8q");
        assert!(!is_safe_promotion(&p, m), "the rook takes the new queen");
        let (p, m) = mv("4k3/8/2P5/8/8/8/8/4K3 w - - 0 1", "c6c7");
        assert!(!is_safe_promotion(&p, m), "not a promotion");
    }

    #[test]
    fn passed_pawn_pushes() {
        let (p, m) = mv("4k3/8/8/8/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(is_passed_pawn_push(&p, m));
        let (p, m) = mv("4k3/2p5/8/8/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(!is_passed_pawn_push(&p, m), "the c-pawn guards d6");
        // Passed only after the move: the e5 pawn stood ahead of d4 on the next file.
        let (p, m) = mv("4k3/8/8/4p3/3P4/8/8/4K3 w - - 0 1", "d4d5");
        assert!(!is_passed_pawn_push(&p, m));
        let (p, m) = mv("4k3/8/8/8/8/3p4/8/4K3 b - - 0 1", "d3d2");
        assert!(is_passed_pawn_push(&p, m));
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
                static_sacrifice(&game.positions[ply - 1], parsed.mv, &cfg, false).map(|s| s.role),
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
        assert_eq!(walk.gain_ply, Some(0), "the pawn is won by the first move");
    }

    #[test]
    fn gain_ply_is_where_the_material_is_won() {
        // 1.Nc7+ Kd8 2.Nxa8: the fork comes first, the rook falls on ply 2.
        let p = pos("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["b5c7", "e8d8", "c7a8"]), 6);
        assert_eq!(walk.net(), 500);
        assert_eq!(walk.gain_ply, Some(2));
        // Rxd5 exd5 gives the exchange: no gain, no gain ply.
        let p = pos("4k3/8/4p3/3n4/8/8/8/3RK3 w - - 0 1");
        let walk = walk_line(&p, Color::White, &line(&["d1d5", "e6d5"]), 6);
        assert_eq!(walk.gain_ply, None);
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
