//! Board-event detectors: one move in one position → the tags it shows.
//!
//! The definitions follow chess semantics and the behaviour of the lichess
//! puzzle tagger as described in `docs/coach-research-notes.md` §2.1. They are
//! written from those descriptions, not from its code.

mod activity;
mod bad;
mod defence;
mod defender;
mod develop;
mod double;
mod lines;
mod mate;
mod material;
mod promotion;
mod structure;
mod tempo;
mod trade;

use review_core::Phase;
use serde::{Deserialize, Serialize};
use shakmaty::{Chess, Color, Move, Position, Square};
use ts_rs::TS;

use super::facts::PieceAt;
use super::geometry::Geometry;

pub use bad::{bad_tags, loses_trade};
pub use defence::defence_tags;
pub use tempo::tempo_tags;
pub use trade::trade_tags;

/// What a good move does by itself (groups 8–10: trades, defence, tempo),
/// best first. `threat` is the opponent's null-move line before the move.
pub fn good_tags(ctx: &MoveCtx<'_>, threat: Option<&[Move]>) -> Vec<Found> {
    let mut out = defence_tags(ctx, threat);
    out.extend(trade_tags(ctx));
    out.extend(tempo_tags(ctx));
    out.sort_by_key(|f| f.tag);
    out
}

/// Our board-event vocabulary. **Declaration order is priority order**: when
/// several tags apply, the earlier one is said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Tag {
    // 1. Mates.
    /// Mate by a rook or queen on the king's home rank, the king boxed in by its own pieces.
    BackRankMate,
    /// The move gives checkmate.
    Checkmate,
    /// The line forces mate in more than one move.
    ForcedMate,
    // 2. Free material.
    /// Captures a piece the opponent cannot win back by exchange.
    FreePiece,
    /// Captures on a square where the opponent can take back, and still comes
    /// out ahead: a cheaper piece takes a dearer one, or more attackers than
    /// defenders.
    WinsTrade,
    /// Attacks a piece that has no safe square.
    TrappedPiece,
    // 3. Double attacks.
    DoubleCheck,
    /// A fork of the king and queen.
    RoyalFork,
    Fork,
    DiscoveredCheck,
    DiscoveredAttack,
    // 4. Line tactics.
    Skewer,
    /// Attacks a pinned piece, or wins material a pinned defender cannot guard.
    ExploitsPin,
    Pin,
    // 5. Defender tactics.
    /// Captures the piece that guarded another, which is then lost.
    RemovesDefender,
    /// A defender with two jobs: taking back on one square gives up the other.
    Overloaded,
    /// A sacrifice that lures a king, queen or rook onto a square where it is attacked.
    Attraction,
    // 6. Promotion.
    Promotion,
    // 7. Bad-move bases (said about the move itself).
    MovesPinnedPiece,
    DidNotRecapture,
    IgnoresThreat,
    RemovesOwnDefender,
    BadTrade,
    /// Captures, and the opponent's reply takes back and comes out ahead.
    LosesTrade,
    HangsPiece,
    HangsPawn,
    // 8. Trades (said about a good move itself).
    /// Takes back on the square where the opponent just captured.
    Recapture,
    /// Captures a piece that will be taken back, for an even trade.
    EqualTrade,
    // 9. Defence (said about a good move itself).
    /// Puts a piece between a checking piece and the king.
    BlocksCheck,
    /// Answers a check by moving the king, or by capturing the checking piece.
    EvadesCheck,
    /// Guards a threatened piece, or blocks the attack on it.
    DefendsPiece,
    /// Moves a threatened piece to a safe square.
    EscapesThreat,
    /// Leaves a threatened piece and attacks a piece worth at least as much.
    CounterThreat,
    // 10. Tempo (said about a good move itself).
    /// A pawn attacks a piece, which has a safe square and has to go there.
    Kicks,
    /// A piece attacks a piece, which has a safe square and has to go there.
    GainsTempo,
    /// Offers to trade a piece for one of the same value.
    OffersTrade,
    // 11. Sacrifices (Brilliant moves, and a best move that gives up material).
    /// The moved piece can be taken, and that is the point.
    Sacrifice,
    /// Another piece is left to be taken, and that is the point.
    OffersPiece,
    // 13. Positional ideas (said about a good move itself, or the best move).
    /// A capture the opponent takes back with a pawn, doubling its pawns.
    ForcesDoubledPawns,
    /// A capture the opponent takes back with a pawn, leaving a pawn isolated.
    ForcesIsolatedPawn,
    /// A pawn hits an opponent pawn on a centre square.
    StrikesAtCenter,
    /// A pawn hits an opponent pawn on the c- to f-files that is locked by one of ours.
    CentralBreak,
    /// A pawn reaches a centre square in the opening.
    PutsPawnInCenter,
    /// A pawn hits an opponent pawn that guards or is guarded by another pawn.
    AttacksPawnChain,
    /// A pawn guards the square a central pawn of ours wants to advance to.
    PreparesCentralPush,
    /// A rook reaches the opponent's second rank.
    RookOnSeventh,
    /// A knight or bishop settles where a pawn guards it and no opponent pawn can attack it.
    Outpost,
    /// A rook moves to a file with no pawns.
    TakesOpenFile,
    /// Two line pieces stand one behind the other, aimed at the opponent.
    Battery,
    /// A knight comes from the edge to the central squares.
    CentralizesKnight,
    /// A piece moves to where it reaches clearly more squares.
    MovesToActiveSquare,
    /// A bishop moves to a longer diagonal.
    BishopGainsScope,
    /// Castles on the side where the opponent's king is.
    CastlesSameSide,
    /// Castles on the other side from the opponent's king.
    CastlesOppositeSide,
    /// Castles kingside (the opponent has not castled).
    CastlesKingside,
    /// Castles queenside (the opponent has not castled).
    CastlesQueenside,
    /// The rooks see each other along the back rank.
    ConnectsRooks,
    /// A bishop develops to the long diagonal in front of a pawn pushed one square.
    Fianchetto,
    /// Clears the last piece between the king and the queenside rook.
    PreparesToCastleQueenside,
    /// Clears the last piece between the king and the kingside rook.
    PreparesToCastleKingside,
    /// A knight or bishop leaves home in the opening and reaches a centre square.
    DevelopsCenter,
    /// A piece leaves home in the opening.
    DevelopsOpening,
    /// A piece leaves home after the opening.
    DevelopsMiddlegame,
    /// A pawn move opens a diagonal for a bishop still shut in at home.
    PreparesBishopDevelopment,
    /// A passed pawn moves on, before the endgame.
    PushesPassedPawn,
    /// A passed pawn moves on in the endgame.
    PushesPassedPawnEndgame,
    /// A pawn becomes passed.
    MakesPassedPawn,
    /// The king walks toward the centre in the endgame.
    KingActivity,
    /// After castling, a pawn in front of the king steps forward one square.
    CreatesLuft,
    // 14. Positional errors (said about a bad move itself).
    /// The king or a rook moves and a right to castle is lost.
    LosesCastling,
    /// Our pawns end up doubled.
    DoubledPawns,
    /// One of our pawns ends up isolated.
    IsolatedPawn,
    /// One of our pawns ends up backward.
    BackwardPawn,
    /// The f-pawn steps forward in the opening and opens the king's diagonal.
    WeakensKing,
    /// After castling, a pawn of the king's shelter moves.
    MovesPawnInFrontOfKing,
    /// The queen comes out while the minor pieces are still at home.
    TooEarlyQueen,
    /// A piece moves a second time in the opening while others are still at home.
    MovesSamePiece,
    /// A piece moves to a square where it does much less.
    MovesToPassiveSquare,
    // 15. The move as a whole.
    /// The only move that keeps a Good result: every other line loses more.
    OnlyMove,
    /// The evaluation slides a bucket (say, from equal to worse).
    Slide,
    /// Fallback: the line wins material through no named tactic.
    MaterialGain,
}

