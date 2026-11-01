//! The whole coach on hand-built positions and engine lines: circumstances,
//! the choice of what to say, and the realised text with its board marks.

use review_core::expected_points::ExpectedOutcome;
use review_core::{Classification, Eval, Phase, Side};
use review_server::board;
use review_server::coach::circ::Circ;
use review_server::coach::detect::Tag;
use review_server::coach::geometry::ArrowKind;
use review_server::coach::{CoachInput, CoachNote, Difficulty, LineInput, Style, coach_move};
use shakmaty::fen::Fen;
use shakmaty::{CastlingMode, Chess, Move, Square};

fn pos(fen: &str) -> Chess {
    fen.parse::<Fen>()
        .unwrap()
        .into_position(CastlingMode::Standard)
        .unwrap()
}

fn pv(moves: &[&str]) -> Vec<String> {
    moves.iter().map(|m| (*m).to_owned()).collect()
}

#[derive(Clone)]
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
    best_threat: Vec<String>,
    best_threat_eval: Eval,
    only_move: bool,
    sacrifice: Option<Square>,
    best_sacrifice: Option<Square>,
    swing: Option<(ExpectedOutcome, ExpectedOutcome)>,
    class: Classification,
    phase: Phase,
    rating: Option<i32>,
    difficulty: Option<Difficulty>,
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
            best_threat: Vec::new(),
            best_threat_eval: Eval::DRAW,
            only_move: false,
            sacrifice: None,
            best_sacrifice: None,
            swing: None,
            class: Classification::Blunder,
            phase: Phase::Middlegame,
            rating: None,
            difficulty: None,
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

    /// N_B: the null-move line after the best move.
    fn best_threat(mut self, moves: &[&str], eval: Eval) -> Case {
        self.best_threat = pv(moves);
        self.best_threat_eval = eval;
        self
    }

    fn only_move(mut self) -> Case {
        self.only_move = true;
        self
    }

    fn sacrifice(mut self, sq: Square) -> Case {
        self.sacrifice = Some(sq);
        self
    }

    fn best_sacrifice(mut self, sq: Square) -> Case {
        self.best_sacrifice = Some(sq);
        self
    }

    fn swing(mut self, from: ExpectedOutcome, to: ExpectedOutcome) -> Case {
        self.swing = Some((from, to));
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

    fn phase(mut self, phase: Phase) -> Case {
        self.phase = phase;
        self
    }

    fn rating(mut self, rating: i32) -> Case {
        self.rating = Some(rating);
        self
    }

    /// P(played) and P(best) from a human-move model.
    fn odds(mut self, played: f32, best: f32) -> Case {
        self.difficulty = Some(Difficulty { played, best });
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
            best_threat: (!self.best_threat.is_empty()).then(|| LineInput {
                pv: &self.best_threat,
                eval: self.best_threat_eval,
            }),
            only_move: self.only_move,
            sacrifice: self.sacrifice,
            best_sacrifice: self.best_sacrifice,
            swing: self.swing,
            classification: Some(self.class),
            phase: self.phase,
            rating: self.rating,
            difficulty: self.difficulty,
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
    // The pattern is named on the mating move only; the line is a forced mate.
    let main = note.main_event().unwrap();
    assert_eq!((main.tag, main.mate_in), (Tag::ForcedMate, Some(2)));
    assert_eq!(
        note.text()[0],
        "a6 allows Rd8, which starts a forced mate in 2."
    );
    assert_eq!(note.show_lines[0].moves.len(), 3);
    assert_eq!(
        note.show_lines[0].narration[2],
        "Rxd8# mates on the back rank."
    );
}

#[test]
fn a_tactic_after_a_capture_in_the_reply_is_material_won() {
    // ...a6 allows Bxc6 bxc6 Nd7, forking the queen and rook. The reply
    // starts with a trade, so the fork two moves later is not named: the
    // move allows White to win material.
    let note = Case::new("5rk1/pp6/1qn5/1B2N3/8/8/8/7K b - - 0 1", "a7a6")
        .reply(&["b5c6", "b7c6", "e5d7", "b6b5", "d7f8", "g8f8"], cp(200))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::MaterialGain, vec![Circ::Permits, Circ::LeadsTo])
    );
    assert_eq!(main.ply, 4);
    assert_eq!(
        note.text()[0],
        "a6 allows Bxc6 bxc6 Nd7 Qb5, and then Nxf8 wins the exchange."
    );
    // The show-line still narrates the fork.
    assert_eq!(
        note.show_lines[0].narration[2],
        "Nd7 forks Black's queen on b6 and rook on f8."
    );
}

