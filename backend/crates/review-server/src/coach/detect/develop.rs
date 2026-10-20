//! Development, castling and the centre (group 13), and the opening errors
//! of group 14: what a quiet opening move is for, or what it neglects.
//!
//! Home squares are those of standard chess. A piece standing on its home
//! square is taken to be undeveloped; the board does not say whether it has
//! moved before and come back.

use review_core::Phase;
use shakmaty::{Bitboard, CastlingSide, Color, File, Position, Role, Square, attacks};

use super::{Found, MoveCtx, Tag};
use crate::coach::facts::{self, CENTER, PieceAt};
use crate::coach::geometry::Geometry;

/// Development and centre tags of the move, best first.
pub fn develop_tags(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out: Vec<Found> = [
        pawn_center(ctx, phase),
        prepares_central_push(ctx, phase),
        castles(ctx),
        connects_rooks(ctx),
        fianchetto(ctx),
        prepares_to_castle(ctx),
        develops(ctx, phase),
        prepares_bishop(ctx, phase),
        creates_luft(ctx),
    ]
    .into_iter()
    .flatten()
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// Opening errors of the move, best first.
pub fn opening_errors(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out: Vec<Found> = [
        loses_castling(ctx),
        weakens_king(ctx, phase),
        pawn_in_front_of_king(ctx),
        early_queen(ctx, phase),
        same_piece(ctx, phase),
    ]
    .into_iter()
    .flatten()
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// The piece of `role` and `color` starts the game on `sq`.
pub fn is_home(role: Role, color: Color, sq: Square) -> bool {
    sq.rank() == color.backrank()
        && match role {
            Role::Knight => matches!(sq.file(), File::B | File::G),
            Role::Bishop => matches!(sq.file(), File::C | File::F),
            Role::Rook => matches!(sq.file(), File::A | File::H),
            Role::Queen => sq.file() == File::D,
            _ => false,
        }
}

/// Knights and bishops of `color` still on their home squares.
fn undeveloped_minors(board: &shakmaty::Board, color: Color) -> usize {
    ((board.knights() | board.bishops()) & board.by_color(color))
        .into_iter()
        .filter(|sq| {
            board
                .role_at(*sq)
                .is_some_and(|role| is_home(role, color, *sq))
        })
        .count()
}

/// The side of the board a king has castled to: it stands on its back rank
/// on the g- or h-file (or a- to c-file) and can no longer castle.
fn castled(pos: &shakmaty::Chess, color: Color) -> Option<CastlingSide> {
    let king = pos.board().king_of(color)?;
    if king.rank() != color.backrank() || pos.castles().has_color(color) {
        return None;
    }
    match king.file() {
        File::G | File::H => Some(CastlingSide::KingSide),
        File::A | File::B | File::C => Some(CastlingSide::QueenSide),
        _ => None,
    }
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

fn moved_relation(ctx: &MoveCtx<'_>) -> Geometry {
    let (from, to) = crate::board::move_squares(ctx.m);
    Geometry::Relation {
        from: from.unwrap_or(to),
        to,
    }
}

/// A knight or bishop, or later a rook or the queen, leaves its home square.
/// In the opening a minor piece that reaches a centre square is
/// `DevelopsCenter`, any other `DevelopsOpening`; later it is
/// `DevelopsMiddlegame`.
fn develops(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx.moved().filter(|_| !ctx.m.is_castle())?;
    if !is_home(moved.role, moved.color, from) {
        return None;
    }
    let minor = matches!(moved.role, Role::Knight | Role::Bishop);
    let tag = match phase {
        Phase::Opening
            if minor && (ctx.after.board().attacks_from(moved.square) & CENTER).any() =>
        {
            Tag::DevelopsCenter
        }
        Phase::Opening => Tag::DevelopsOpening,
        _ => Tag::DevelopsMiddlegame,
    };
    Some(found(tag, Some(moved), Vec::new(), moved_relation(ctx)))
}

/// A bishop leaves its home square for the long diagonal in front of a pawn
/// pushed one square (b2/g2 behind b3/g3).
fn fianchetto(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    let moved = ctx.moved().filter(|p| p.role == Role::Bishop)?;
    if !is_home(Role::Bishop, moved.color, from) {
        return None;
    }
    let up = moved.color.fold_wb(8, -8);
    let second = moved.color.relative_rank(shakmaty::Rank::Second);
    let pawn = moved.square.offset(up)?;
    let fits = moved.square.rank() == second
        && matches!(moved.square.file(), File::B | File::G)
        && ctx.after.board().piece_at(pawn) == Some(Role::Pawn.of(moved.color));
    fits.then(|| {
        found(
            Tag::Fianchetto,
            Some(moved),
            Vec::new(),
            moved_relation(ctx),
        )
    })
}

/// A bishop on its home square with no square to go to: hemmed in by its
/// own pawns.
fn shut_in_bishops(board: &shakmaty::Board, color: Color) -> Bitboard {
    (board.bishops() & board.by_color(color))
        .into_iter()
        .filter(|sq| is_home(Role::Bishop, color, *sq) && facts::mobility(board, *sq) == 0)
        .collect()
}

/// A pawn move that opens a diagonal for a bishop still shut in on its home
/// square.
fn prepares_bishop(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    if phase != Phase::Opening || ctx.m.role() != Role::Pawn || ctx.m.is_capture() {
        return None;
    }
    let mover = ctx.mover();
    let freed = shut_in_bishops(ctx.before.board(), mover)
        .into_iter()
        .find(|sq| facts::mobility(ctx.after.board(), *sq) > 0)?;
    let bishop = PieceAt::on(ctx.after.board(), freed)?;
    Some(found(
        Tag::PreparesBishopDevelopment,
        Some(bishop),
        Vec::new(),
        Geometry::Relation {
            from: freed,
            to: ctx.m.from().unwrap_or(freed),
        },
    ))
}

/// The pawn moves of the centre, in the opening and later: a pawn that hits
/// an opponent pawn on a centre square strikes at the centre; after the
/// opening, a pawn that hits a locked pawn (one of ours in front of it) on
/// the c- or f-file makes a central break; a pawn that reaches a centre square
/// without hitting a pawn puts a pawn in the centre (opening only); and a
/// pawn that hits a pawn guarded by another pawn attacks the pawn chain.
fn pawn_center(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    if ctx.m.role() != Role::Pawn || ctx.m.is_promotion() {
        return None;
    }
    let mover = ctx.mover();
    let to = ctx.to();
    let board = ctx.after.board();
    let theirs = facts::pawns(board, mover.other());
    let hit = attacks::pawn_attacks(mover, to) & theirs;
    let locked = |sq: Square| {
        sq.offset(mover.other().fold_wb(8, -8))
            .is_some_and(|front| board.piece_at(front) == Some(Role::Pawn.of(mover)))
    };
    let pick = |tag: Tag, target: Square| {
        found(
            tag,
            PieceAt::on(board, target),
            Vec::new(),
            Geometry::Relation {
                from: to,
                to: target,
            },
        )
    };
    let central_files = Bitboard::from_file(File::C)
        | Bitboard::from_file(File::D)
        | Bitboard::from_file(File::E)
        | Bitboard::from_file(File::F);
    if let Some(t) = (hit & CENTER).into_iter().next() {
        return Some(pick(Tag::StrikesAtCenter, t));
    }
    if let Some(t) = (hit & central_files)
        .into_iter()
        .find(|sq| phase != Phase::Opening && locked(*sq))
    {
        return Some(pick(Tag::CentralBreak, t));
    }
    if phase == Phase::Opening && CENTER.contains(to) && hit.is_empty() && !ctx.m.is_capture() {
        let pawn = PieceAt::on(board, to)?;
        return Some(found(
            Tag::PutsPawnInCenter,
            Some(pawn),
            Vec::new(),
            moved_relation(ctx),
        ));
    }
    // A pawn of a chain: guarded by another pawn, or guarding one.
    hit.into_iter()
        .find(|sq| {
            (attacks::pawn_attacks(mover, *sq) & theirs).any()
                || (attacks::pawn_attacks(mover.other(), *sq) & theirs).any()
        })
        .map(|t| pick(Tag::AttacksPawnChain, t))
}

/// A pawn move that guards the square a central pawn of ours wants to
/// advance to (c3 or e3 for d4; c6 or e6 for d5), where no pawn guarded it.
fn prepares_central_push(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    if phase != Phase::Opening || ctx.m.role() != Role::Pawn || ctx.m.is_capture() {
        return None;
    }
    let mover = ctx.mover();
    let from = ctx.m.from()?;
    if matches!(from.file(), File::D | File::E | File::F) {
        return None;
    }
    let ours = facts::pawns(ctx.before.board(), mover);
    let board = ctx.after.board();
    let back = mover.fold_wb(-8, 8);
    (attacks::pawn_attacks(mover, ctx.to()) & CENTER)
        .into_iter()
        .filter(|t| !board.occupied().contains(*t))
        .filter(|t| (attacks::pawn_attacks(mover.other(), *t) & ours).is_empty())
        .find_map(|t| {
            // Our pawn on the file behind the square, with a clear path.
            let mut sq = t.offset(back)?;
            for _ in 0..2 {
                match board.piece_at(sq) {
                    Some(p) if p == Role::Pawn.of(mover) => {
                        // The moved pawn guards the square the push goes to.
                        return Some(found(
                            Tag::PreparesCentralPush,
                            PieceAt::on(board, sq),
                            Vec::new(),
                            Geometry::Relation {
                                from: ctx.to(),
                                to: t,
                            },
                        ));
                    }
                    Some(_) => return None,
                    None => sq = sq.offset(back)?,
                }
            }
            None
        })
}

/// Castling: on the same side as the opponent's king, on the opposite side,
/// or (the opponent not castled yet) kingside or queenside.
fn castles(ctx: &MoveCtx<'_>) -> Option<Found> {
    let rook = ctx.m.castling_side()?;
    let king = ctx.moved()?;
    let tag = match castled(ctx.after, ctx.mover().other()) {
        Some(side) if side == rook => Tag::CastlesSameSide,
        Some(_) => Tag::CastlesOppositeSide,
        None if rook == CastlingSide::KingSide => Tag::CastlesKingside,
        None => Tag::CastlesQueenside,
    };
    Some(found(tag, Some(king), Vec::new(), moved_relation(ctx)))
}

/// The move clears the last piece between the king and a rook it may still
/// castle with.
fn prepares_to_castle(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    if ctx.m.is_castle() || ctx.m.role() == Role::King {
        return None;
    }
    let mover = ctx.mover();
    let castles = ctx.before.castles();
    [CastlingSide::KingSide, CastlingSide::QueenSide]
        .into_iter()
        .filter(|side| castles.has(mover, *side))
        .find(|side| {
            let path = castles.path(mover, *side);
            path.contains(from) && (path & ctx.after.board().occupied()).is_empty()
        })
        .map(|side| {
            let tag = match side {
                CastlingSide::KingSide => Tag::PreparesToCastleKingside,
                CastlingSide::QueenSide => Tag::PreparesToCastleQueenside,
            };
            let king = ctx.after.board().king_of(mover);
            found(
                tag,
                ctx.moved(),
                king.and_then(|k| PieceAt::on(ctx.after.board(), k))
                    .into_iter()
                    .collect(),
                moved_relation(ctx),
            )
        })
}

/// Both rooks on the back rank see each other after the move, and the moved
/// piece stood between them.
fn connects_rooks(ctx: &MoveCtx<'_>) -> Option<Found> {
    let from = ctx.m.from()?;
    let mover = ctx.mover();
    let board = ctx.after.board();
    let rooks = board.rooks() & board.by_color(mover) & Bitboard::from_rank(mover.backrank());
    let [a, b] = <[Square; 2]>::try_from(rooks.into_iter().collect::<Vec<_>>()).ok()?;
    let between = attacks::between(a, b);
    let connected = (between & board.occupied()).is_empty();
    (connected && between.contains(from) && !ctx.m.is_castle()).then(|| {
        found(
            Tag::ConnectsRooks,
            ctx.moved(),
            [a, b]
                .into_iter()
                .filter_map(|sq| PieceAt::on(board, sq))
                .collect(),
            Geometry::Relation { from: a, to: b },
        )
    })
}

/// After castling, a pawn in front of the king steps forward one square and
/// gives the king a square to breathe.
fn creates_luft(ctx: &MoveCtx<'_>) -> Option<Found> {
    let shield = king_shield_move(ctx)?;
    let second = ctx.mover().relative_rank(shakmaty::Rank::Second);
    let one_step = shield.from.rank() == second && shield.to.rank().distance(second) == 1;
    (one_step
        && !ctx.m.is_capture()
        && matches!(shield.to.file(), File::G | File::H | File::A | File::B))
    .then(|| {
        found(
            Tag::CreatesLuft,
            PieceAt::on(ctx.after.board(), shield.king),
            Vec::new(),
            Geometry::Relation {
                from: shield.king,
                to: shield.from,
            },
        )
    })
}

struct ShieldMove {
    king: Square,
    from: Square,
    to: Square,
}

/// A pawn move on the files around the king, after it has castled, by a
/// pawn still near it (on the second or third rank).
fn king_shield_move(ctx: &MoveCtx<'_>) -> Option<ShieldMove> {
    let from = ctx.m.from()?;
    let mover = ctx.mover();
    if ctx.m.role() != Role::Pawn || castled(ctx.before, mover).is_none() {
        return None;
    }
    let king = ctx.before.board().king_of(mover)?;
    let near =
        from.file().distance(king.file()) <= 1 && from.rank().distance(mover.backrank()) <= 2;
    near.then_some(ShieldMove {
        king,
        from,
        to: ctx.to(),
    })
}

// ---------------------------------------------------------------- errors

/// The king or a rook moves (not castling) and a right to castle is lost.
fn loses_castling(ctx: &MoveCtx<'_>) -> Option<Found> {
    let mover = ctx.mover();
    if ctx.m.is_castle() || !matches!(ctx.m.role(), Role::King | Role::Rook) {
        return None;
    }
    let had = ctx.before.castles();
    let has = ctx.after.castles();
    let lost = [CastlingSide::KingSide, CastlingSide::QueenSide]
        .into_iter()
        .any(|side| had.has(mover, side) && !has.has(mover, side));
    let king = ctx.after.board().king_of(mover)?;
    lost.then(|| {
        found(
            Tag::LosesCastling,
            ctx.moved(),
            PieceAt::on(ctx.after.board(), king).into_iter().collect(),
            moved_relation(ctx),
        )
    })
}

/// In the opening, the f-pawn steps forward one square while the king is
/// still at home or castled short: the diagonal to the king opens.
fn weakens_king(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    let mover = ctx.mover();
    let second = mover.relative_rank(shakmaty::Rank::Second);
    if phase != Phase::Opening
        || ctx.m.role() != Role::Pawn
        || ctx.m.is_capture()
        || from.file() != File::F
        || from.rank() != second
        || ctx.to().rank().distance(second) != 1
    {
        return None;
    }
    let king = ctx.after.board().king_of(mover)?;
    let exposed =
        king.rank() == mover.backrank() && matches!(king.file(), File::E | File::G | File::H);
    // The square the pawn left and the king behind it: nothing attacks along
    // the diagonal yet, so it is marked, not drawn as an attack.
    exposed.then(|| {
        found(
            Tag::WeakensKing,
            PieceAt::on(ctx.after.board(), king),
            Vec::new(),
            Geometry::Squares(vec![from, king]),
        )
    })
}

/// After castling, a pawn of the king's shelter moves.
fn pawn_in_front_of_king(ctx: &MoveCtx<'_>) -> Option<Found> {
    let shield = king_shield_move(ctx)?;
    // The king and the square the pawn left (not always next to the king).
    Some(found(
        Tag::MovesPawnInFrontOfKing,
        PieceAt::on(ctx.after.board(), shield.king),
        Vec::new(),
        Geometry::Squares(vec![shield.king, shield.from]),
    ))
}

/// In the opening, the queen leaves home while two or more minor pieces have
/// not moved.
fn early_queen(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    let mover = ctx.mover();
    let early = phase == Phase::Opening
        && ctx.m.role() == Role::Queen
        && is_home(Role::Queen, mover, from)
        && !ctx.m.is_capture()
        && undeveloped_minors(ctx.before.board(), mover) >= 2;
    early.then(|| {
        found(
            Tag::TooEarlyQueen,
            ctx.moved(),
            Vec::new(),
            moved_relation(ctx),
        )
    })
}

/// In the opening, a piece that has already moved moves again (without
/// capturing or answering a check) while two or more minor pieces have not
/// moved.
fn same_piece(ctx: &MoveCtx<'_>, phase: Phase) -> Option<Found> {
    let from = ctx.m.from()?;
    let mover = ctx.mover();
    let role = ctx.m.role();
    let again = phase == Phase::Opening
        && matches!(role, Role::Knight | Role::Bishop | Role::Queen)
        && !is_home(role, mover, from)
        && !ctx.m.is_capture()
        && !ctx.before.is_check()
        && undeveloped_minors(ctx.before.board(), mover) >= 2;
    again.then(|| {
        found(
            Tag::MovesSamePiece,
            ctx.moved(),
            Vec::new(),
            moved_relation(ctx),
        )
    })
}
