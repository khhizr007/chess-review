//! Piece activity (group 13): a piece moves to where it does more, and one
//! error of group 14, a piece moved to where it does much less.
//!
//! Activity is measured by mobility: the squares a piece attacks that its
//! own pieces do not stand on.

use review_core::Phase;
use shakmaty::{Bitboard, Color, File, Position, Rank, Role, Square, attacks};

use super::develop::is_home;
use super::{Found, MoveCtx, Tag};
use crate::analysis::see;
use crate::coach::facts::{self, BIG_CENTER, CENTER, PieceAt};
use crate::coach::geometry::Geometry;

/// A move gains at least this many squares to count as activating a piece.
const ACTIVE_GAIN: usize = 3;
/// A move loses at least this many squares to count as a passive move.
const PASSIVE_LOSS: usize = 4;

/// Activity tags of the move, best first.
pub fn activity_tags(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out: Vec<Found> = [
        rook_on_seventh(ctx),
        outpost(ctx),
        takes_open_file(ctx),
        battery(ctx),
        centralizes_knight(ctx),
        active_square(ctx),
        king_activity(ctx, phase),
    ]
    .into_iter()
    .flatten()
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// A knight, bishop, rook or queen moved off a square it already stood on
/// (not developed from home, not castled), with its square after the move.
fn repositioned(ctx: &MoveCtx<'_>) -> Option<(Square, PieceAt)> {
    let from = ctx.m.from()?;
    let moved = ctx.moved().filter(|_| !ctx.m.is_castle())?;
    let piece = matches!(
        moved.role,
        Role::Knight | Role::Bishop | Role::Rook | Role::Queen
    );
    (piece && !is_home(moved.role, moved.color, from)).then_some((from, moved))
}

/// The moved piece cannot simply be won where it lands.
fn safe(ctx: &MoveCtx<'_>, sq: Square) -> bool {
    see::capture_gain(ctx.after, sq) == 0
}

fn found(tag: Tag, focus: Option<PieceAt>, targets: Vec<PieceAt>, geometry: Geometry) -> Found {
    Found {
        tag,
        focus,
        targets,
        geometry,
        stake: 0,
    }
}

fn relation(from: Square, to: Square) -> Geometry {
    Geometry::Relation { from, to }
}

/// The square a castling rook lands on, or the moved piece's square.
fn rook_square(ctx: &MoveCtx<'_>) -> Option<(Square, Square)> {
    match ctx.m {
        shakmaty::Move::Castle { king, rook } => {
            let file = if rook.file() > king.file() {
                File::F
            } else {
                File::D
            };
            Some((rook, Square::from_coords(file, king.rank())))
        }
        m if m.role() == Role::Rook => Some((m.from()?, m.to())),
        _ => None,
    }
}

/// A rook reaches the opponent's second rank, where its pawns stand or its
/// king is boxed in on the back rank.
fn rook_on_seventh(ctx: &MoveCtx<'_>) -> Option<Found> {
    let (from, to) = rook_square(ctx)?;
    let mover = ctx.mover();
    let seventh = mover.relative_rank(Rank::Seventh);
    if to.rank() != seventh || from.rank() == seventh || !safe(ctx, to) {
        return None;
    }
    let board = ctx.after.board();
    let pawns = facts::pawns(board, mover.other()) & Bitboard::from_rank(seventh);
    let king = board
        .king_of(mover.other())
        .filter(|k| k.rank() == mover.other().backrank());
    (pawns.any() || king.is_some()).then(|| {
        found(
            Tag::RookOnSeventh,
            PieceAt::on(board, to),
            pawns
                .into_iter()
                .filter_map(|sq| PieceAt::on(board, sq))
                .collect(),
            relation(from, to),
        )
    })
}

/// A knight or bishop settles on a square in the opponent's half that a pawn
/// of ours guards and no opponent pawn can ever attack.
fn outpost(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx
        .moved()
        .filter(|p| matches!(p.role, Role::Knight | Role::Bishop))?;
    let mover = moved.color;
    let to = moved.square;
    let rank = mover.relative_rank(to.rank()).to_u32();
    if !(3..=5).contains(&rank) || !safe(ctx, to) {
        return None;
    }
    let board = ctx.after.board();
    let guarded = (attacks::pawn_attacks(mover.other(), to) & facts::pawns(board, mover)).any();
    // Opponent pawns on the files next to it, ahead of the square for them
    // (behind it for us), could one day attack it.
    let attackers = facts::pawns(board, mover.other())
        .into_iter()
        .filter(|p| p.file().distance(to.file()) == 1)
        .any(|p| match mover {
            Color::White => p.rank() > to.rank(),
            Color::Black => p.rank() < to.rank(),
        });
    (guarded && !attackers)
        .then(|| found(Tag::Outpost, Some(moved), Vec::new(), relation(from, to)))
}

/// A rook (also a castling rook) moves to a file with no pawns, from a file
/// that had some, and no rook of ours is on that file yet.
fn takes_open_file(ctx: &MoveCtx<'_>) -> Option<Found> {
    let (from, to) = rook_square(ctx)?;
    let board = ctx.after.board();
    let mover = ctx.mover();
    let open = facts::is_open_file(board, to.file());
    let was_open = facts::is_open_file(ctx.before.board(), from.file()) && from.file() == to.file();
    let others = board.rooks() & board.by_color(mover) & Bitboard::from_file(to.file());
    (open && !was_open && others.count() == 1 && safe(ctx, to)).then(|| {
        found(
            Tag::TakesOpenFile,
            PieceAt::on(board, to),
            Vec::new(),
            relation(from, to),
        )
    })
}

/// Two of our line pieces now stand one behind the other on a line they both
/// move along (rooks or a rook and the queen on a file or rank, a bishop
/// and the queen on a diagonal), and the front one bears on the opponent's
/// pieces or king along that line.
fn battery(ctx: &MoveCtx<'_>) -> Option<Found> {
    let moved = ctx.moved().filter(|p| {
        matches!(p.role, Role::Bishop | Role::Rook | Role::Queen) && !ctx.m.is_castle()
    })?;
    let board = ctx.after.board();
    let mover = moved.color;
    let partner_on = |pos: &shakmaty::Chess, a: Square| {
        let b = pos.board();
        (b.by_color(mover) & b.sliders())
            .without(a)
            .into_iter()
            .find(|other| lined_up(b, a, *other))
    };
    let partner = partner_on(ctx.after, moved.square)?;
    // The pair was not lined up before the move.
    if ctx
        .m
        .from()
        .is_some_and(|f| lined_up(ctx.before.board(), f, partner))
    {
        return None;
    }
    // The front piece is the one nearer the opponent along the line; what it
    // bears on lies past it, away from the rear piece.
    let (rear, front) = (partner, moved.square);
    let (rear, front) = if bears_on_opponent(board, rear, front) {
        (rear, front)
    } else if bears_on_opponent(board, front, rear) {
        (front, rear)
    } else {
        return None;
    };
    let partner_piece = PieceAt::on(board, partner)?;
    Some(found(
        Tag::Battery,
        Some(moved),
        vec![partner_piece],
        relation(rear, front),
    ))
}

/// The pieces on `a` and `b` both move along the line through them, and
/// nothing stands between them.
fn lined_up(board: &shakmaty::Board, a: Square, b: Square) -> bool {
    let (Some(ra), Some(rb)) = (board.role_at(a), board.role_at(b)) else {
        return false;
    };
    if a == b || (attacks::between(a, b) & board.occupied()).any() {
        return false;
    }
    let straight = a.file() == b.file() || a.rank() == b.rank();
    let moves_along = |r: Role| match r {
        Role::Queen => true,
        Role::Rook => straight,
        Role::Bishop => !straight,
        _ => false,
    };
    attacks::ray(a, b).any() && moves_along(ra) && moves_along(rb)
}

/// Past `front`, away from `rear`, the front piece attacks an opponent piece
/// or a square next to the opponent's king; for rooks on a file, the file is
/// free of our own pawns.
fn bears_on_opponent(board: &shakmaty::Board, rear: Square, front: Square) -> bool {
    let Some(piece) = board.piece_at(front) else {
        return false;
    };
    let past = attacks::attacks(front, piece, board.occupied())
        & attacks::ray(rear, front)
        & !attacks::between(rear, front)
        & !Bitboard::from_square(rear);
    let them = piece.color.other();
    let king_zone = board.king_of(them).map_or(Bitboard::EMPTY, |k| {
        attacks::king_attacks(k) | Bitboard::from_square(k)
    });
    if (past & (board.by_color(them) | king_zone)).any() {
        return true;
    }
    rear.file() == front.file()
        && (facts::pawns(board, piece.color) & Bitboard::from_file(front.file())).is_empty()
        && past.any()
}

/// A knight from the edge or the back of the board comes to the sixteen
/// central squares.
fn centralizes_knight(ctx: &MoveCtx<'_>) -> Option<Found> {
    let (from, moved) = repositioned(ctx)?;
    let central = moved.role == Role::Knight
        && !BIG_CENTER.contains(from)
        && BIG_CENTER.contains(moved.square)
        && safe(ctx, moved.square);
    central.then(|| {
        found(
            Tag::CentralizesKnight,
            Some(moved),
            Vec::new(),
            relation(from, moved.square),
        )
    })
}

/// A piece that already stood somewhere moves to a square where it reaches
/// clearly more squares (a bishop gains scope).
fn active_square(ctx: &MoveCtx<'_>) -> Option<Found> {
    let (from, moved) = repositioned(ctx)?;
    if ctx.before.is_check() || !safe(ctx, moved.square) {
        return None;
    }
    let before = facts::mobility(ctx.before.board(), from);
    let after = facts::mobility(ctx.after.board(), moved.square);
    let enough = match moved.role {
        Role::Knight => 5,
        Role::Bishop | Role::Rook => 7,
        _ => 12,
    };
    (after >= before + ACTIVE_GAIN && after >= enough).then(|| {
        let tag = if moved.role == Role::Bishop {
            Tag::BishopGainsScope
        } else {
            Tag::MovesToActiveSquare
        };
        found(tag, Some(moved), Vec::new(), relation(from, moved.square))
    })
}

/// In the endgame the king walks toward the centre, to a square where it is
/// not attacked.
fn king_activity(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    if phase != Phase::Endgame
        || ctx.m.role() != Role::King
        || ctx.m.is_castle()
        || ctx.before.is_check()
        || ctx.mover().relative_rank(from.rank()) > Rank::Second
    {
        return None;
    }
    let to = ctx.to();
    let distance = |sq: Square| {
        CENTER
            .into_iter()
            .map(|c| c.distance(sq))
            .min()
            .unwrap_or(0)
    };
    let attacked = facts::attackers(ctx.after.board(), to, ctx.mover().other()).any();
    (distance(to) < distance(from) && !attacked).then(|| {
        found(
            Tag::KingActivity,
            ctx.moved(),
            Vec::new(),
            relation(from, to),
        )
    })
}

/// A piece that already stood somewhere moves (not capturing, not answering
/// a check or a threat to it) to a square where it reaches far fewer
/// squares, and few.
pub fn passive_square(ctx: &MoveCtx<'_>) -> Option<Found> {
    let (from, moved) = repositioned(ctx)?;
    if ctx.m.is_capture()
        || ctx.before.is_check()
        || facts::threat(ctx.before.board(), from, moved.color.other()) > 0
    {
        return None;
    }
    let before = facts::mobility(ctx.before.board(), from);
    let after = facts::mobility(ctx.after.board(), moved.square);
    (before >= after + PASSIVE_LOSS && after <= 3).then(|| {
        found(
            Tag::MovesToPassiveSquare,
            Some(moved),
            Vec::new(),
            relation(from, moved.square),
        )
    })
}
