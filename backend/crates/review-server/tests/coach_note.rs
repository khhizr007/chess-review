//! The whole coach on hand-built positions and engine lines: circumstances,
//! the choice of what to say, and the realised text with its board marks.

use review_core::{Classification, Eval, Side};
use review_server::board;
use review_server::coach::circ::Circ;
use review_server::coach::detect::Tag;
use review_server::coach::geometry::ArrowKind;
use review_server::coach::{CoachInput, CoachNote, LineInput, Style, coach_move};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Move};

fn pos(fen: &str) -> Chess {
    fen.parse::<Fen>()
        .unwrap()
        .into_position(CastlingMode::Standard)
        .unwrap()
}

fn pv(moves: &[&str]) -> Vec<String> {
    moves.iter().map(|m| (*m).to_owned()).collect()
}

struct Case {
    fen: &'static str,
    played: &'static str,
    previous: Option<Move>,
    reply: Vec<String>,
    reply_eval: Eval,
    best: Vec<String>,
    best_eval: Eval,
    class: Classification,
    style: Style,
    seed: u32,
}

impl Case {
    fn new(fen: &'static str, played: &'static str) -> Case {
        Case {
            fen,
            played,
            previous: None,
            reply: Vec::new(),
            reply_eval: Eval::DRAW,
            best: Vec::new(),
            best_eval: Eval::DRAW,
            class: Classification::Blunder,
            style: Style::Neutral,
            seed: 0,
        }
    }

    fn reply(mut self, moves: &[&str], eval: Eval) -> Case {
        self.reply = pv(moves);
        self.reply_eval = eval;
        self
    }

    fn best(mut self, moves: &[&str], eval: Eval) -> Case {
        self.best = pv(moves);
        self.best_eval = eval;
        self
    }

    fn class(mut self, class: Classification) -> Case {
        self.class = class;
        self
    }

    fn style(mut self, style: Style) -> Case {
        self.style = style;
        self
    }

    fn run(&self) -> CoachNote {
        let before = pos(self.fen);
        let played = board::parse_uci(&before, self.played).expect("played move is legal");
        coach_move(&CoachInput {
            before: &before,
            played,
            previous: self.previous,
            reply: (!self.reply.is_empty()).then(|| LineInput {
                pv: &self.reply,
                eval: self.reply_eval,
            }),
            best: (!self.best.is_empty()).then(|| LineInput {
                pv: &self.best,
                eval: self.best_eval,
            }),
            classification: Some(self.class),
            style: self.style,
            horizon: 6,
            seed: self.seed,
        })
    }
}

fn cp(white: i32) -> Eval {
    Eval::Cp { cp: white }
}

fn mate(winner: Side, moves: u32) -> Eval {
    Eval::Mate { winner, moves }
}

/// No placeholder is left unrendered and every sentence ends properly.
fn assert_well_formed(note: &CoachNote) {
    for s in note.text() {
        assert!(
            !s.contains('{') && !s.contains('}'),
            "unrendered placeholder: {s}"
        );
        assert!(s.ends_with('.'), "sentence ends with a full stop: {s}");
        let first = s.chars().next().unwrap_or(' ');
        assert!(
            first.is_uppercase() || ('a'..='h').contains(&first),
            "capitalised or a move: {s}"
        );
    }
    for line in &note.show_lines {
        assert_eq!(line.moves.len(), line.narration.len());
        for n in &line.narration {
            assert!(!n.contains('{'), "unrendered narration: {n}");
        }
    }
}

// The fork position: after ...Ke8, Nc7+ forks the king and the rook.
const FORK: &str = "r2k4/8/8/1N6/8/8/8/4K3 b - - 0 1";

#[test]
fn permits_a_fork_that_wins_material() {
    let note = Case::new(FORK, "d8e8")
        .reply(&["b5c7", "e8d7", "c7a8"], cp(500))
        .best(&["d8c8"], cp(0))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().expect("a main event");
    assert_eq!(main.tag, Tag::Fork);
    assert_eq!(main.chain, vec![Circ::Permits, Circ::WinsMaterial]);
    assert_eq!(main.actor, Side::White);
    assert_eq!(main.gain_cp, Some(500));
    assert_eq!(main.line, pv(&["b5c7", "e8d7", "c7a8"]));
    let text = note.text();
    assert_eq!(
        text[0],
        "Ke8 allows Nc7+, which forks Black's king and rook on a8, winning a rook."
    );
    assert_eq!(text[1], "Better was Kc8.");

    // The fragment naming the fork carries the knight's move and the fork arrows.
    let marked: Vec<_> = note.sentences[0]
        .fragments
        .iter()
        .filter(|f| !f.arrows.is_empty())
        .collect();
    assert!(marked.iter().any(|f| f.text == "Nc7+"));
    let arrows = &marked[0].arrows;
    assert!(
        arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Move && a.from == "b5" && a.to == "c7")
    );
    assert!(
        arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Attack && a.from == "c7" && a.to == "a8")
    );
    assert!(
        arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Attack && a.from == "c7" && a.to == "e8")
    );

    // Show the fork, then the best line, each narrated move by move.
    assert_eq!(note.show_lines[0].label, "Show the fork");
    assert_eq!(note.show_lines[0].moves.len(), 3);
    assert_eq!(
        note.show_lines[0].narration,
        vec![
            "Nc7+ forks Black's king and rook on a8.",
            "Kd7.",
            "Nxa8 takes Black's rook on a8."
        ]
    );
    assert_eq!(note.show_lines[1].label, "Show the best line");
}

