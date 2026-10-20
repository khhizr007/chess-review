//! Board-event detectors: one move in one position → the tags it shows.
//!
//! The definitions follow chess semantics and the behaviour of the lichess
//! puzzle tagger as described in `docs/coach-research-notes.md` §2.1. They are
//! written from those descriptions, not from its code.

mod bad;
mod defence;
mod defender;
mod double;
mod lines;
mod mate;
mod material;
mod promotion;
mod tempo;
mod trade;

use serde::Serialize;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, TS)]
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
    // 12. The move as a whole.
    /// The only move that keeps a Good result: every other line loses more.
    OnlyMove,
    /// The evaluation slides a bucket (say, from equal to worse).
    Slide,
    /// Fallback: the line wins material through no named tactic.
    MaterialGain,
}

impl Tag {
    pub const ALL: [Tag; 41] = [
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
