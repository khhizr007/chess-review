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
    own: Vec<String>,
    own_eval: Eval,
    threat: Vec<String>,
    threat_eval: Eval,
    prev_threat: Vec<String>,
    prev_threat_eval: Eval,
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
            own: Vec::new(),
            own_eval: Eval::DRAW,
            threat: Vec::new(),
            threat_eval: Eval::DRAW,
            prev_threat: Vec::new(),
            prev_threat_eval: Eval::DRAW,
            class: Classification::Blunder,
            style: Style::Neutral,
            seed: 0,
        }
    }

    /// The played move's own line (the played move first).
    fn own(mut self, moves: &[&str], eval: Eval) -> Case {
        self.own = pv(moves);
        self.own_eval = eval;
        self
    }

    /// N: the null-move line after the played move.
    fn threat(mut self, moves: &[&str], eval: Eval) -> Case {
        self.threat = pv(moves);
        self.threat_eval = eval;
        self
    }

    /// N_prev: the null-move line before the played move.
    fn prev_threat(mut self, moves: &[&str], eval: Eval) -> Case {
        self.prev_threat = pv(moves);
        self.prev_threat_eval = eval;
        self
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
            played_line: (!self.own.is_empty()).then(|| LineInput {
                pv: &self.own,
                eval: self.own_eval,
            }),
            threat: (!self.threat.is_empty()).then(|| LineInput {
                pv: &self.threat,
                eval: self.threat_eval,
            }),
            prev_threat: (!self.prev_threat.is_empty()).then(|| LineInput {
                pv: &self.prev_threat,
                eval: self.prev_threat_eval,
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
fn book_moves_are_not_explained() {
    let note = Case::new(FORK, "d8c8")
        .reply(&["b5c7"], cp(0))
        .class(Classification::Book)
        .run();
    assert_eq!(note, CoachNote::default());
}

// ---------------------------------------------------------------- good moves

#[test]
fn a_good_move_is_explained_by_its_own_line() {
    let note = Case::new("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7")
        .own(&["b5c7", "e8d7", "c7a8"], cp(500))
        .reply(&["e8d7", "c7a8"], cp(500))
        .best(&["b5c7", "e8d7", "c7a8"], cp(500))
        .class(Classification::Best)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Fork, vec![Circ::Base, Circ::WinsMaterial])
    );
    assert_eq!(main.actor, Side::White);
    assert_eq!(main.line, pv(&["b5c7", "e8d7", "c7a8"]));
    // One sentence: a good move gets no "better was".
    assert_eq!(
        note.text(),
        vec!["Nc7+ forks Black's king and rook on a8, winning a rook."]
    );
    let first = &note.sentences[0].fragments[0];
    assert_eq!(first.text, "Nc7+");
    assert!(
        first
            .arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Move && a.from == "b5" && a.to == "c7")
    );
    assert!(
        first
            .arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Attack && a.from == "c7" && a.to == "a8")
    );
    assert_eq!(note.show_lines.len(), 1);
    assert_eq!(note.show_lines[0].label, "Show the fork");
}

#[test]
fn a_mating_move() {
    let note = Case::new(
        "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
        "h5f7",
    )
    .own(&["h5f7"], mate(Side::White, 1))
    .class(Classification::Best)
    .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Checkmate, vec![Circ::Base])
    );
    assert_eq!(note.text(), vec!["Qxf7# is checkmate."]);
}

// 6.Bc4 in the Opera game: the bishop joins the queen against f7.
const BEFORE_BC4: &str = "rn1qkbnr/ppp2ppp/8/4p3/4P3/5Q2/PPP2PPP/RNB1KB1R w KQkq - 0 6";

#[test]
fn threatens_mate() {
    let note = Case::new(BEFORE_BC4, "f1c4")
        .own(&["f1c4", "g8f6", "f3b3"], cp(150))
        .reply(&["g8f6", "f3b3"], cp(150))
        .threat(&["f3f7"], mate(Side::White, 1))
        .class(Classification::Best)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone(), main.mate_in),
        (Tag::Checkmate, vec![Circ::Threatens], Some(1))
    );
    assert_eq!(main.actor, Side::White);
    assert_eq!(main.line, pv(&["f3f7"]));
    assert_eq!(
        note.text(),
        vec!["Bc4 threatens Qxf7#, which is checkmate."]
    );
    // The threat is marked on the board: the queen's move and the mated king.
    let reply = note.sentences[0]
        .fragments
        .iter()
        .find(|f| f.text == "Qxf7#")
        .expect("the threat is a fragment");
    assert!(reply.arrows.iter().any(|a| a.from == "f3" && a.to == "f7"));
    assert!(reply.squares.contains(&"e8".to_owned()));
    // The threat line starts from the position with White to move again.
    let line = &note.show_lines[0];
    assert_eq!(line.label, "Show the threat");
    assert_eq!(
        line.start_fen,
        "rn1qkbnr/ppp2ppp/8/4p3/2B1P3/5Q2/PPP2PPP/RNB1K2R w KQkq - 1 6"
    );
    assert_eq!(line.narration, vec!["Qxf7# is checkmate."]);
}