#[test]
fn styles_change_the_voice() {
    let case = Case::new(FORK, "d8e8").reply(&["b5c7", "e8d7", "c7a8"], cp(500));
    let you = case.style(Style::You).run();
    assert_eq!(
        you.text()[0],
        "Ke8 allows Nc7+, which forks your king and rook on a8, winning a rook."
    );
    let case = Case::new(FORK, "d8e8").reply(&["b5c7", "e8d7", "c7a8"], cp(500));
    let opponent = case.style(Style::Opponent).run();
    assert_eq!(
        opponent.text()[0],
        "Ke8 allows Nc7+, which forks their king and rook on a8, winning a rook."
    );
}

#[test]
fn a_miss_names_the_missed_tactic() {
    // White to move could have forked with Nc7+; a quiet king move instead.
    let case = |style| {
        Case::new("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "e1d2")
            .best(&["b5c7", "e8d7", "c7a8"], cp(500))
            .reply(&["a8a5"], cp(0))
            .class(Classification::Miss)
            .style(style)
            .run()
    };
    let note = case(Style::You);
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Fork, vec![Circ::Misses, Circ::WinsMaterial])
    );
    assert_eq!(
        note.text(),
        vec!["You missed Nc7+, which forks their king and rook on a8, winning a rook."]
    );
    assert_eq!(
        case(Style::Neutral).text(),
        vec!["White missed Nc7+, which forks Black's king and rook on a8, winning a rook."]
    );
}

#[test]
fn permits_mate() {
    // 3...Nf6?? allows Qxf7#.
    let note = Case::new(
        "r1bqkbnr/pppp1ppp/2n5/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR b KQkq - 3 3",
        "g8f6",
    )
    .reply(&["h5f7"], mate(Side::White, 1))
    .best(&["g7g6"], cp(0))
    .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(main.tag, Tag::Checkmate);
    assert_eq!(main.chain, vec![Circ::Permits]);
    assert_eq!(main.mate_in, Some(1));
    assert_eq!(note.text()[0], "Nf6 allows Qxf7#, which is checkmate.");
    assert_eq!(note.show_lines[0].label, "Show the mate");
}

#[test]
fn permits_a_back_rank_mate_in_two() {
    // ...a6 ignores the doubled rooks: Rd8 Rxd8 Rxd8#.
    let note = Case::new("4r1k1/p4ppp/8/8/8/8/3R1PPP/3R2K1 b - - 0 1", "a7a6")
        .reply(&["d2d8", "e8d8", "d1d8"], mate(Side::White, 2))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!((main.tag, main.mate_in), (Tag::BackRankMate, Some(2)));
    assert_eq!(
        note.text()[0],
        "a6 allows Rd8, which starts a back-rank mate in 2."
    );
    assert_eq!(note.show_lines[0].moves.len(), 3);
    assert_eq!(
        note.show_lines[0].narration[2],
        "Rxd8# mates on the back rank."
    );
}

#[test]
fn leads_to_a_fork_after_a_sacrifice() {
    // ...Kg8?? allows Rf8+ Kxf8 Nd7+, forking the king and queen.
    let note = Case::new("1q5k/8/8/4N3/8/8/8/5RK1 b - - 0 1", "h8g8")
        .reply(&["f1f8", "g8f8", "e5d7", "f8e7", "d7b8"], cp(400))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(main.tag, Tag::RoyalFork);
    assert_eq!(main.chain, vec![Circ::Permits, Circ::LeadsTo]);
    assert_eq!(main.ply, 2);
    assert_eq!(
        note.text()[0],
        "Kg8 allows Rf8+ Kxf8, and then Nd7+ forks Black's king and queen on b8, winning material worth about 4 pawns."
    );
    let narration = &note.show_lines[0].narration;
    assert_eq!(narration[0], "Rf8+ lures Black's king to f8.");
    assert_eq!(narration[1], "Kxf8 takes White's rook on f8.");
}

