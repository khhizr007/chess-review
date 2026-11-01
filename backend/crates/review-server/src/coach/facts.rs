//! Position facts the detectors are built from: who attacks and defends a
//! square, what an exchange there wins (SEE), hanging and trapped pieces,
//! pins and x-rays.
//!
//! Everything here is static: it looks at one board and never searches.

use shakmaty::{Bitboard, Board, Chess, Color, File, Position, Rank, Role, Square, attacks};

use crate::analysis::see;

/// Exchange value of a piece (cp), with the king worth more than everything.
pub fn value(role: Role) -> i32 {
    see::value(role)
}

/// A piece on a square, as the text names it ("your knight on f6").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieceAt {
    pub role: Role,
    pub color: Color,
    pub square: Square,
}

impl PieceAt {
    pub fn on(board: &Board, square: Square) -> Option<PieceAt> {
        board.piece_at(square).map(|p| PieceAt {
            role: p.role,
            color: p.color,
            square,
        })
    }
}

/// Pieces of `by` that attack `sq` on this board.
pub fn attackers(board: &Board, sq: Square, by: Color) -> Bitboard {
    board.attacks_to(sq, by, board.occupied())
}

/// The least valuable piece of `by` attacking `sq`.
pub fn least_valuable_attacker(board: &Board, sq: Square, by: Color) -> Option<Square> {
    attackers(board, sq, by)
        .into_iter()
        .min_by_key(|s| board.role_at(*s).map_or(i32::MAX, value))
}

/// Material that `by` wins by starting an exchange on `sq` (never negative:
/// a side that would lose material simply does not capture). Ignores whose
/// turn it is; see [`see::capture_gain`] for the side to move.
pub fn threat(board: &Board, sq: Square, by: Color) -> i32 {
    see::see(board, sq, by).max(0)
}

/// The piece on `sq` can be won by its opponent (statically).
pub fn is_hanging(board: &Board, sq: Square) -> bool {
    board
        .piece_at(sq)
        .is_some_and(|p| p.role != Role::King && threat(board, sq, p.color.other()) > 0)
}

/// Non-king pieces of `color` that the opponent can win by exchange, with the
/// material at stake.
pub fn hanging(board: &Board, color: Color) -> Vec<(Square, i32)> {
    (board.by_color(color) & !board.kings())
        .into_iter()
        .filter_map(|sq| {
            let gain = threat(board, sq, color.other());
            (gain > 0).then_some((sq, gain))
        })
        .collect()
}

/// A line piece aimed at `front`, with `behind` the next piece on the same
/// line past it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XRay {
    pub attacker: Square,
    pub front: Square,
    pub behind: Square,
}

/// The first piece past `through` on the line from the slider on `from`, if
/// the slider attacks `through` and moves along that line.
pub fn behind(board: &Board, from: Square, through: Square) -> Option<Square> {
    let piece = board.piece_at(from)?;
    if !matches!(piece.role, Role::Bishop | Role::Rook | Role::Queen)
        || !board.attacks_from(from).contains(through)
    {
        return None;
    }
    let occupied = board.occupied().without(through);
    let reach = attacks::attacks(from, piece, occupied) & attacks::ray(from, through);
    (reach & occupied)
        .into_iter()
        .find(|sq| attacks::between(from, *sq).contains(through))
}

/// Every x-ray of the slider on `from`: each piece it attacks with the piece
/// behind it.
pub fn xrays(board: &Board, from: Square) -> Vec<XRay> {
    board
        .attacks_from(from)
        .into_iter()
        .filter(|front| board.occupied().contains(*front))
        .filter_map(|front| {
            behind(board, from, front).map(|behind| XRay {
                attacker: from,
                front,
                behind,
            })
        })
        .collect()
}

/// A piece of one side that cannot leave a line without exposing the king
/// (absolute pin) or a more valuable piece (relative pin) behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pin {
    pub pinner: Square,
    pub pinned: Square,
    pub behind: Square,
    pub absolute: bool,
}

/// Pins against `color`'s pieces.
pub fn pins(board: &Board, color: Color) -> Vec<Pin> {
    let mut out = Vec::new();
    for pinner in board.by_color(color.other()) & board.sliders() {
        for x in xrays(board, pinner) {
            let (Some(front), Some(back)) = (board.piece_at(x.front), board.piece_at(x.behind))
            else {
                continue;
            };
            if front.color != color || back.color != color || front.role == Role::King {
                continue;
            }
            let absolute = back.role == Role::King;
            if absolute || value(back.role) > value(front.role) {
                out.push(Pin {
                    pinner,
                    pinned: x.front,
                    behind: x.behind,
                    absolute,
                });
            }
        }
    }
    out
}

