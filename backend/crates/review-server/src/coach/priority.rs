//! Choosing what to say about a move (`docs/coach-analysis.md` §6.5).
//!
//! A bad move is explained first by what it allows, then by what was missed;
//! for a Miss the order is the other way round. What was missed is the best
//! move B said like a played good move. A good move is explained by what it
//! does on its own line, then by what it does by itself, threatens or stops.
//! Inside each block the slots are tried in order, and inside a slot the tag
//! order decides. One main event is said, plus, for a bad move, at most one
//! sentence about the best move, and for a Brilliant sacrifice what it leads
//! to.

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
    /// Index of a good move's second event, said after the main one: what a
    /// Brilliant sacrifice leads to.
    pub follow: Option<usize>,
}

type Slot<'a> = Box<dyn Fn(&Event) -> bool + 'a>;

fn first_by_tag(events: &[Event], keep: impl Fn(&Event) -> bool) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .filter(|(_, e)| keep(e))
        .min_by_key(|(_, e)| e.tag())
        .map(|(i, _)| i)
}

/// The first slot with an event, and inside it the first event by tag.
fn first_slot(events: &[Event], slots: &[Slot<'_>]) -> Option<usize> {
    slots.iter().find_map(|keep| first_by_tag(events, keep))
}

/// Material won by capturing a piece the opponent cannot profitably win back.
fn takes_piece(tag: Tag) -> bool {
    matches!(tag, Tag::FreePiece | Tag::WinsTrade)
}

fn named_tactic(e: &Event) -> bool {
    !takes_piece(e.tag()) && e.tag() != Tag::MaterialGain
}

/// A tactic of the move itself that wins nothing on its line.
fn bare_tactic(e: &Event) -> bool {
    e.gain.is_none()
        && !e.tag().is_mate()
        && !e.tag().is_good_base()
        && !e.tag().is_sacrifice()
        && !matches!(e.tag(), Tag::OnlyMove | Tag::Slide | Tag::MaterialGain)
}

/// The event wins the piece a base tag is about: it takes it, or attacks it.
fn wins_same_piece(e: &Event, base: &Event) -> bool {
    base.found.focus.is_some_and(|p| {
        e.found.focus.is_some_and(|f| f.square == p.square)
            || e.found.targets.iter().any(|t| t.square == p.square)
    })
}

/// What allowing it costs: a mate; for a threat ignored or a bad trade, the
/// reply that takes the piece (a free piece, or the material won with it);
/// the move's own error; a tactic of the reply; a free piece; any material;
/// anything else.
///
/// The move's own error gives way to a tactic that wins much more than the
/// error puts at stake, unless that tactic just wins the same piece.
fn permits_block(events: &[Event]) -> Option<usize> {
    let permits = |e: &Event| e.first() == Circ::Permits;
    let base = first_by_tag(events, |e| e.first() == Circ::Base && e.tag().is_bad_base());
    let outweighed = |b: &Event| {
        let tactic_gain = events
            .iter()
            .filter(|e| permits(e) && !e.tag().is_mate() && !wins_same_piece(e, b))
            .filter_map(|e| e.gain.map(|g| g.cp))
            .max()
            .unwrap_or(0);
        tactic_gain >= b.found.stake + BASE_OUTWEIGHED_CP
    };
    // A threat ignored or a bad trade is said by the reply that takes the
    // piece: a free piece, or the material the line wins with it.
    let lost_piece =
        base.filter(|&i| matches!(events[i].tag(), Tag::IgnoresThreat | Tag::BadTrade));
    let takes_it = |e: &Event| {
        permits(e)
            && e.ply == 0
            && (e.tag() == Tag::FreePiece
                || (e.tag() == Tag::MaterialGain
                    && lost_piece.is_some_and(|b| wins_same_piece(e, &events[b]))))
    };
    first_by_tag(events, |e| permits(e) && e.tag().is_mate())
        .or_else(|| first_by_tag(events, takes_it).filter(|_| lost_piece.is_some()))
        .or_else(|| base.filter(|&i| !outweighed(&events[i])))
        .or_else(|| {
            first_by_tag(events, |e| {
                permits(e) && e.gain.is_some() && named_tactic(e)
            })
        })
        .or_else(|| first_by_tag(events, |e| permits(e) && takes_piece(e.tag())))
        .or_else(|| first_by_tag(events, |e| permits(e) && e.tag() == Tag::MaterialGain))
        .or_else(|| first_by_tag(events, permits))
}

/// What the best move B would have done, said like a played good move: a
/// mate on its line; the piece just left hanging; a sacrifice; the tactic
/// that wins material; a piece it takes; a mate it threatens or stops; what
/// it does by itself (a trade, a defence, a tempo); a tactic that wins
/// nothing; any threat; any threat it stops; a plain material gain; and last
/// that it was the only good move. This follows the good-move order of
/// [`select_good`], with the piece just left hanging added.
fn misses_block(events: &[Event]) -> Option<usize> {
    let misses = |e: &Event| matches!(e.first(), Circ::Misses | Circ::MissesSame);
    // The circumstance after MISSES says whose line it is: B's own, or the
    // null-move lines of what it threatens and stops.
    let then = |e: &Event| e.chain.get(1).copied();
    let own =
        move |e: &Event| misses(e) && !matches!(then(e), Some(Circ::Threatens | Circ::Prevents));
    let threat = move |e: &Event| misses(e) && then(e) == Some(Circ::Threatens);
    let stops = move |e: &Event| misses(e) && then(e) == Some(Circ::Prevents);
    let slots: [Slot<'_>; 13] = [
        Box::new(move |e| own(e) && e.tag().is_mate()),
        Box::new(|e| e.first() == Circ::MissesSame),
        Box::new(move |e| own(e) && e.tag().is_sacrifice()),
        Box::new(move |e| own(e) && e.gain.is_some() && named_tactic(e)),
        Box::new(move |e| own(e) && takes_piece(e.tag())),
        Box::new(move |e| threat(e) && e.tag().is_mate()),
        Box::new(move |e| stops(e) && e.tag().is_mate()),
        Box::new(move |e| own(e) && e.tag().is_good_base()),
        Box::new(move |e| own(e) && bare_tactic(e)),
        Box::new(threat),
        Box::new(stops),
        Box::new(move |e| own(e) && e.tag() == Tag::MaterialGain),
        Box::new(move |e| misses(e) && e.tag() == Tag::OnlyMove),
    ];
    first_slot(events, &slots)
}

/// What to say about a bad move. `miss` puts the missed chance first. When
/// neither what it allows nor what it misses has a motif, what it threatens is
/// said; then that the best move was the only good one; then, last, that the
/// evaluation slid. The best move is said after any main event that is not
/// already about it.
pub fn select_bad(events: &[Event], miss: bool) -> Selection {
    let permits = permits_block(events);
    let misses = misses_block(events);
    // "The only good move" is said only when nothing else is.
    let missed_idea = misses.filter(|&i| events[i].tag() != Tag::OnlyMove);
    let main = if miss {
        missed_idea.or(permits)
    } else {
        permits.or(missed_idea)
    }
    .or_else(|| first_by_tag(events, |e| e.first() == Circ::Threatens))
    .or(misses)
    .or_else(|| first_by_tag(events, |e| e.tag() == Tag::Slide));
    let best = match main {
        Some(i) if matches!(events[i].first(), Circ::Misses | Circ::MissesSame) => None,
        Some(_) => misses,
        None => None,
    };
    Selection {
        main,
        best,
        follow: None,
    }
}

/// What to say about a good move: a mate on its own line; a Brilliant
/// sacrifice (then what it leads to); the tactic that wins material there; a
/// mate it threatens; a mate it stops; what it does by itself (a trade, a
/// defence, a tempo); a tactic that wins nothing; any threat it makes; any
/// threat it stops; a plain material gain on its line; and last that it was
/// the only good move.
pub fn select_good(events: &[Event]) -> Selection {
    let own = |e: &Event| e.first() == Circ::Base;
    let threatens = |e: &Event| e.first() == Circ::Threatens;
    let prevents = |e: &Event| e.first() == Circ::Prevents;
    let mut slots: Vec<Slot<'_>> = vec![
        Box::new(move |e| own(e) && e.tag().is_mate()),
        Box::new(move |e| own(e) && e.tag().is_sacrifice()),
        Box::new(move |e| own(e) && e.gain.is_some() && e.tag() != Tag::MaterialGain),
        Box::new(move |e| threatens(e) && e.tag().is_mate()),
        Box::new(move |e| prevents(e) && e.tag().is_mate()),
        Box::new(move |e| own(e) && e.tag().is_good_base()),
        Box::new(move |e| own(e) && bare_tactic(e)),
        Box::new(threatens),
        Box::new(prevents),
        Box::new(move |e| own(e) && e.tag() == Tag::MaterialGain),
        Box::new(move |e| own(e) && e.tag() == Tag::OnlyMove),
    ];
    let main = first_slot(events, &slots);
    // A sacrifice is said with what it leads to.
    let follow = match main {
        Some(i) if events[i].tag().is_sacrifice() => {
            drop(slots.remove(1));
            first_slot(events, &slots).filter(|&j| events[j].tag() != Tag::OnlyMove)
        }
        _ => None,
    };
    Selection {
        main,
        best: None,
        follow,
    }
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
        // Taken in an exchange: the reply's material on that piece is said.
        let mut events = vec![
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
        assert_eq!(select_bad(&events, false).main, Some(1));
        // The reply wins something else, or wins the piece later in its line:
        // the move's own error is said.
        events[1].found.focus.as_mut().unwrap().square = Square::B5;
        assert_eq!(select_bad(&events, false).main, Some(0));
        events[1].found.focus.as_mut().unwrap().square = Square::A5;
        events[1].ply = 2;
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
                best: None,
                follow: None,
            }
        );
        let s = select_bad(&events, false);
        assert_eq!(
            s,
            Selection {
                main: Some(0),
                best: Some(1),
                follow: None,
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

    #[test]
    fn the_best_move_is_said_like_a_good_move() {
        // B defends a piece and threatens to win one: the defence comes first,
        // and either comes before "the only good move".
        let events = vec![
            event(&[Circ::Misses], Tag::OnlyMove, 0, None),
            event(
                &[Circ::Misses, Circ::Threatens, Circ::WinsMaterial],
                Tag::FreePiece,
                300,
                Some(300),
            ),
            event(&[Circ::Misses], Tag::DefendsPiece, 300, None),
        ];
        assert_eq!(select_bad(&events, true).main, Some(2));
        assert_eq!(select_bad(&events[..2], true).main, Some(1));
        // After a base error, the best move's idea is the second sentence.
        let mut with_base = events.clone();
        with_base.push(event(&[Circ::Base], Tag::HangsPiece, 300, Some(300)));
        let s = select_bad(&with_base, false);
        assert_eq!((s.main, s.best), (Some(3), Some(2)));
    }

    #[test]
    fn the_only_move_and_the_slide_are_fallbacks() {
        let events = vec![
            event(&[Circ::Base], Tag::Slide, 0, None),
            event(&[Circ::Misses], Tag::OnlyMove, 0, None),
        ];
        assert_eq!(select_bad(&events, false).main, Some(1));
        assert_eq!(select_bad(&events[..1], false).main, Some(0));
        // A threat of the played move comes before both.
        let mut events = events;
        events.push(event(
            &[Circ::Threatens, Circ::WinsMaterial],
            Tag::Fork,
            300,
            Some(300),
        ));
        let s = select_bad(&events, false);
        assert_eq!((s.main, s.best), (Some(2), Some(1)));
    }

    #[test]
    fn trades_and_defences_come_before_bare_tactics() {
        let events = vec![
            event(&[Circ::Base], Tag::RemovesDefender, 0, None),
            event(
                &[Circ::Prevents, Circ::WinsMaterial],
                Tag::FreePiece,
                300,
                Some(300),
            ),
            event(&[Circ::Base], Tag::Recapture, 0, None),
        ];
        assert_eq!(select_good(&events).main, Some(2));
        assert_eq!(select_good(&events[..2]).main, Some(0));
    }

    #[test]
    fn a_sacrifice_is_said_with_what_it_leads_to() {
        let events = vec![
            event(&[Circ::Base, Circ::LeadsTo], Tag::Fork, 500, Some(500)),
            event(&[Circ::Base], Tag::Sacrifice, 0, None),
        ];
        let s = select_good(&events);
        assert_eq!((s.main, s.follow), (Some(1), Some(0)));
        // A mate is still said first.
        let events = vec![
            event(&[Circ::Base], Tag::Sacrifice, 0, None),
            event(&[Circ::Base], Tag::ForcedMate, 0, None),
        ];
        assert_eq!(select_good(&events).main, Some(1));
    }
}