#[test]
fn a_tactic_after_a_quiet_move_in_the_reply_is_material_won() {
    // ...h6 allows Kf2 Kg7, and then Nc7 forks the rooks: the first move of
    // the reply forces nothing, so the fork is too far down the line.
    let note = Case::new("r3r2k/7p/8/1N6/8/8/8/6K1 b - - 0 1", "h7h6")
        .reply(&["g1f2", "h8g7", "b5c7", "e8e7", "c7a8"], cp(500))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(main.tag, Tag::MaterialGain);
    assert_eq!(main.chain, vec![Circ::Permits, Circ::LeadsTo]);
}

#[test]
fn a_threat_ignored_is_said_as_the_free_piece_the_reply_takes() {
    // ...h6 leaves the queen on a5 to the bishop.
    let note = Case::new("4k3/7p/8/q7/8/8/3B4/6K1 b - - 0 1", "h7h6")
        .reply(&["d2a5", "e8e7"], cp(900))
        .best(&["a5a4"], cp(0))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::FreePiece, vec![Circ::Permits, Circ::WinsMaterial])
    );
    assert!(
        note.events
            .iter()
            .any(|e| e.tag == Tag::IgnoresThreat && e.chain == vec![Circ::Base])
    );
    assert_eq!(
        note.text()[0],
        "h6 allows Bxa5, which wins Black's queen on a5."
    );
}

#[test]
fn a_bad_trade_taken_back_for_free_is_a_free_piece() {
    // ...Nxd5 takes the pawn, and Bxd5 takes the knight: the queen cannot
    // take back, as Nxd5 would follow. The knight was simply lost.
    let note = Case::new("3qk3/8/5n2/3P4/2B5/2N5/8/4K3 b - - 0 1", "f6d5")
        .reply(&["c4d5", "e8e7"], cp(400))
        .best(&["e8e7"], cp(100))
        .class(Classification::Mistake)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::FreePiece, vec![Circ::Permits, Circ::WinsMaterial])
    );
    assert!(note.events.iter().any(|e| e.tag == Tag::BadTrade));
    assert_eq!(main.gain_cp, Some(200));
    assert_eq!(
        note.text()[0],
        "Nxd5 allows Bxd5, which wins Black's knight on d5."
    );
}

#[test]
fn a_threat_ignored_and_lost_in_a_line_is_said_as_losing_material() {
    // ...h6 leaves the rook on a5 to the bishop, which takes it after a
    // check: there is no free piece on the reply's first move.
    let note = Case::new("4k3/7p/8/r7/8/8/3B4/4K2Q b - - 0 1", "h7h6")
        .reply(&["h1e4", "e8f8", "d2a5", "f8g8"], cp(900))
        .best(&["a5a1"], cp(0))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::IgnoresThreat, vec![Circ::Base])
    );
    assert_eq!(
        note.text()[0],
        "h6 leaves Black's rook on a5 under attack and loses material after Qe4+."
    );
}

#[test]
fn a_hanging_queen_is_said_even_when_the_line_wins_more() {
    // ...Qg5?? hangs the queen to Bxg5, and the bishop then takes the rook on
    // d8 too. The reply wins the queen the move left hanging, so the hanging
    // queen is said, not the reply.
    let note = Case::new("3r3k/8/8/8/7q/8/8/K1B5 b - - 0 1", "h4g5")
        .reply(&["c1g5", "h8g8", "g5d8"], cp(1400))
        .best(&["h4h1"], cp(0))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::HangsPiece, vec![Circ::Base])
    );
    assert!(note.text()[0].starts_with("Qg5 leaves Black's queen on g5 hanging"));
    // The line wins more than the queen, so the reply is said as material won.
    assert!(
        note.events
            .iter()
            .any(|e| e.tag == Tag::MaterialGain && e.chain == vec![Circ::Permits])
    );
}