impl Tag {
    pub const ALL: [Tag; 81] = [
        Tag::BackRankMate,
        Tag::Checkmate,
        Tag::ForcedMate,
        Tag::FreePiece,
        Tag::WinsTrade,
        Tag::TrappedPiece,
        Tag::DoubleCheck,
        Tag::RoyalFork,
        Tag::Fork,
        Tag::DiscoveredCheck,
        Tag::DiscoveredAttack,
        Tag::Skewer,
        Tag::ExploitsPin,
        Tag::Pin,
        Tag::RemovesDefender,
        Tag::Overloaded,
        Tag::Attraction,
        Tag::Promotion,
        Tag::MovesPinnedPiece,
        Tag::DidNotRecapture,
        Tag::IgnoresThreat,
        Tag::RemovesOwnDefender,
        Tag::BadTrade,
        Tag::LosesTrade,
        Tag::HangsPiece,
        Tag::HangsPawn,
        Tag::Recapture,
        Tag::EqualTrade,
        Tag::BlocksCheck,
        Tag::EvadesCheck,
        Tag::DefendsPiece,
        Tag::EscapesThreat,
        Tag::CounterThreat,
        Tag::Kicks,
        Tag::GainsTempo,
        Tag::OffersTrade,
        Tag::Sacrifice,
        Tag::OffersPiece,
        Tag::ForcesDoubledPawns,
        Tag::ForcesIsolatedPawn,
        Tag::StrikesAtCenter,
        Tag::CentralBreak,
        Tag::PutsPawnInCenter,
        Tag::AttacksPawnChain,
        Tag::PreparesCentralPush,
        Tag::RookOnSeventh,
        Tag::Outpost,
        Tag::TakesOpenFile,
        Tag::Battery,
        Tag::CentralizesKnight,
        Tag::MovesToActiveSquare,
        Tag::BishopGainsScope,
        Tag::CastlesSameSide,
        Tag::CastlesOppositeSide,
        Tag::CastlesKingside,
        Tag::CastlesQueenside,
        Tag::ConnectsRooks,
        Tag::Fianchetto,
        Tag::PreparesToCastleQueenside,
        Tag::PreparesToCastleKingside,
        Tag::DevelopsCenter,
        Tag::DevelopsOpening,
        Tag::DevelopsMiddlegame,
        Tag::PreparesBishopDevelopment,
        Tag::PushesPassedPawn,
        Tag::PushesPassedPawnEndgame,
        Tag::MakesPassedPawn,
        Tag::KingActivity,
        Tag::CreatesLuft,
        Tag::LosesCastling,
        Tag::DoubledPawns,
        Tag::IsolatedPawn,
        Tag::BackwardPawn,
        Tag::WeakensKing,
        Tag::MovesPawnInFrontOfKing,
        Tag::TooEarlyQueen,
        Tag::MovesSamePiece,
        Tag::MovesToPassiveSquare,
        Tag::OnlyMove,
        Tag::Slide,
        Tag::MaterialGain,
    ];

