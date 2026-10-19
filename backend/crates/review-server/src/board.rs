//! Board helpers on top of `shakmaty`: material, perspective, FEN, phases.

use review_core::{Phase, Side};
use shakmaty::fen::{Epd, Fen};
use shakmaty::uci::UciMove;
use shakmaty::{
    Board, CastlingMode, Chess, Color, EnPassantMode, Move, Position, Rank, Role, Square,
};

pub fn side(color: Color) -> Side {
    match color {
        Color::White => Side::White,
        Color::Black => Side::Black,
    }
}

/// Material values used by the reference engine's sacrifice gates (cp).
pub fn role_value(role: Role) -> i32 {
    match role {
        Role::Pawn => 100,
        Role::Knight | Role::Bishop => 300,
        Role::Rook => 500,
        Role::Queen => 900,
        Role::King => 0,
    }
}

pub fn material(board: &Board, color: Color) -> i32 {
    let m = board.material_side(color);
    i32::from(m.pawn) * 100
        + i32::from(m.knight) * 300
        + i32::from(m.bishop) * 300
        + i32::from(m.rook) * 500
        + i32::from(m.queen) * 900
}

/// Material balance from `color`'s point of view (cp).
pub fn balance(board: &Board, color: Color) -> i32 {
    material(board, color) - material(board, color.other())
}

pub fn fen(pos: &Chess) -> String {
    Fen::from_position(pos, EnPassantMode::Legal).to_string()
}

/// Position key without move counters (used for book lookup and repetition).
pub fn epd(pos: &Chess) -> String {
    Epd::from_position(pos, EnPassantMode::Legal).to_string()
}

pub fn uci(m: Move, mode: CastlingMode) -> String {
    UciMove::from_move(m, mode).to_string()
}

pub fn parse_uci(pos: &Chess, text: &str) -> Option<Move> {
    let uci: UciMove = text.parse().ok()?;
    uci.to_move(pos).ok()
}

/// Draw by rule visible on the board alone (repetition is tracked by the caller).
pub fn is_board_draw(pos: &Chess) -> bool {
    pos.is_stalemate() || pos.is_insufficient_material() || pos.halfmoves() >= 100
}

/// The position with the other side to move and no en passant square (a null
/// move). `None` when the side to move is in check, when the result is not a
/// legal position, or when the new side to move has no legal move.
pub fn null_move(pos: &Chess) -> Option<Chess> {
    if pos.is_check() {
        return None;
    }
    pos.clone()
        .swap_turn()
        .ok()
        .filter(|p| !p.legal_moves().is_empty())
}

/// Non-pawn, non-king pieces on the board (both colours).
fn piece_count(board: &Board) -> u32 {
    (board.knights() | board.bishops() | board.rooks() | board.queens()).count() as u32
}

fn back_rank_sparse(board: &Board, color: Color) -> bool {
    let rank = if color == Color::White {
        Rank::First
    } else {
        Rank::Eighth
    };
    let own_on_rank = (board.by_color(color) & shakmaty::Bitboard::from_rank(rank)).count();
    own_on_rank < 4
}

/// Fallback phase heuristic (functional spec §9, open question 9). Callers
/// make it monotonic over the game.
pub fn raw_phase(board: &Board, ply: u32) -> Phase {
    let pieces = piece_count(board);
    if pieces <= 6 {
        Phase::Endgame
    } else if pieces <= 10
        || back_rank_sparse(board, Color::White)
        || back_rank_sparse(board, Color::Black)
        || ply >= 40
    {
        Phase::Middlegame
    } else {
        Phase::Opening
    }
}

/// Squares of a move for board highlighting (castling uses king → destination).
pub fn move_squares(m: Move) -> (Option<Square>, Square) {
    match m {
        Move::Castle { king, rook } => {
            let to = Square::from_coords(
                if rook.file() > king.file() {
                    shakmaty::File::G
                } else {
                    shakmaty::File::C
                },
                king.rank(),
            );
            (Some(king), to)
        }
        other => (other.from(), other.to()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_position_is_opening_with_equal_material() {
        let pos = Chess::default();
        assert_eq!(balance(pos.board(), Color::White), 0);
        assert_eq!(material(pos.board(), Color::White), 3900);
        assert_eq!(raw_phase(pos.board(), 0), Phase::Opening);
    }

    #[test]
    fn bare_kings_and_pawns_is_endgame() {
        let pos: Chess = "8/5k2/8/3p4/3P4/8/5K2/8 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        assert_eq!(raw_phase(pos.board(), 60), Phase::Endgame);
    }
}
