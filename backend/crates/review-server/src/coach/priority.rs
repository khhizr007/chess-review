//! Choosing what to say about a move (`docs/coach-analysis.md` §6.5).
//!
//! A bad move is explained first by what it allows, then by what was missed;
//! for a Miss the order is the other way round. A good move is explained by
//! what it does on its own line, then by what it threatens or stops. Inside
//! each block the slots are tried in order, and inside a slot the tag order
//! decides. One main event is said, plus, for a bad move, at most one sentence
//! about the best move.

use super::circ::{Circ, Event};
use super::detect::Tag;

/// A base tag gives way to a tactic in the opponent's line that wins this
/// much more than the base tag puts at stake.
const BASE_OUTWEIGHED_CP: i32 = 200;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Selection {
    /// Index of the main event.
    pub main: Option<usize>,
    /// Index of the best-move event, said after the main one.
    pub best: Option<usize>,
}

fn first_by_tag(events: &[Event], keep: impl Fn(&Event) -> bool) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .filter(|(_, e)| keep(e))
        .min_by_key(|(_, e)| e.tag())
        .map(|(i, _)| i)
}

fn named_tactic(e: &Event) -> bool {
    !matches!(e.tag(), Tag::FreePiece | Tag::MaterialGain)
}

/// The event wins the piece a base tag is about: it takes it, or attacks it.
fn wins_same_piece(e: &Event, base: &Event) -> bool {
    base.found.focus.is_some_and(|p| {
        e.found.focus.is_some_and(|f| f.square == p.square)
            || e.found.targets.iter().any(|t| t.square == p.square)
    })
}

/// What allowing it costs: a mate; the free piece the reply takes, before a
/// threat ignored or a bad trade (the piece is simply lost); the move's own
/// error; a tactic of the reply; a free piece; any material; anything else.
///
/// The move's own error gives way to a tactic that wins much more than the
/// error puts at stake, unless that tactic just wins the same piece.
fn permits_block(events: &[Event]) -> Option<usize> {
    let permits = |e: &Event| e.first() == Circ::Permits;
    let base = first_by_tag(events, |e| e.first() == Circ::Base);
    let outweighed = |b: &Event| {
        let tactic_gain = events
            .iter()
            .filter(|e| permits(e) && !e.tag().is_mate() && !wins_same_piece(e, b))
            .filter_map(|e| e.gain.map(|g| g.cp))
            .max()
            .unwrap_or(0);
        tactic_gain >= b.found.stake + BASE_OUTWEIGHED_CP
    };
    let piece_lost =
        base.is_some_and(|i| matches!(events[i].tag(), Tag::IgnoresThreat | Tag::BadTrade));
    first_by_tag(events, |e| permits(e) && e.tag().is_mate())
        .or_else(|| {
            first_by_tag(events, |e| {
                permits(e) && e.tag() == Tag::FreePiece && e.ply == 0
            })
            .filter(|_| piece_lost)
        })
        .or_else(|| base.filter(|&i| !outweighed(&events[i])))
        .or_else(|| {
            first_by_tag(events, |e| {
                permits(e) && e.gain.is_some() && named_tactic(e)
            })
        })
        .or_else(|| first_by_tag(events, |e| permits(e) && e.tag() == Tag::FreePiece))
        .or_else(|| first_by_tag(events, |e| permits(e) && e.tag() == Tag::MaterialGain))
        .or_else(|| first_by_tag(events, permits))
}

fn misses_block(events: &[Event]) -> Option<usize> {
    let misses = |e: &Event| matches!(e.first(), Circ::Misses | Circ::MissesSame);
    first_by_tag(events, |e| misses(e) && e.tag().is_mate())
        .or_else(|| first_by_tag(events, |e| e.first() == Circ::MissesSame))
        .or_else(|| first_by_tag(events, |e| misses(e) && e.gain.is_some() && named_tactic(e)))
        .or_else(|| first_by_tag(events, |e| misses(e) && e.tag() == Tag::FreePiece))
        .or_else(|| first_by_tag(events, |e| misses(e) && e.tag() == Tag::MaterialGain))
        .or_else(|| first_by_tag(events, misses))
}