#[test]
fn a_forced_move_says_so() {
    // Ra1+ leaves the king one square.
    let fen = "7k/8/8/8/8/8/6P1/r6K w - - 0 1";
    let note = Case::new(fen, "h1h2").class(Classification::Forced).run();
    assert_well_formed(&note);
    assert_eq!(note.text(), vec!["Kh2 was the only legal move."]);
    assert!(note.events.is_empty() && note.show_lines.is_empty());
    assert!(!note.sentences[0].fragments[0].arrows.is_empty());
    let you = Case::new(fen, "h1h2")
        .class(Classification::Forced)
        .style(Style::You)
        .run();
    assert_eq!(you.text(), vec!["Kh2 was your only legal move."]);
}

#[test]
fn the_player_side_sets_the_style() {
    assert_eq!(
        Style::for_player(Some(Side::White), Side::White),
        Style::You
    );
    assert_eq!(
        Style::for_player(Some(Side::White), Side::Black),
        Style::Opponent
    );
    assert_eq!(Style::for_player(None, Side::Black), Style::Neutral);
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
        (Tag::Recapture, vec![Circ::Misses, Circ::WinsMaterial])
    );
    assert_eq!(best.gain_cp, Some(200));
    assert_eq!(
        note.text(),
        vec![
            "Ng4 does not take back White's rook on d7.",
            "Better was Nxd7, which takes back on d7, winning the exchange."
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
    // Rb1 attacks the knight on h1, which has no safe square (f2 is the
    // king's, g3 the pawn's); h3 was better. (h3 pushes a passed pawn, an
    // idea above a player rated 800.)
    let fen = "4k3/8/8/1R6/8/4K3/7P/7n w - - 0 1";
    let note = Case::new(fen, "b5b1")
        .reply(&["e8e7"], cp(0))
        .best(&["h2h3"], cp(30))
        .threat(&["b1h1"], cp(300))
        .class(Classification::Inaccuracy)
        .rating(800)
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
            "Rb1 threatens Rxh1, which wins Black's knight on h1.",
            "Better was h3."
        ]
    );
}

#[test]
fn attacking_a_piece_that_can_move_away_is_no_threat() {
    // Ra4 attacks the knight on h4, which simply moves: no THREATENS.
    let fen = "4k3/8/8/8/7n/8/8/R3K3 w - - 0 1";
    let note = Case::new(fen, "a1a4")
        .reply(&["h4f5"], cp(0))
        .best(&["e1e2"], cp(30))
        .threat(&["a4h4"], cp(300))
        .class(Classification::Inaccuracy)
        .run();
    assert!(note.events.iter().all(|e| e.chain[0] != Circ::Threatens));
    // As a good move it gains a tempo.
    let note = Case::new(fen, "a1a4")
        .own(&["a1a4", "h4f5"], cp(0))
        .reply(&["h4f5"], cp(0))
        .threat(&["a4h4"], cp(300))
        .class(Classification::Best)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::GainsTempo, vec![Circ::Base])
    );
    assert_eq!(
        note.text(),
        vec!["Ra4 attacks Black's knight on h4, gaining time as it moves away."]
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
        "misses_threatens",
        "misses_prevents",
        "misses_sacrifice",
        "misses_only",
        "best_threatens",
        "best_prevents",
        "best_sacrifice",
        "best_only",
        "slide",
        "sacrifice",
        "only",
        "brilliant_sacrifice",
        "great_find_only",
    ] {
        assert!(c.frame.contains_key(frame), "frame {frame}");
    }
}

// ---------------------------------------------------------------- the best move (phase 2b)

#[test]
fn the_best_move_would_have_defended() {
    // The rook on d8 attacks the knight on d4. Kf1 leaves it; c3 guards it.
    let fen = "3rk3/8/8/8/3N4/8/2P5/4K3 w - - 0 1";
    let note = Case::new(fen, "e1f1")
        .reply(&["d8d4"], cp(-300))
        .best(&["c2c3", "e8e7"], cp(0))
        .prev_threat(&["d8d4"], cp(-300))
        .class(Classification::Mistake)
        .run();
    assert_well_formed(&note);
    // The knight is simply lost: the reply that takes it is said first.
    assert_eq!(note.main_event().map(|e| e.tag), Some(Tag::FreePiece));
    let best = &note.events[note.sentences[1].event.unwrap() as usize];
    assert_eq!(
        (best.tag, best.chain.clone()),
        (Tag::DefendsPiece, vec![Circ::Misses])
    );
    assert_eq!(
        note.text()[1],
        "Better was c3, which defends White's knight on d4."
    );
    // The threat it stops is an event too, said after the defence.
    assert!(note.events.iter().any(|e| e.tag == Tag::FreePiece
        && e.chain == vec![Circ::Misses, Circ::Prevents, Circ::WinsMaterial]));
}