/// The piece on `sq` is pinned to its own king.
pub fn is_absolutely_pinned(board: &Board, sq: Square) -> bool {
    board.color_at(sq).is_some_and(|color| {
        pins(board, color)
            .iter()
            .any(|p| p.absolute && p.pinned == sq)
    })
}

/// Pins on `color`'s pieces that attack `sq` but may not capture there,
/// because they are pinned to their king along another line.
pub fn pinned_guards(board: &Board, sq: Square, color: Color) -> Vec<Pin> {
    let guards = attackers(board, sq, color);
    pins(board, color)
        .into_iter()
        .filter(|p| p.absolute && guards.contains(p.pinned))
        .filter(|p| !attacks::aligned(p.pinner, p.behind, sq))
        .collect()
}

/// The piece on `sq` (a knight, bishop, rook or queen of the side to move in
/// `pos`) is lost wherever it goes: the opponent wins it where it stands, and
/// every legal move of it lands on a square where it is lost too, unless the
/// move captures something at least as valuable. Pieces pinned to their king
/// and positions in check are excluded: those are other motifs.
pub fn is_trapped(pos: &Chess, sq: Square) -> bool {
    let board = pos.board();
    let Some(piece) = board.piece_at(sq) else {
        return false;
    };
    if piece.color != pos.turn()
        || matches!(piece.role, Role::Pawn | Role::King)
        || pos.is_check()
        || is_absolutely_pinned(board, sq)
        || threat(board, sq, piece.color.other()) == 0
    {
        return false;
    }
    let piece_value = value(piece.role);
    pos.legal_moves()
        .iter()
        .filter(|m| m.from() == Some(sq))
        .all(|m| {
            if m.capture().is_some_and(|c| value(c) >= piece_value) {
                return false;
            }
            let mut after = pos.clone();
            after.play_unchecked(*m);
            see::capture_gain(&after, m.to()) > 0
        })
}

/// The piece on `sq` (of the side to move in `pos`) has a square to go to
/// where it cannot be won, or a capture of something at least as valuable.
/// This is how an attacked piece answers a simple threat: it moves away.
pub fn has_safe_square(pos: &Chess, sq: Square) -> bool {
    let Some(piece) = pos.board().piece_at(sq) else {
        return false;
    };
    if piece.color != pos.turn() {
        return false;
    }
    let piece_value = value(piece.role);
    pos.legal_moves()
        .iter()
        .filter(|m| m.from() == Some(sq))
        .any(|m| {
            if m.capture().is_some_and(|c| value(c) >= piece_value) {
                return true;
            }
            let mut after = pos.clone();
            after.play_unchecked(*m);
            see::capture_gain(&after, m.to()) == 0
        })
}

/// Pieces of `color` that the piece on `sq` defends (attacks while being the
/// same colour).
pub fn defended_by(board: &Board, sq: Square) -> Bitboard {
    match board.color_at(sq) {
        Some(color) => board.attacks_from(sq) & board.by_color(color),
        None => Bitboard::EMPTY,
    }
}

// ---------------------------------------------------------------- structure

/// The four centre squares.
pub const CENTER: Bitboard = Bitboard(0x0000_0018_1800_0000);
/// The sixteen squares from c3 to f6.
pub const BIG_CENTER: Bitboard = Bitboard(0x0000_3c3c_3c3c_0000);

pub fn pawns(board: &Board, color: Color) -> Bitboard {
    board.pawns() & board.by_color(color)
}

/// The files next to `file`.
fn neighbour_files(file: File) -> Bitboard {
    [-1, 1]
        .into_iter()
        .filter_map(|d| file.offset(d))
        .fold(Bitboard::EMPTY, |bb, f| bb | Bitboard::from_file(f))
}

/// Squares strictly ahead of `sq` for `color` on its own rank count (ranks
/// further up the board for White, down for Black).
fn ahead(sq: Square, color: Color) -> Bitboard {
    let rank = sq.rank().to_u32();
    (0..8)
        .filter(|r| match color {
            Color::White => *r > rank,
            Color::Black => *r < rank,
        })
        .fold(Bitboard::EMPTY, |bb, r| {
            bb | Bitboard::from_rank(Rank::new(r))
        })
}

/// The pawn on `sq` has no pawn of its side on the files next to it.
pub fn is_isolated(board: &Board, sq: Square) -> bool {
    board
        .color_at(sq)
        .is_some_and(|color| (pawns(board, color) & neighbour_files(sq.file())).is_empty())
}

/// Pawns of `color` that share their file with another pawn of `color`.
pub fn doubled(board: &Board, color: Color) -> Bitboard {
    pawns(board, color)
        .into_iter()
        .filter(|sq| (pawns(board, color) & Bitboard::from_file(sq.file())).more_than_one())
        .collect()
}

/// Pawns of `color` with no pawn of their side on the files next to them.
pub fn isolated(board: &Board, color: Color) -> Bitboard {
    pawns(board, color)
        .into_iter()
        .filter(|sq| is_isolated(board, *sq))
        .collect()
}

