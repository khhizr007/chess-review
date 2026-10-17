//! Static exchange evaluation (SEE).
//!
//! `see` plays out the capture sequence on one square, each side capturing
//! with its least valuable piece and free to stop when continuing would lose
//! material (the standard swap list). Attackers are recomputed as pieces
//! leave the board, so x-ray attackers behind them join in. Pieces pinned to
//! their king only capture along the pin line, and a king never captures
//! into an attacked square. Promotions during the exchange are ignored.

use shakmaty::{Bitboard, Board, Chess, Color, Position, Role, Square, attacks};

use crate::board;

/// Exchange value of a piece (cp): the reference piece values, with the king
/// at 99999 as in the reference sacrifice gates.
pub fn value(role: Role) -> i32 {
    match role {
        Role::King => 99_999,
        other => board::role_value(other),
    }
}

/// Material (cp) that `attacker` wins by capturing first on `sq` and then
/// exchanging as long as it pays. Zero when `sq` holds no `attacker.other()`
/// piece or no piece of `attacker` can capture there. Negative when the only
/// captures lose material.
pub fn see(board: &Board, sq: Square, attacker: Color) -> i32 {
    let Some(target) = board.piece_at(sq) else {
        return 0;
    };
    if target.color == attacker {
        return 0;
    }
    let (white_pins, black_pins) = (pinned(board, Color::White), pinned(board, Color::Black));
    let may_capture = |color: Color, from: Square| {
        let pins = match color {
            Color::White => white_pins,
            Color::Black => black_pins,
        };
        !pins.contains(from)
            || board
                .king_of(color)
                .is_some_and(|king| attacks::aligned(from, sq, king))
    };
    let least_valuable = |color: Color, occupied: Bitboard| {
        let candidates = board.attacks_to(sq, color, occupied) & occupied;
        [
            Role::Pawn,
            Role::Knight,
            Role::Bishop,
            Role::Rook,
            Role::Queen,
            Role::King,
        ]
        .into_iter()
        .find_map(|role| {
            (candidates & board.by_role(role))
                .into_iter()
                .find(|&from| may_capture(color, from))
                .map(|from| (from, role))
        })
    };

    let mut occupied = board.occupied();
    let mut gain = [0i32; 32];
    let mut depth = 0;
    gain[0] = value(target.role);
    let mut side = attacker;
    let Some(mut capturer) = least_valuable(side, occupied) else {
        return 0;
    };
    loop {
        let (from, role) = capturer;
        // A king may not capture a defended piece.
        if role == Role::King && least_valuable(side.other(), occupied.without(from)).is_some() {
            break;
        }
        depth += 1;
        // Score if the other side now recaptures the piece that just captured. No
        // early cut-off: it would keep the sign but not the exact value.
        gain[depth] = value(role) - gain[depth - 1];
        if depth == gain.len() - 1 {
            break;
        }
        occupied.discard(from);
        side = side.other();
        match least_valuable(side, occupied) {
            Some(next) => capturer = next,
            None => break,
        }
    }
    // No legal first capture. Otherwise gain[depth] assumed a recapture that never
    // came, so only the entries below it are folded back.
    if depth == 0 {
        return 0;
    }
    while depth > 1 {
        depth -= 1;
        gain[depth - 1] = -(-gain[depth - 1]).max(gain[depth]);
    }
    gain[0]
}

/// Material the side to move in `pos` wins by capturing on `sq` now: the SEE
/// of the exchange if a legal capture there exists, otherwise zero.
pub fn capture_gain(pos: &Chess, sq: Square) -> i32 {
    let can_capture = pos
        .capture_moves()
        .iter()
        .any(|m| m.to() == sq && !m.is_en_passant());
    if can_capture {
        see(pos.board(), sq, pos.turn()).max(0)
    } else {
        0
    }
}

/// Pieces of `color` pinned to their own king by an enemy slider.
fn pinned(board: &Board, color: Color) -> Bitboard {
    let Some(king) = board.king_of(color) else {
        return Bitboard::EMPTY;
    };
    let enemy = board.by_color(color.other());
    let snipers = (attacks::rook_attacks(king, Bitboard::EMPTY) & board.rooks_and_queens())
        | (attacks::bishop_attacks(king, Bitboard::EMPTY) & board.bishops_and_queens());
    let mut pins = Bitboard::EMPTY;
    for sniper in snipers & enemy {
        let blockers = attacks::between(king, sniper) & board.occupied();
        if blockers.count() == 1 {
            pins.add(blockers & board.by_color(color));
        }
    }
    pins
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

    fn see_at(fen: &str, sq: Square) -> i32 {
        let p = pos(fen);
        see(p.board(), sq, p.turn())
    }

    #[test]
    fn undefended_piece_is_won_outright() {
        assert_eq!(see_at("4k3/8/8/3n4/8/8/8/3RK3 w - - 0 1", Square::D5), 300);
    }

    #[test]
    fn defended_pawn_costs_the_capturing_rook() {
        // Rxd5 exd5: rook for pawn, so the capture loses 400.
        assert_eq!(
            see_at("4k3/8/4p3/3p4/8/8/8/3RK3 w - - 0 1", Square::D5),
            -400
        );
    }

    #[test]
    fn piece_defended_only_by_a_pawn_is_still_won_by_a_pawn() {
        // exd5 exd5: knight for pawn.
        assert_eq!(
            see_at("4k3/8/4p3/3n4/4P3/8/8/4K3 w - - 0 1", Square::D5),
            200
        );
    }

    #[test]
    fn x_ray_attackers_join_the_exchange() {
        // Doubled rooks against one defender: Rxd5 Rxd5 Rxd5 wins the knight.
        assert_eq!(
            see_at("3rk3/8/8/3n4/8/8/3R4/3RK3 w - - 0 1", Square::D5),
            300
        );
        // Without the second rook the knight is defended: Rxd5 Rxd5 loses the exchange.
        assert_eq!(
            see_at("3rk3/8/8/3n4/8/8/8/3RK3 w - - 0 1", Square::D5),
            -200
        );
    }

    #[test]
    fn king_does_not_capture_a_defended_piece() {
        // Kxe2 is illegal because the rook on e8 defends e2.
        assert_eq!(see_at("4r1k1/8/8/8/8/8/4n3/4K3 w - - 0 1", Square::E2), 0);
        // Undefended: the king takes it.
        assert_eq!(see_at("6k1/8/8/8/8/8/4n3/4K3 w - - 0 1", Square::E2), 300);
    }

    #[test]
    fn pinned_piece_does_not_capture_off_the_pin_line() {
        // The knight on e4 is pinned by the rook on e8 and cannot take on d6... it cannot
        // move at all; a rook on d1 can.
        assert_eq!(see_at("4r1k1/8/3b4/8/4N3/8/8/4K3 w - - 0 1", Square::D6), 0);
        assert_eq!(
            see_at("4r1k1/8/3b4/8/4N3/8/8/3RK3 w - - 0 1", Square::D6),
            300
        );
    }

    #[test]
    fn capture_gain_needs_a_legal_capture() {
        // Black is in check from the bishop, so ...exd5 is not available.
        let p = pos("4k3/8/4p3/1B1N4/8/8/8/4K3 b - - 0 1");
        assert!(p.is_check());
        assert_eq!(capture_gain(&p, Square::D5), 0);
        let p = pos("4k3/8/4p3/3N4/8/8/8/4K3 b - - 0 1");
        assert_eq!(capture_gain(&p, Square::D5), 300);
    }
}