#[test]
fn the_best_move_would_have_threatened_mate() {
    // Rd2 would threaten Rd8 mate; White played Kf1.
    let fen = "6k1/p4ppp/8/8/8/8/R4PPP/6K1 w - - 0 1";
    let note = Case::new(fen, "g1f1")
        .reply(&["a7a5"], cp(0))
        .best(&["a2d2", "a7a5"], cp(80))
        .best_threat(&["d2d8"], mate(Side::White, 1))
        .class(Classification::Inaccuracy)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::BackRankMate, vec![Circ::Misses, Circ::Threatens])
    );
    assert_eq!(main.line, vec!["d2d8"]);
    assert_eq!(
        note.text(),
        vec!["Better was Rd2, threatening Rd8#, which mates on the back rank."]
    );
    assert_eq!(note.show_lines[0].label, "Show the threat");
    // As a Miss it is what was missed.
    let note = Case::new(fen, "g1f1")
        .reply(&["a7a5"], cp(0))
        .best(&["a2d2", "a7a5"], cp(80))
        .best_threat(&["d2d8"], mate(Side::White, 1))
        .class(Classification::Miss)
        .run();
    assert_eq!(
        note.text(),
        vec!["White missed Rd2, which threatens Rd8#: it mates on the back rank."]
    );
}

#[test]
fn the_only_good_move() {
    let note = Case::new(FORK, "d8e8")
        .reply(&["e1e2"], cp(0))
        .best(&["d8c8"], cp(0))
        .only_move()
        .class(Classification::Mistake)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::OnlyMove, vec![Circ::Misses])
    );
    assert_eq!(
        note.text(),
        vec!["Better was Kc8, the only good move here."]
    );
    // A Great Find that is the only good move says so in its own words.
    let note = Case::new(FORK, "d8c8")
        .own(&["d8c8"], cp(0))
        .reply(&["b5a7"], cp(0))
        .only_move()
        .class(Classification::GreatFind)
        .run();
    assert_eq!(
        note.text(),
        vec!["Kc8 is the one strong move here: the others all fall short."]
    );
}

#[test]
fn an_inaccuracy_with_nothing_to_name_slides() {
    let note = Case::new(FORK, "d8e8")
        .reply(&["e1e2"], cp(0))
        .best(&["d8c8"], cp(0))
        .swing(ExpectedOutcome::Equal, ExpectedOutcome::Worse)
        .class(Classification::Inaccuracy)
        .run();
    assert_well_formed(&note);
    assert_eq!(note.main_event().map(|e| e.tag), Some(Tag::Slide));
    assert_eq!(
        note.text(),
        vec![
            "Ke8 lets Black's position slide from equal to worse.",
            "Better was Kc8."
        ]
    );
    // Within one step (better and winning count as one each) nothing slides.
    let note = Case::new(FORK, "d8e8")
        .reply(&["e1e2"], cp(0))
        .best(&["d8c8"], cp(0))
        .swing(ExpectedOutcome::CompletelyWinning, ExpectedOutcome::Winning)
        .class(Classification::Inaccuracy)
        .run();
    assert!(note.main_event().is_none());
}

// ---------------------------------------------------------------- trades, checks, sacrifices

#[test]
fn a_recapture_is_a_recapture() {
    // Black has just played ...exd5; exd5 takes back. The null-move line says
    // Black would otherwise keep the pawn, but that is not how to say it.
    let prior = pos("4k3/8/4p3/3N4/4P3/8/8/4K3 b - - 0 1");
    let previous = board::parse_uci(&prior, "e6d5").unwrap();
    let mut case = Case::new("4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1", "e4d5")
        .own(&["e4d5", "e8e7"], cp(0))
        .reply(&["e8e7"], cp(0))
        .class(Classification::Best);
    case.previous = Some(previous);
    let note = case.run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Recapture, vec![Circ::Base])
    );
    assert_eq!(note.text(), vec!["exd5 takes back on d5."]);
}