/// The pawn on `sq` has no opponent pawn ahead of it on its file or the
/// files next to it.
pub fn is_passed(board: &Board, sq: Square) -> bool {
    let Some(color) = board.color_at(sq) else {
        return false;
    };
    let span = (Bitboard::from_file(sq.file()) | neighbour_files(sq.file())) & ahead(sq, color);
    (pawns(board, color.other()) & span).is_empty()
}

pub fn passed(board: &Board, color: Color) -> Bitboard {
    pawns(board, color)
        .into_iter()
        .filter(|sq| is_passed(board, *sq))
        .collect()
}

/// The pawn on `sq` is backward: no pawn of its side on the files next to it
/// stands level with it or behind it (none can come up to support it), and
/// an opponent pawn guards the square in front of it.
pub fn is_backward(board: &Board, sq: Square) -> bool {
    let Some(color) = board.color_at(sq) else {
        return false;
    };
    let supporters = pawns(board, color) & neighbour_files(sq.file()) & !ahead(sq, color);
    let Some(stop) = sq.offset(color.fold_wb(8, -8)) else {
        return false;
    };
    let guarded = (attacks::pawn_attacks(color, stop) & pawns(board, color.other())).any();
    supporters.is_empty()
        && !(pawns(board, color) & neighbour_files(sq.file())).is_empty()
        && guarded
}

pub fn backward(board: &Board, color: Color) -> Bitboard {
    pawns(board, color)
        .into_iter()
        .filter(|sq| is_backward(board, *sq))
        .collect()
}

/// A file with no pawns on it.
pub fn is_open_file(board: &Board, file: File) -> bool {
    (board.pawns() & Bitboard::from_file(file)).is_empty()
}

/// Squares the piece on `sq` attacks that are not taken by its own pieces.
pub fn mobility(board: &Board, sq: Square) -> usize {
    let own = board
        .color_at(sq)
        .map_or(Bitboard::EMPTY, |c| board.by_color(c));
    (board.attacks_from(sq) & !own).count()
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

    #[test]
    fn relative_and_absolute_pins() {
        // Bg5 pins the knight on f6 to the queen on d8; Bb5 pins the knight on c6 to the king.
        let p = pos("r2qkb1r/ppp2ppp/2n2n2/1B2p1B1/4P3/8/PPP2PPP/RN1QK2R b KQkq - 0 1");
        let mut found = pins(p.board(), Color::Black);
        found.sort_by_key(|p| p.pinned);
        assert_eq!(
            found,
            vec![
                Pin {
                    pinner: Square::B5,
                    pinned: Square::C6,
                    behind: Square::E8,
                    absolute: true
                },
                Pin {
                    pinner: Square::G5,
                    pinned: Square::F6,
                    behind: Square::D8,
                    absolute: false
                },
            ]
        );
        assert!(is_absolutely_pinned(p.board(), Square::C6));
        assert!(!is_absolutely_pinned(p.board(), Square::F6));
    }

    #[test]
    fn a_more_valuable_piece_in_front_is_no_pin() {
        // The rook on e1 sees the queen on e5 with the knight behind it: that is a skewer.
        let p = pos("4k3/4n3/8/4q3/8/8/8/4RK2 b - - 0 1");
        assert!(pins(p.board(), Color::Black).is_empty());
        assert_eq!(
            xrays(p.board(), Square::E1),
            vec![XRay {
                attacker: Square::E1,
                front: Square::E5,
                behind: Square::E7
            }]
        );
    }

    #[test]
    fn hanging_pieces_by_exchange() {
        // The knight on d5 is attacked by a rook and undefended; the pawn on b6 is defended.
        let p = pos("4k3/2p5/1p6/3n4/8/8/8/1R1RK3 b - - 0 1");
        assert_eq!(hanging(p.board(), Color::Black), vec![(Square::D5, 300)]);
        assert!(is_hanging(p.board(), Square::D5));
        assert!(!is_hanging(p.board(), Square::B6));
    }

    #[test]
    fn trapped_bishop() {
        // The rook attacks the bishop on a7; Bb8 runs into the rook and Bxb6 into ...cxb6.
        let p = pos("r3k3/B1p5/1p6/8/8/8/8/4K3 w - - 0 1");
        assert!(is_trapped(&p, Square::A7));
        // Without the c7 pawn, Bxb6 is a way out.
        let p = pos("r3k3/B7/1p6/8/8/8/8/4K3 w - - 0 1");
        assert!(!is_trapped(&p, Square::A7));
        // Not attacked at all: not trapped, however few squares it has.
        let p = pos("4k3/B1p5/1p6/8/8/8/8/4K3 w - - 0 1");
        assert!(!is_trapped(&p, Square::A7));
    }
}