#[test]
fn a_hanging_piece_is_said_with_the_reply_that_takes_it() {
    let note = Case::new("4k3/8/5n2/8/8/8/8/3RK3 b - - 0 1", "f6d5")
        .reply(&["d1d5", "e8e7"], cp(800))
        .best(&["f6g4"], cp(500))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::HangsPiece, vec![Circ::Base])
    );
    assert_eq!(
        note.text(),
        vec![
            "Nd5 leaves Black's knight on d5 hanging, and Rxd5 wins a knight.",
            "Better was Ng4."
        ]
    );
    assert_eq!(note.show_lines[0].label, "Show the punishment");
}

#[test]
fn the_piece_just_left_hanging() {
    // White has just played Nd5??, into the pawn; Black does not take it.
    let prior = pos("4k3/8/4p3/8/8/2N5/8/4K3 w - - 0 1");
    let previous = board::parse_uci(&prior, "c3d5").unwrap();
    let mut case = Case::new("4k3/8/4p3/3N4/8/8/8/4K3 b - - 0 1", "e8d7")
        .best(&["e6d5"], cp(-300))
        .reply(&["d5f4"], cp(0))
        .class(Classification::Miss);
    case.previous = Some(previous);
    let note = case.run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(main.chain, vec![Circ::MissesSame]);
    assert_eq!(
        note.text(),
        vec!["Black missed exd5, which wins White's knight on d5: it had just been left hanging."]
    );
}

#[test]
fn taking_back_a_rook_wins_the_exchange() {
    // White has just played Rxd7, giving the rook for the knight; Black plays
    // ...Ng4 instead of taking back. The better move nets the exchange, so the
    // coach must not say ...Nxd7 "wins White's rook" as if it were hanging.
    let prior = pos("4k3/3n4/5n2/8/8/8/8/3RK3 w - - 0 1");
    let previous = board::parse_uci(&prior, "d1d7").unwrap();
    let mut case = Case::new("4k3/3R4/5n2/8/8/8/8/4K3 b - - 0 1", "f6g4")
        .reply(&["d7b7"], cp(-200))
        .best(&["f6d7"], cp(-500))
        .class(Classification::Mistake);
    case.previous = Some(previous);
    let note = case.run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::DidNotRecapture, vec![Circ::Base])
    );
    let best = &note.events[note.sentences[1].event.unwrap() as usize];
    assert_eq!(
        (best.tag, best.chain.clone()),
        (Tag::MaterialGain, vec![Circ::Misses])
    );
    assert_eq!(best.gain_cp, Some(200));
    assert_eq!(
        note.text(),
        vec![
            "Ng4 does not take back White's rook on d7.",
            "Better was Nxd7, which wins the exchange."
        ]
    );
}

#[test]
fn wording_variants_follow_the_seed() {
    let mut case = Case::new(FORK, "d8e8").reply(&["b5c7", "e8d7", "c7a8"], cp(500));
    case.seed = 1;
    assert_eq!(
        case.run().text()[0],
        "After Ke8, Nc7+ forks Black's king and rook on a8, winning a rook."
    );
}

#[test]
fn nothing_to_say_gives_an_empty_note() {
    // A quiet inaccuracy with quiet engine lines: no motif, so no sentence.
    let note = Case::new(
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
        "a7a6",
    )
    .reply(&["d2d4"], cp(60))
    .best(&["e7e5"], cp(30))
    .class(Classification::Inaccuracy)
    .run();
    assert!(note.sentences.is_empty());
    assert!(note.show_lines.is_empty());
}

#[test]
fn good_moves_are_not_explained_yet() {
    let note = Case::new(FORK, "d8c8")
        .reply(&["b5c7"], cp(0))
        .class(Classification::Best)
        .run();
    assert_eq!(note, CoachNote::default());
}

#[test]
fn every_tag_has_text() {
    use review_server::coach::realise::corpus;
    let c = corpus();
    for tag in Tag::ALL {
        let t = c
            .tag
            .get(tag.key())
            .unwrap_or_else(|| panic!("text for {tag:?}"));
        assert!(!t.vp.is_empty() && !t.label.is_empty());
    }
    for frame in [
        "base",
        "base_refuted",
        "permits",
        "permits_line",
        "misses",
        "misses_line",
        "misses_same",
        "best",
        "best_event",
        "best_line",
        "best_long",
        "permits_long",
        "misses_long",
    ] {
        assert!(c.frame.contains_key(frame), "frame {frame}");
    }
}