    /// Key of the tag's wording in `text/en.toml`.
    pub fn key(self) -> &'static str {
        match self {
            Tag::BackRankMate => "back_rank_mate",
            Tag::Checkmate => "checkmate",
            Tag::ForcedMate => "forced_mate",
            Tag::FreePiece => "free_piece",
            Tag::WinsTrade => "wins_trade",
            Tag::TrappedPiece => "trapped_piece",
            Tag::DoubleCheck => "double_check",
            Tag::RoyalFork => "royal_fork",
            Tag::Fork => "fork",
            Tag::DiscoveredCheck => "discovered_check",
            Tag::DiscoveredAttack => "discovered_attack",
            Tag::Skewer => "skewer",
            Tag::ExploitsPin => "exploits_pin",
            Tag::Pin => "pin",
            Tag::RemovesDefender => "removes_defender",
            Tag::Overloaded => "overloaded",
            Tag::Attraction => "attraction",
            Tag::Promotion => "promotion",
            Tag::MovesPinnedPiece => "moves_pinned_piece",
            Tag::DidNotRecapture => "did_not_recapture",
            Tag::IgnoresThreat => "ignores_threat",
            Tag::RemovesOwnDefender => "removes_own_defender",
            Tag::BadTrade => "bad_trade",
            Tag::LosesTrade => "loses_trade",
            Tag::HangsPiece => "hangs_piece",
            Tag::HangsPawn => "hangs_pawn",
            Tag::Recapture => "recapture",
            Tag::EqualTrade => "equal_trade",
            Tag::BlocksCheck => "blocks_check",
            Tag::EvadesCheck => "evades_check",
            Tag::DefendsPiece => "defends_piece",
            Tag::EscapesThreat => "escapes_threat",
            Tag::CounterThreat => "counter_threat",
            Tag::Kicks => "kicks",
            Tag::GainsTempo => "gains_tempo",
            Tag::OffersTrade => "offers_trade",
            Tag::Sacrifice => "sacrifice",
            Tag::OffersPiece => "offers_piece",
            Tag::ForcesDoubledPawns => "forces_doubled_pawns",
            Tag::ForcesIsolatedPawn => "forces_isolated_pawn",
            Tag::StrikesAtCenter => "strikes_at_center",
            Tag::CentralBreak => "central_break",
            Tag::PutsPawnInCenter => "puts_pawn_in_center",
            Tag::AttacksPawnChain => "attacks_pawn_chain",
            Tag::PreparesCentralPush => "prepares_central_push",
            Tag::RookOnSeventh => "rook_on_seventh",
            Tag::Outpost => "outpost",
            Tag::TakesOpenFile => "takes_open_file",
            Tag::Battery => "battery",
            Tag::CentralizesKnight => "centralizes_knight",
            Tag::MovesToActiveSquare => "moves_to_active_square",
            Tag::BishopGainsScope => "bishop_gains_scope",
            Tag::CastlesSameSide => "castles_same_side",
            Tag::CastlesOppositeSide => "castles_opposite_side",
            Tag::CastlesKingside => "castles_kingside",
            Tag::CastlesQueenside => "castles_queenside",
            Tag::ConnectsRooks => "connects_rooks",
            Tag::Fianchetto => "fianchetto",
            Tag::PreparesToCastleQueenside => "prepares_to_castle_queenside",
            Tag::PreparesToCastleKingside => "prepares_to_castle_kingside",
            Tag::DevelopsCenter => "develops_center",
            Tag::DevelopsOpening => "develops_opening",
            Tag::DevelopsMiddlegame => "develops_middlegame",
            Tag::PreparesBishopDevelopment => "prepares_bishop_development",
            Tag::PushesPassedPawn => "pushes_passed_pawn",
            Tag::PushesPassedPawnEndgame => "pushes_passed_pawn_endgame",
            Tag::MakesPassedPawn => "makes_passed_pawn",
            Tag::KingActivity => "king_activity",
            Tag::CreatesLuft => "creates_luft",
            Tag::LosesCastling => "loses_castling",
            Tag::DoubledPawns => "doubled_pawns",
            Tag::IsolatedPawn => "isolated_pawn",
            Tag::BackwardPawn => "backward_pawn",
            Tag::WeakensKing => "weakens_king",
            Tag::MovesPawnInFrontOfKing => "moves_pawn_in_front_of_king",
            Tag::TooEarlyQueen => "too_early_queen",
            Tag::MovesSamePiece => "moves_same_piece",
            Tag::MovesToPassiveSquare => "moves_to_passive_square",
            Tag::OnlyMove => "only_move",
            Tag::Slide => "slide",
            Tag::MaterialGain => "material_gain",
        }
    }

    pub fn is_mate(self) -> bool {
        matches!(self, Tag::BackRankMate | Tag::Checkmate | Tag::ForcedMate)
    }

    /// Group 7: said about a bad move itself (BASE), never about a line.
    pub fn is_bad_base(self) -> bool {
        matches!(
            self,
            Tag::MovesPinnedPiece
                | Tag::DidNotRecapture
                | Tag::IgnoresThreat
                | Tag::RemovesOwnDefender
                | Tag::BadTrade
                | Tag::LosesTrade
                | Tag::HangsPiece
                | Tag::HangsPawn
        )
    }

    /// Group 9: how a good move answers a threat or a check (BASE).
    pub fn is_defence(self) -> bool {
        matches!(
            self,
            Tag::BlocksCheck
                | Tag::EvadesCheck
                | Tag::DefendsPiece
                | Tag::EscapesThreat
                | Tag::CounterThreat
        )
    }

    /// Groups 8–10: what a good move does by itself without winning anything
    /// (trades, defence, tempo). Said before a tactic that wins nothing.
    pub fn is_good_base(self) -> bool {
        (Tag::Recapture..=Tag::OffersTrade).contains(&self)
    }

    /// Group 11.
    pub fn is_sacrifice(self) -> bool {
        matches!(self, Tag::Sacrifice | Tag::OffersPiece)
    }

    /// Group 13: a positional idea of a good move (or the best move).
    pub fn is_positional(self) -> bool {
        (Tag::ForcesDoubledPawns..=Tag::CreatesLuft).contains(&self)
    }

    /// Group 14: a positional error, said about a bad move itself (BASE).
    pub fn is_positional_error(self) -> bool {
        (Tag::LosesCastling..=Tag::MovesToPassiveSquare).contains(&self)
    }

    /// A weakness forced on the opponent's pawns.
    pub fn forces_weakness(self) -> bool {
        matches!(self, Tag::ForcesDoubledPawns | Tag::ForcesIsolatedPawn)
    }

    /// Said before trades, since it is what the trade is for: doubled pawns.
    /// (An isolated pawn left by a trade is said after the trade.)
    pub fn outranks_trades(self) -> bool {
        self == Tag::ForcesDoubledPawns
    }

    /// Development, castling and the centre: what a book move is for.
    pub fn is_development(self) -> bool {
        matches!(
            self,
            Tag::StrikesAtCenter
                | Tag::PutsPawnInCenter
                | Tag::PreparesCentralPush
                | Tag::CastlesSameSide
                | Tag::CastlesOppositeSide
                | Tag::CastlesKingside
                | Tag::CastlesQueenside
                | Tag::ConnectsRooks
                | Tag::Fianchetto
                | Tag::PreparesToCastleQueenside
                | Tag::PreparesToCastleKingside
                | Tag::DevelopsCenter
                | Tag::DevelopsOpening
                | Tag::DevelopsMiddlegame
                | Tag::PreparesBishopDevelopment
        )
    }
}