#[test]
fn stepping_out_of_check() {
    let note = Case::new("4k3/8/8/8/8/3n4/8/2B1K3 w - - 0 1", "e1e2")
        .own(&["e1e2", "d3c1"], cp(-300))
        .reply(&["d3c1"], cp(-300))
        .class(Classification::Best)
        .run();
    assert_well_formed(&note);
    assert_eq!(note.main_event().map(|e| e.tag), Some(Tag::EvadesCheck));
    assert_eq!(
        note.text(),
        vec!["Ke2 gets out of the check from Black's knight on d3."]
    );
}

#[test]
fn a_brilliant_sacrifice_then_what_it_leads_to() {
    let note = Case::new("1q4k1/8/8/4N3/8/8/8/5RK1 w - - 0 1", "f1f8")
        .own(&["f1f8", "g8f8", "e5d7", "f8e7", "d7b8"], cp(400))
        .sacrifice(Square::F8)
        .class(Classification::Brilliant)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Sacrifice, vec![Circ::Base])
    );
    assert_eq!(
        note.text(),
        vec![
            "Rf8+ is a sacrifice: it gives up White's rook on f8, and that pays off.",
            "After Rf8+ Kxf8, Nd7+ forks Black's king and queen on b8, winning material worth about 4 pawns."
        ]
    );
    // The best move as a sacrifice, after a bad move.
    let note = Case::new("1q4k1/8/8/4N3/8/8/8/5RK1 w - - 0 1", "g1h1")
        .reply(&["b8b1"], cp(0))
        .best(&["f1f8", "g8f8", "e5d7", "f8e7", "d7b8"], cp(400))
        .best_sacrifice(Square::F8)
        .class(Classification::Mistake)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::Sacrifice, vec![Circ::Misses])
    );
    assert_eq!(
        note.text(),
        vec!["Better was Rf8+, a sacrifice that works."]
    );
}

#[test]
fn a_free_piece_the_line_outgrows_is_said_as_material_won() {
    // ...h6 leaves the knight on a5 to the bishop, which then takes the rook
    // on d8 as well: the reply wins more than the knight, so the move allows
    // White to win material.
    let note = Case::new("3rk3/7p/8/n7/8/8/3B4/6K1 b - - 0 1", "h7h6")
        .reply(&["d2a5", "e8e7", "a5d8", "e7d8"], cp(500))
        .best(&["a5c4"], cp(0))
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::MaterialGain, vec![Circ::Permits])
    );
    assert_eq!(main.square.as_deref(), Some("a5"));
    assert!(note.text()[0].starts_with("h6 allows Bxa5, which wins"));
}

// ---------------------------------------------------------------- positional ideas

const AFTER_E4_E5: &str = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2";
const RUY: &str = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4";

#[test]
fn a_good_move_is_said_by_its_positional_idea() {
    let note = Case::new(AFTER_E4_E5, "g1f3")
        .own(&["g1f3", "b8c6"], cp(30))
        .class(Classification::Best)
        .phase(Phase::Opening)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::DevelopsCenter, vec![Circ::Base])
    );
    assert_eq!(
        note.text(),
        vec!["Nf3 develops White's knight toward the centre."]
    );
}

#[test]
fn a_forced_weakness_comes_before_the_trade() {
    let case = Case::new(RUY, "b5c6")
        .own(&["b5c6", "b7c6"], cp(0))
        .class(Classification::Good)
        .phase(Phase::Opening);
    let note = case.run();
    assert_well_formed(&note);
    assert_eq!(note.main_event().unwrap().tag, Tag::ForcesDoubledPawns);
    assert_eq!(
        note.text(),
        vec!["Bxc6 leaves Black's pawns on c6 and c7 doubled once a pawn takes back."]
    );
    // Below the structure gate the trade is said instead.
    let note = case.clone().rating(800).run();
    assert_eq!(note.main_event().unwrap().tag, Tag::EqualTrade);
}

#[test]
fn a_bad_move_is_said_by_its_opening_error_then_the_best_moves_idea() {
    let note = Case::new(AFTER_E4_E5, "d1h5")
        .reply(&["b8c6"], cp(-20))
        .best(&["g1f3", "b8c6"], cp(30))
        .class(Classification::Inaccuracy)
        .phase(Phase::Opening)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::TooEarlyQueen, vec![Circ::Base])
    );
    assert_eq!(
        note.text(),
        vec![
            "Qh5 brings White's queen out early, before the knights and bishops.",
            "Better was Nf3, which develops White's knight toward the centre.",
        ]
    );
}