// 7...Qe7 in the Opera game: Bxf7+ Kd7 Qe6# was threatened.
const BEFORE_QE7: &str = "rn1qkb1r/ppp2ppp/5n2/4p3/2B1P3/1Q6/PPP2PPP/RNB1K2R b KQkq - 3 7";

#[test]
fn prevents_mate() {
    let case = || {
        Case::new(BEFORE_QE7, "d8e7")
            .own(&["d8e7", "b1c3", "c7c6"], cp(150))
            .prev_threat(&["c4f7", "e8d7", "b3e6"], mate(Side::White, 2))
            .class(Classification::Best)
    };
    let note = case().reply(&["b1c3", "c7c6"], cp(150)).run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone(), main.mate_in),
        (Tag::ForcedMate, vec![Circ::Prevents], Some(2))
    );
    // The threat is the opponent's idea.
    assert_eq!(main.actor, Side::White);
    assert_eq!(
        note.text(),
        vec!["Qe7 stops Bxf7+, which starts a forced mate in 2."]
    );
    assert_eq!(note.show_lines[0].moves.len(), 3);
    assert_eq!(
        note.show_lines[0].start_fen,
        "rn1qkb1r/ppp2ppp/5n2/4p3/2B1P3/1Q6/PPP2PPP/RNB1K2R w KQkq - 3 7"
    );

    // When the opponent's best reply still mates, nothing was stopped.
    let note = case()
        .reply(&["c4f7", "e7f7", "b3b7"], mate(Side::White, 3))
        .run();
    assert!(note.events.iter().all(|e| e.chain[0] != Circ::Prevents));
}

#[test]
fn a_defending_move_and_the_threat_it_stops() {
    // The rook on d8 attacks the knight on d4; c3 guards it.
    let note = Case::new("3rk3/8/8/8/3N4/8/2P5/4K3 w - - 0 1", "c2c3")
        .own(&["c2c3", "e8e7"], cp(300))
        .reply(&["e8e7"], cp(300))
        .prev_threat(&["d8d4"], cp(-300))
        .class(Classification::Best)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::DefendsPiece, vec![Circ::Base])
    );
    assert_eq!(note.text(), vec!["c3 defends White's knight on d4."]);
    let piece = note.sentences[0]
        .fragments
        .iter()
        .find(|f| f.text == "White's knight on d4")
        .expect("the defended piece is a fragment");
    assert!(
        piece
            .arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Defence && a.from == "c3" && a.to == "d4")
    );
    assert!(
        piece
            .arrows
            .iter()
            .any(|a| a.kind == ArrowKind::Attack && a.from == "d8" && a.to == "d4")
    );
    // The threat it stops is an event too; a good BASE tag is said first.
    assert!(note.events.iter().any(|e| e.tag == Tag::FreePiece
        && e.chain == vec![Circ::Prevents, Circ::WinsMaterial]
        && e.actor == Side::Black));

    // Moving the knight away is an escape, also said before the stopped threat.
    let note = Case::new("3rk3/8/8/8/3N4/8/2P5/4K3 w - - 0 1", "d4b5")
        .own(&["d4b5", "e8e7"], cp(300))
        .reply(&["e8e7"], cp(300))
        .prev_threat(&["d8d4"], cp(-300))
        .class(Classification::Best)
        .run();
    assert_eq!(note.main_event().map(|e| e.tag), Some(Tag::EscapesThreat));
    assert_eq!(note.text(), vec!["Nb5 moves White's knight out of danger."]);
}

#[test]
fn a_brilliant_sacrifice_says_so() {
    let note = Case::new("1q4k1/8/8/4N3/8/8/8/5RK1 w - - 0 1", "f1f8")
        .own(&["f1f8", "g8f8", "e5d7", "f8e7", "d7b8"], cp(400))
        .class(Classification::Brilliant)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone(), main.ply),
        (Tag::RoyalFork, vec![Circ::Base, Circ::LeadsTo], 2)
    );
    assert_eq!(
        note.text(),
        vec![
            "Rf8+ is a sacrifice that works: after Rf8+ Kxf8, Nd7+ forks Black's king and queen on b8, winning material worth about 4 pawns."
        ]
    );
}

#[test]
fn a_great_find_with_nothing_to_name() {
    let note = Case::new(FORK, "d8c8")
        .reply(&["b5c7"], cp(0))
        .class(Classification::GreatFind)
        .run();
    assert_well_formed(&note);
    assert_eq!(
        note.text(),
        vec!["Kc8 is the one strong move here: the others are clearly worse."]
    );
    assert!(note.main_event().is_none());
    assert!(note.show_lines.is_empty());
}