/// Positional ideas of a move (group 13), best first: the weakness its
/// capture forces on the opponent's pawns (`ctx.rest` holds the opponent's
/// answer), its centre, development, activity and passed-pawn ideas.
pub fn positional_tags(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out: Vec<Found> = structure::forces_structure(ctx).into_iter().collect();
    out.extend(develop::develop_tags(ctx, phase));
    out.extend(activity::activity_tags(ctx, phase));
    out.extend(structure::structure_tags(ctx, phase));
    out.sort_by_key(|f| f.tag);
    out
}

/// Positional errors of a bad move (group 14), best first.
pub fn positional_errors(ctx: &MoveCtx<'_>, phase: Phase) -> Vec<Found> {
    let mut out = develop::opening_errors(ctx, phase);
    out.extend(structure::structure_errors(ctx));
    out.extend(activity::passive_square(ctx));
    out.sort_by_key(|f| f.tag);
    out
}

/// The weakness a capture forces on the opponent's pawns, if any (see
/// [`positional_tags`]).
pub fn forced_weakness(ctx: &MoveCtx<'_>) -> Option<Found> {
    structure::forces_structure(ctx)
}

/// One tag found on one move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub tag: Tag,
    /// The piece the sentence is about ("…your knight on f6").
    pub focus: Option<PieceAt>,
    /// Other pieces involved: fork targets, the piece behind a pin, the piece a
    /// defender guarded.
    pub targets: Vec<PieceAt>,
    pub geometry: Geometry,
    /// Material at stake (cp); 0 when the tag is not about material.
    pub stake: i32,
}