#[test]
fn a_bad_move_that_allows_doubled_pawns() {
    // ...h6 lets Bxc6 double Black's pawns.
    let fen = "r1bqkbnr/pppp1ppp/2n5/1B6/8/5N2/PPPP1PPP/RNBQK2R b KQkq - 0 1";
    let note = Case::new(fen, "h7h6")
        .reply(&["b5c6", "d7c6"], cp(40))
        .class(Classification::Inaccuracy)
        .phase(Phase::Opening)
        .run();
    assert_well_formed(&note);
    let main = note.main_event().unwrap();
    assert_eq!(
        (main.tag, main.chain.clone()),
        (Tag::ForcesDoubledPawns, vec![Circ::Permits])
    );
    assert_eq!(
        note.text()[0],
        "h6 allows Bxc6, which leaves Black's pawns on c6 and c7 doubled once a pawn takes back."
    );
}

#[test]
fn an_idea_above_the_players_rating_is_not_said() {
    let fen = "6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1";
    let case = Case::new(fen, "h2h3")
        .own(&["h2h3", "g8f8"], cp(0))
        .class(Classification::Good)
        .phase(Phase::Middlegame);
    assert_eq!(
        case.run().main_event().map(|e| e.tag),
        Some(Tag::CreatesLuft)
    );
    assert_eq!(
        case.clone().rating(1200).run().main_event().map(|e| e.tag),
        Some(Tag::CreatesLuft)
    );
    // The only idea is gated: the coach stays quiet.
    let quiet = case.rating(500).run();
    assert!(quiet.sentences.is_empty());
    assert!(quiet.events.is_empty());
}

#[test]
fn a_book_move_with_a_development_idea_gets_a_note() {
    let note = Case::new(AFTER_E4_E5, "g1f3")
        .own(&["g1f3", "b8c6"], cp(30))
        .class(Classification::Book)
        .phase(Phase::Opening)
        .run();
    assert_well_formed(&note);
    assert_eq!(
        note.text(),
        vec!["Nf3 is a book move: it develops White's knight toward the centre."]
    );
    // A book move with no such idea gets none.
    let note = Case::new(AFTER_E4_E5, "a2a3")
        .own(&["a2a3", "b8c6"], cp(0))
        .class(Classification::Book)
        .phase(Phase::Opening)
        .run();
    assert_eq!(note, CoachNote::default());
}

// ---------------------------------------------------------------- difficulty

#[test]
fn a_natural_blunder_leads_with_the_safety_check() {
    let note = Case::new(FORK, "d8e8")
        .reply(&["b5c7", "e8d7", "c7a8"], cp(500))
        .best(&["d8c8"], cp(0))
        .odds(0.45, 0.2)
        .run();
    assert_well_formed(&note);
    assert_eq!(
        note.text(),
        vec![
            "Ke8 looks natural, but check the reply first: Nc7+ forks Black's king and rook on a8, winning a rook.",
            "Better was Kc8.",
        ]
    );
}

#[test]
fn a_best_move_hard_to_find_is_said_gently() {
    let note = Case::new(FORK, "d8e8")
        .reply(&["b5c7", "e8d7", "c7a8"], cp(500))
        .best(&["d8c8"], cp(0))
        .odds(0.05, 0.04)
        .run();
    assert_eq!(
        note.text()[1],
        "Better was Kc8, though it is hard to find at this level."
    );
    // With an idea of its own.
    let note = Case::new(AFTER_E4_E5, "d1h5")
        .reply(&["b8c6"], cp(-20))
        .best(&["g1f3", "b8c6"], cp(30))
        .class(Classification::Inaccuracy)
        .phase(Phase::Opening)
        .odds(0.2, 0.05)
        .run();
    assert_eq!(
        note.text()[1],
        "Better was Nf3, a hard move to find at this level: it develops White's knight toward the centre."
    );
    // Good moves are never softened.
    let note = Case::new(AFTER_E4_E5, "g1f3")
        .own(&["g1f3", "b8c6"], cp(30))
        .class(Classification::Best)
        .phase(Phase::Opening)
        .odds(0.05, 0.05)
        .run();
    assert_eq!(
        note.text(),
        vec!["Nf3 develops White's knight toward the centre."]
    );
}