#[test]
fn good_move_priority() {
    use review_server::coach::circ::{Event, Gain, LineKind};
    use review_server::coach::detect::Found;
    use review_server::coach::geometry::Geometry;
    use review_server::coach::priority::select_good;
    use shakmaty::Color;

    let ev = |chain: &[Circ], tag: Tag, gain: Option<i32>, line: LineKind| Event {
        chain: chain.to_vec(),
        found: Found {
            tag,
            focus: None,
            targets: Vec::new(),
            geometry: Geometry::Squares(Vec::new()),
            stake: 0,
        },
        actor: Color::White,
        line,
        ply: 0,
        gain: gain.map(|cp| Gain {
            cp,
            role: None,
            ply: 0,
        }),
        mate_in: tag.is_mate().then_some(2),
    };
    let events = [
        ev(
            &[Circ::Prevents, Circ::WinsMaterial],
            Tag::FreePiece,
            Some(300),
            LineKind::PrevThreat,
        ),
        ev(
            &[Circ::Threatens, Circ::WinsMaterial],
            Tag::Fork,
            Some(300),
            LineKind::Threat,
        ),
        ev(&[Circ::Base], Tag::DefendsPiece, None, LineKind::Played),
        ev(
            &[Circ::Prevents],
            Tag::Checkmate,
            None,
            LineKind::PrevThreat,
        ),
        ev(
            &[Circ::Threatens],
            Tag::BackRankMate,
            None,
            LineKind::Threat,
        ),
        ev(
            &[Circ::Base, Circ::WinsMaterial],
            Tag::Pin,
            Some(300),
            LineKind::Played,
        ),
        ev(&[Circ::Base], Tag::ForcedMate, None, LineKind::Played),
        ev(
            &[Circ::Base, Circ::LeadsTo],
            Tag::MaterialGain,
            Some(100),
            LineKind::Played,
        ),
    ];
    // Taking the chosen event away each time walks down the slots: own mate,
    // own tactic with a gain, threatened mate, stopped mate, own BASE tag,
    // threat, stopped threat, plain gain.
    let mut left: Vec<usize> = (0..events.len()).collect();
    let mut order = Vec::new();
    while !left.is_empty() {
        let subset: Vec<Event> = left.iter().map(|&i| events[i].clone()).collect();
        let pick = select_good(&subset).main.expect("something to say");
        order.push(left.remove(pick));
    }
    assert_eq!(order, vec![6, 5, 4, 3, 2, 1, 0, 7]);
}

// ---------------------------------------------------------------- threats of bad moves

#[test]
fn a_bad_move_with_nothing_else_to_say_names_its_threat() {
    // Ra4 attacks the knight on h4, but Ke2 was better.
    let note = Case::new("4k3/8/8/8/7n/8/8/R3K3 w - - 0 1", "a1a4")
        .reply(&["h4f5"], cp(0))
        .best(&["e1e2"], cp(30))
        .threat(&["a4h4"], cp(300))
        .class(Classification::Inaccuracy)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::FreePiece, vec![Circ::Threatens, Circ::WinsMaterial])
    );
    assert_eq!(
        note.text(),
        vec![
            "Ra4 threatens Rxh4, which wins Black's knight on h4.",
            "Better was Ke2."
        ]
    );
}

#[test]
fn no_threat_when_the_reply_takes_the_piece_or_it_was_there_already() {
    // The bishop on d7 simply takes the rook.
    let note = Case::new("4k3/3b4/8/8/7n/8/8/R3K3 w - - 0 1", "a1a4")
        .reply(&["d7a4"], cp(-500))
        .threat(&["a4h4"], cp(300))
        .class(Classification::Blunder)
        .run();
    assert!(note.events.iter().all(|e| e.chain[0] != Circ::Threatens));
    assert_eq!(note.main_event().map(|e| e.chain[0]), Some(Circ::Base));
    // The rook already attacked the knight before the king move.
    let note = Case::new("4k3/8/8/8/R6n/8/8/4K3 w - - 0 1", "e1e2")
        .reply(&["h4f5"], cp(0))
        .threat(&["a4h4"], cp(300))
        .class(Classification::Inaccuracy)
        .run();
    assert!(note.events.iter().all(|e| e.chain[0] != Circ::Threatens));
    // A plan that needs a quiet move first (Kd2, then Rxh4) is no threat.
    let note = Case::new("4k3/8/8/8/7n/8/8/R3K3 w - - 0 1", "a1a4")
        .reply(&["h4f5"], cp(0))
        .threat(&["e1d2", "e8e7", "a4h4"], cp(300))
        .class(Classification::Inaccuracy)
        .run();
    assert!(note.events.iter().all(|e| e.chain[0] != Circ::Threatens));
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
        "good",
        "good_line",
        "good_long",
        "threatens",
        "prevents",
        "brilliant",
        "great_find",
    ] {
        assert!(c.frame.contains_key(frame), "frame {frame}");
    }
}