/// A move in context: the position before it, the line that follows it, and
/// the move that came just before it.
pub struct MoveCtx<'a> {
    pub before: &'a Chess,
    pub m: Move,
    pub after: &'a Chess,
    /// The other side's move that led to `before`, if known.
    pub previous: Option<Move>,
    /// The line after `m` (the other side's reply first), possibly empty.
    pub rest: &'a [Move],
    /// `m` is the first move of its line. Free-piece captures count only there:
    /// later in a line a capture just collects what a tactic won.
    pub first: bool,
}

impl MoveCtx<'_> {
    pub fn mover(&self) -> Color {
        self.before.turn()
    }

    /// The moved piece on its destination (the king for castling).
    pub fn moved(&self) -> Option<PieceAt> {
        let to = if self.m.is_castle() {
            crate::board::move_squares(self.m).1
        } else {
            self.m.to()
        };
        PieceAt::on(self.after.board(), to)
    }

    pub fn to(&self) -> Square {
        self.m.to()
    }
}

/// Tactics (groups 1–6) the move carries out for its mover, best first.
pub fn tactics(ctx: &MoveCtx<'_>) -> Vec<Found> {
    let mut out: Vec<Found> = [
        mate::checkmate(ctx),
        material::free_piece(ctx),
        material::trapped_piece(ctx),
        double::double_check(ctx),
        double::fork(ctx),
        double::discovered_check(ctx),
        double::discovered_attack(ctx),
        lines::skewer(ctx),
        lines::exploits_pin(ctx),
        lines::pin(ctx),
        defender::removes_defender(ctx),
        defender::overloaded(ctx),
        defender::attraction(ctx),
        promotion::promotion(ctx),
    ]
    .into_iter()
    .flatten()
    .collect();
    out.sort_by_key(|f| f.tag);
    out
}

/// The move gives checkmate (a back-rank mate when the pattern fits).
pub fn mate(ctx: &MoveCtx<'_>) -> Option<Found> {
    mate::checkmate(ctx)
}

/// The highest-priority tactic of the move, if any.
pub fn best_tactic(ctx: &MoveCtx<'_>) -> Option<Found> {
    tactics(ctx).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_every_tag_in_priority_order() {
        let mut sorted = Tag::ALL;
        sorted.sort();
        assert_eq!(sorted, Tag::ALL);
        assert!(Tag::Checkmate < Tag::FreePiece && Tag::FreePiece < Tag::Fork);
        assert!(Tag::Fork < Tag::Pin && Tag::Pin < Tag::HangsPiece);
    }
}