/// What to say about a bad move. `miss` puts the missed chance first. When
/// neither what it allows nor what it misses has a motif, what it threatens
/// is said (and the best move after it).
pub fn select_bad(events: &[Event], miss: bool) -> Selection {
    let permits = permits_block(events);
    let misses = misses_block(events);
    let main = if miss {
        misses.or(permits)
    } else {
        permits.or(misses)
    }
    .or_else(|| first_by_tag(events, |e| e.first() == Circ::Threatens));
    let best = match main {
        Some(i) if matches!(events[i].first(), Circ::Base | Circ::Permits) => misses,
        _ => None,
    };
    Selection { main, best }
}

/// What to say about a good move: a mate on its own line; the tactic that
/// wins material there; a mate it threatens; a mate it stops; a tactic or a
/// defence of the move itself; any threat it makes; any threat it stops; and
/// last a plain material gain on its line.
pub fn select_good(events: &[Event]) -> Selection {
    let own = |e: &Event| e.first() == Circ::Base;
    let threatens = |e: &Event| e.first() == Circ::Threatens;
    let prevents = |e: &Event| e.first() == Circ::Prevents;
    let main = first_by_tag(events, |e| own(e) && e.tag().is_mate())
        .or_else(|| {
            first_by_tag(events, |e| {
                own(e) && e.gain.is_some() && e.tag() != Tag::MaterialGain
            })
        })
        .or_else(|| first_by_tag(events, |e| threatens(e) && e.tag().is_mate()))
        .or_else(|| first_by_tag(events, |e| prevents(e) && e.tag().is_mate()))
        .or_else(|| first_by_tag(events, |e| own(e) && e.gain.is_none()))
        .or_else(|| first_by_tag(events, threatens))
        .or_else(|| first_by_tag(events, prevents))
        .or_else(|| first_by_tag(events, own));
    Selection { main, best: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::circ::{Gain, LineKind};
    use crate::coach::detect::Found;
    use crate::coach::geometry::Geometry;
    use shakmaty::Color;

    fn event(chain: &[Circ], tag: Tag, stake: i32, gain: Option<i32>) -> Event {
        event_on(chain, tag, None, stake, gain)
    }

    fn event_on(
        chain: &[Circ],
        tag: Tag,
        square: Option<shakmaty::Square>,
        stake: i32,
        gain: Option<i32>,
    ) -> Event {
        Event {
            chain: chain.to_vec(),
            found: Found {
                tag,
                focus: square.map(|square| crate::coach::facts::PieceAt {
                    role: shakmaty::Role::Queen,
                    color: Color::White,
                    square,
                }),
                targets: Vec::new(),
                geometry: Geometry::Squares(Vec::new()),
                stake,
            },
            actor: Color::White,
            line: LineKind::Reply,
            ply: 0,
            gain: gain.map(|cp| Gain {
                cp,
                role: None,
                ply: 0,
            }),
            mate_in: None,
        }
    }

    #[test]
    fn allowing_mate_comes_before_everything() {
        let events = vec![
            event(&[Circ::Base], Tag::HangsPiece, 300, Some(300)),
            event(&[Circ::Permits], Tag::Checkmate, 0, None),
            event(
                &[Circ::Misses, Circ::WinsMaterial],
                Tag::Fork,
                500,
                Some(500),
            ),
        ];
        let s = select_bad(&events, false);
        assert_eq!(s.main, Some(1));
        assert_eq!(s.best, Some(2));
    }

    #[test]
    fn a_hanging_piece_is_said_before_the_reply_that_takes_it() {
        let events = vec![
            event(
                &[Circ::Permits, Circ::WinsMaterial],
                Tag::FreePiece,
                300,
                Some(300),
            ),
            event(&[Circ::Base], Tag::HangsPiece, 300, Some(300)),
        ];
        assert_eq!(select_bad(&events, false).main, Some(1));
    }

    #[test]
    fn a_much_bigger_tactic_outweighs_a_hanging_pawn() {
        let events = vec![
            event(&[Circ::Base], Tag::HangsPawn, 100, Some(500)),
            event(&[Circ::Permits, Circ::LeadsTo], Tag::Fork, 500, Some(500)),
        ];
        assert_eq!(select_bad(&events, false).main, Some(1));
    }

    #[test]
    fn the_free_piece_comes_before_a_threat_ignored_or_a_bad_trade() {
        use shakmaty::Square;
        for tag in [Tag::IgnoresThreat, Tag::BadTrade] {
            let events = vec![
                event_on(&[Circ::Base], tag, Some(Square::A5), 900, Some(900)),
                event_on(
                    &[Circ::Permits, Circ::WinsMaterial],
                    Tag::FreePiece,
                    Some(Square::A5),
                    900,
                    Some(900),
                ),
            ];
            assert_eq!(select_bad(&events, false).main, Some(1), "{tag:?}");
        }
        // Lost in an exchange, not for free: the move's own error is said.
        let events = vec![
            event_on(
                &[Circ::Base],
                Tag::IgnoresThreat,
                Some(Square::A5),
                200,
                Some(200),
            ),
            event_on(
                &[Circ::Permits],
                Tag::MaterialGain,
                Some(Square::A5),
                200,
                Some(200),
            ),
        ];
        assert_eq!(select_bad(&events, false).main, Some(0));
    }

    #[test]
    fn a_hanging_piece_is_not_outweighed_by_winning_that_piece() {
        use shakmaty::Square;
        // The line wins more than the queen (it goes on), but the reply that
        // wins it takes the queen the move left hanging.
        let events = vec![
            event_on(
                &[Circ::Base],
                Tag::HangsPiece,
                Some(Square::G5),
                600,
                Some(1200),
            ),
            event_on(
                &[Circ::Permits, Circ::WinsMaterial],
                Tag::FreePiece,
                Some(Square::G5),
                900,
                Some(1200),
            ),
        ];
        assert_eq!(select_bad(&events, false).main, Some(0));
        // A tactic that wins something else still outweighs it.
        let events = vec![
            event_on(
                &[Circ::Base],
                Tag::HangsPiece,
                Some(Square::G5),
                300,
                Some(1200),
            ),
            event_on(
                &[Circ::Permits, Circ::WinsMaterial],
                Tag::Fork,
                Some(Square::D2),
                900,
                Some(1200),
            ),
        ];
        assert_eq!(select_bad(&events, false).main, Some(1));
    }

    #[test]
    fn a_miss_leads_with_what_was_missed() {
        let events = vec![
            event(
                &[Circ::Permits, Circ::WinsMaterial],
                Tag::Fork,
                300,
                Some(300),
            ),
            event(
                &[Circ::Misses, Circ::WinsMaterial],
                Tag::Skewer,
                500,
                Some(500),
            ),
        ];
        let s = select_bad(&events, true);
        assert_eq!(
            s,
            Selection {
                main: Some(1),
                best: None
            }
        );
        let s = select_bad(&events, false);
        assert_eq!(
            s,
            Selection {
                main: Some(0),
                best: Some(1)
            }
        );
    }

    #[test]
    fn named_tactics_come_before_free_pieces() {
        let events = vec![
            event(
                &[Circ::Misses, Circ::WinsMaterial],
                Tag::FreePiece,
                300,
                Some(300),
            ),
            event(&[Circ::Misses, Circ::LeadsTo], Tag::Pin, 300, Some(500)),
        ];
        assert_eq!(select_bad(&events, true).main, Some(1));
        // ...but the piece the opponent just hung comes first.
        let events = vec![
            event(&[Circ::MissesSame], Tag::FreePiece, 300, Some(300)),
            event(&[Circ::Misses, Circ::LeadsTo], Tag::Pin, 300, Some(500)),
        ];
        assert_eq!(select_bad(&events, true).main, Some(0));
    }
}
