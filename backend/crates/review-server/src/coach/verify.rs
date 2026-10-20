//! Fact check of what the coach says (`docs/coach-analysis.md` §8 item 3).
//!
//! The checker reads a [`CoachNote`] the way a user does: as text. Each
//! sentence is parsed back against the frames and tag phrases of
//! `text/en.toml`, which tells which words are the move, the line, the reply,
//! a piece, a gain or a number of moves. Every claim that can be checked on
//! the board is then checked, independently of the detectors that produced
//! it:
//!
//! - **pieces:** a named piece stands on its named square in the position the
//!   sentence refers to: before the move for what it "was" or takes, after it
//!   for what it creates, allows or attacks;
//! - **moves:** every SAN is legal in sequence from where the sentence starts
//!   it (the played move, the best move, the reply line, a null-move threat),
//!   is written as SAN writes it, and follows the engine line of its event;
//! - **material:** a stated gain occurs on the event's line;
//! - **mate:** a stated mate in N is a mate within N moves on the line;
//! - **geometry:** forks, pins, skewers, discovered attacks and the sentence's
//!   attack and defence arrows match real attacks and x-rays;
//! - **show-lines:** their moves are legal from their start, which is a
//!   position the note is about, and each narration names its own move and
//!   says only true things about it.
//!
//! A sentence the frames cannot produce is a failure too (`Unparsed`): that is
//! what wording held to a fact list must never do. Claims about the engine's
//! judgement ("the only good move", "hard to find", "slide from equal to
//! worse") are not board facts and are not checked.

pub mod mutate;

use std::collections::BTreeMap;

use serde::Serialize;
use shakmaty::fen::Fen;
use shakmaty::san::SanPlus;
use shakmaty::{
    Bitboard, CastlingMode, CastlingSide, Chess, Color, Move, Piece, Position, Role, Square,
    attacks,
};

use super::circ::Circ;
use super::detect::Tag;
use super::geometry::{Arrow, ArrowKind};
use super::realise::corpus;
use super::{CoachEvent, CoachNote, ShowLine, Style};
use crate::board;

/// What kind of claim is false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The text is not one the frames of `en.toml` can produce.
    Unparsed,
    /// "You", "White" or an owner word names the wrong side.
    Side,
    /// A named piece is not on its named square.
    Piece,
    /// A SAN is illegal, misspelt, or not the move or line it stands for.
    Move,
    /// A stated material gain does not occur on the line.
    Material,
    /// A stated mate does not occur within its moves on the line.
    Mate,
    /// A fork, pin, skewer, discovered attack or arrow has no real attack.
    Geometry,
    /// A show-line narration does not name its move, or says something false.
    Narration,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::Unparsed,
        Kind::Side,
        Kind::Piece,
        Kind::Move,
        Kind::Material,
        Kind::Mate,
        Kind::Geometry,
        Kind::Narration,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Unparsed => "unparsed",
            Kind::Side => "side",
            Kind::Piece => "piece",
            Kind::Move => "move",
            Kind::Material => "material",
            Kind::Mate => "mate",
            Kind::Geometry => "geometry",
            Kind::Narration => "narration",
        }
    }
}

/// Where in the note a claim is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Sentence(usize),
    ShowLine(usize),
    Narration { line: usize, ply: usize },
}

/// One false claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    pub kind: Kind,
    pub place: Place,
    /// The words that make the claim.
    pub claim: String,
    /// What the board says instead.
    pub detail: String,
}

/// The result of checking one note.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    pub sentences: usize,
    /// Sentences with at least one false claim.
    pub false_sentences: usize,
    pub narrations: usize,
    /// Narrations with at least one false claim.
    pub false_narrations: usize,
    pub show_lines: usize,
    /// Claims whose proof lies past the end of the line (a mate longer than
    /// the line the coach kept): neither true nor false here.
    pub unchecked: Vec<Failure>,
    pub failures: Vec<Failure>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }

    /// Some claim at `place` is false.
    pub fn fails_at(&self, place: Place) -> bool {
        self.failures.iter().any(|f| f.place == place)
    }

    /// Adds another report's counts and failures.
    pub fn add(&mut self, other: Report) {
        self.sentences += other.sentences;
        self.false_sentences += other.false_sentences;
        self.narrations += other.narrations;
        self.false_narrations += other.false_narrations;
        self.show_lines += other.show_lines;
        self.unchecked.extend(other.unchecked);
        self.failures.extend(other.failures);
    }

    /// Failures per kind.
    pub fn by_kind(&self) -> BTreeMap<Kind, usize> {
        let mut out = BTreeMap::new();
        for f in &self.failures {
            *out.entry(f.kind).or_insert(0) += 1;
        }
        out
    }
}

/// One reviewed move and what the coach said about it.
pub struct VerifyInput<'a> {
    pub note: &'a CoachNote,
    pub before: &'a Chess,
    pub played: Move,
    /// The opponent's move that led to `before`.
    pub previous: Option<Move>,
    /// The engine's best line from `before` (UCI); empty when unknown.
    pub best: &'a [String],
    pub style: Style,
}

/// One reviewed move given as text (FEN and UCI), as a stored review has it.
pub struct PlyText<'a> {
    pub fen_before: &'a str,
    pub uci: &'a str,
    /// The previous ply's position and move.
    pub previous: Option<(&'a str, &'a str)>,
    pub best: &'a [String],
    pub note: &'a CoachNote,
    pub style: Style,
}

fn parse_fen(fen: &str) -> Result<Chess, String> {
    fen.parse::<Fen>()
        .map_err(|e| format!("{fen}: {e}"))?
        .into_position(CastlingMode::Standard)
        .map_err(|e| format!("{fen}: {e}"))
}

/// Checks a stored ply. `Err` when its own FEN or move does not parse.
pub fn verify_text(ply: &PlyText<'_>) -> Result<Report, String> {
    let before = parse_fen(ply.fen_before)?;
    let played = board::parse_uci(&before, ply.uci)
        .ok_or_else(|| format!("{} is not legal in {}", ply.uci, ply.fen_before))?;
    let previous = match ply.previous {
        Some((fen, uci)) => Some(
            board::parse_uci(&parse_fen(fen)?, uci)
                .ok_or_else(|| format!("{uci} is not legal in {fen}"))?,
        ),
        None => None,
    };
    Ok(verify(&VerifyInput {
        note: ply.note,
        before: &before,
        played,
        previous,
        best: ply.best,
        style: ply.style,
    }))
}

/// Checks every claim of a note.
pub fn verify(input: &VerifyInput<'_>) -> Report {
    let ctx = Ctx::new(input);
    let mut report = Report::default();
    for (i, s) in input.note.sentences.iter().enumerate() {
        report.sentences += 1;
        let text: String = s.fragments.iter().map(|f| f.text.as_str()).collect();
        let arrows: Vec<&Arrow> = s.fragments.iter().flat_map(|f| &f.arrows).collect();
        let event = match s.event {
            Some(e) => match input.note.events.get(e as usize) {
                Some(e) => Some(e),
                None => {
                    report.failures.push(Failure {
                        kind: Kind::Unparsed,
                        place: Place::Sentence(i),
                        claim: text,
                        detail: format!("points to event {e}, which the note does not have"),
                    });
                    report.false_sentences += 1;
                    continue;
                }
            },
            None => None,
        };
        let (failures, unchecked) = ctx.sentence(&text, &arrows, event);
        report
            .unchecked
            .extend(unchecked.into_iter().map(|(kind, claim, detail)| Failure {
                kind,
                place: Place::Sentence(i),
                claim,
                detail,
            }));
        if !failures.is_empty() {
            report.false_sentences += 1;
        }
        report
            .failures
            .extend(failures.into_iter().map(|(kind, claim, detail)| Failure {
                kind,
                place: Place::Sentence(i),
                claim,
                detail,
            }));
    }
    for (i, line) in input.note.show_lines.iter().enumerate() {
        report.show_lines += 1;
        ctx.show_line(i, line, &mut report);
    }
    report
}

// ---------------------------------------------------------------- positions

/// The positions a sentence or line can start from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Start {
    Before,
    After,
    /// The position before the move with the opponent to move (N_prev).
    NullBefore,
    /// The position after the move with its mover to move again (N).
    NullAfter,
    /// The position after the best move with its mover to move again (N_B).
    NullBest,
}

/// A line played out on the board.
#[derive(Debug, Clone)]
struct Walk {
    moves: Vec<Move>,
    /// `positions[k]` is the position before `moves[k]`; one more at the end.
    positions: Vec<Chess>,
    /// The move that led to the first position, when there is one.
    lead: Option<Move>,
}

impl Walk {
    fn empty(start: &Chess, lead: Option<Move>) -> Walk {
        Walk {
            moves: Vec::new(),
            positions: vec![start.clone()],
            lead,
        }
    }

    fn push(&mut self, m: Move) {
        let mut pos = self.positions.last().expect("a walk has a start").clone();
        pos.play_unchecked(m);
        self.moves.push(m);
        self.positions.push(pos);
    }

    fn last(&self) -> &Chess {
        self.positions.last().expect("a walk has a start")
    }

    /// The move before `moves[k]`.
    fn previous(&self, k: usize) -> Option<Move> {
        if k == 0 {
            self.lead
        } else {
            self.moves.get(k - 1).copied()
        }
    }

    /// Plays UCI moves; `Err` with the first that is illegal.
    fn uci(start: &Chess, lead: Option<Move>, uci: &[String]) -> Result<Walk, String> {
        let mut walk = Walk::empty(start, lead);
        for u in uci {
            match board::parse_uci(walk.last(), u) {
                Some(m) => walk.push(m),
                None => return Err(u.clone()),
            }
        }
        Ok(walk)
    }

    fn facts(&self, k: usize) -> MoveFacts<'_> {
        MoveFacts {
            before: &self.positions[k],
            m: self.moves[k],
            after: &self.positions[k + 1],
            previous: self.previous(k),
            next: &self.positions[k + 2..],
        }
    }
}

/// The SAN of `m` in `pos`, with its check suffix.
fn san(pos: &Chess, m: Move) -> String {
    SanPlus::from_move(pos.clone(), m).to_string()
}

/// A SAN token in `pos`: the move, or why it is not one.
fn parse_san(pos: &Chess, token: &str) -> Result<Move, String> {
    let parsed =
        SanPlus::from_ascii(token.as_bytes()).map_err(|_| format!("{token} is not SAN"))?;
    let m = parsed
        .san
        .to_move(pos)
        .map_err(|_| format!("{token} is not legal in {}", board::fen(pos)))?;
    let written = san(pos, m);
    if written != token {
        return Err(format!("{token} is written {written}"));
    }
    Ok(m)
}

fn is_san_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b"abcdefghNBRQKO012345678x=+#-".contains(&b))
        && SanPlus::from_ascii(s.as_bytes()).is_ok()
}

/// Where a castling rook lands.
fn castle_rook(m: Move) -> Option<Square> {
    let Move::Castle { king, rook } = m else {
        return None;
    };
    let file = if rook.file() > king.file() {
        shakmaty::File::F
    } else {
        shakmaty::File::D
    };
    Some(Square::from_coords(file, king.rank()))
}

/// The position the move lands the moved piece on (the king for castling).
fn moved_square(m: Move) -> Square {
    board::move_squares(m).1
}

fn same_squares(m: Move, a: &Arrow) -> bool {
    let (from, to) = board::move_squares(m);
    from.is_some_and(|f| f.to_string() == a.from) && to.to_string() == a.to
}

// ---------------------------------------------------------------- attacks

fn piece_attacks(pos: &Chess, from: Square, occupied: Bitboard) -> Bitboard {
    pos.board()
        .piece_at(from)
        .map_or(Bitboard::EMPTY, |p| attacks::attacks(from, p, occupied))
}

/// The piece on `from` attacks `to`.
fn attacks_sq(pos: &Chess, from: Square, to: Square) -> bool {
    piece_attacks(pos, from, pos.board().occupied()).contains(to)
}

/// The piece on `from` attacks `to` through at most one piece in between.
fn xrays(pos: &Chess, from: Square, to: Square) -> bool {
    if attacks_sq(pos, from, to) {
        return true;
    }
    let occupied = pos.board().occupied();
    let between = attacks::between(from, to) & occupied;
    between.count() == 1
        && between.first().is_some_and(|b| {
            piece_attacks(pos, from, occupied & !Bitboard::from_square(b)).contains(to)
        })
}

/// `by` attacks `sq`.
fn attacked_by(pos: &Chess, sq: Square, by: Color) -> bool {
    pos.board().attacks_to(sq, by, pos.board().occupied()).any()
}

/// A slider on `from` attacks `front`, and `behind` stands next on the same
/// ray (a pin or a skewer).
fn through(pos: &Chess, from: Square, front: Square, behind: Square) -> bool {
    let occupied = pos.board().occupied();
    attacks_sq(pos, from, front)
        && attacks::between(from, behind).contains(front)
        && piece_attacks(pos, from, occupied & !Bitboard::from_square(front)).contains(behind)
}

fn pawn_passed(pos: &Chess, sq: Square, color: Color) -> bool {
    let theirs = pos.board().pawns() & pos.board().by_color(color.other());
    !theirs.into_iter().any(|p| {
        let file_near = (i32::from(p.file()) - i32::from(sq.file())).abs() <= 1;
        let ahead = match color {
            Color::White => p.rank() > sq.rank(),
            Color::Black => p.rank() < sq.rank(),
        };
        file_near && ahead
    })
}

fn pawn_isolated(pos: &Chess, sq: Square, color: Color) -> bool {
    let ours = pos.board().pawns() & pos.board().by_color(color);
    !ours
        .into_iter()
        .any(|p| (i32::from(p.file()) - i32::from(sq.file())).abs() == 1)
}

fn relative_rank(sq: Square, color: Color) -> u32 {
    let r = u32::from(sq.rank());
    match color {
        Color::White => r,
        Color::Black => 7 - r,
    }
}

// ---------------------------------------------------------------- words

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    White,
    Black,
    Your,
    Their,
}

/// A piece as the text names it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Named {
    owner: Owner,
    role: Role,
    square: Option<Square>,
    text: String,
}

fn owner_word(s: &str) -> Option<Owner> {
    let w = &corpus().words;
    if s == w.white_s {
        Some(Owner::White)
    } else if s == w.black_s {
        Some(Owner::Black)
    } else if s == w.your {
        Some(Owner::Your)
    } else if s == w.their {
        Some(Owner::Their)
    } else {
        None
    }
}

fn role_word(s: &str) -> Option<Role> {
    let w = &corpus().words;
    Role::ALL.into_iter().find(|r| w.role(*r) == s)
}

/// "Black's knight on f6", "your king", "White's rook" (`square` false).
fn parse_piece(s: &str, square: bool) -> Option<Named> {
    let mut words = s.split(' ');
    let owner = owner_word(words.next()?)?;
    let role = role_word(words.next()?)?;
    let rest: Vec<&str> = words.collect();
    let sq = match rest.as_slice() {
        [] if !square || role == Role::King => None,
        ["on", sq] if square && role != Role::King => Some(sq.parse().ok()?),
        _ => return None,
    };
    Some(Named {
        owner,
        role,
        square: sq,
        text: s.to_owned(),
    })
}

/// A list of pieces: "your queen on d8 and rook on h8", "Black's pawns on b7
/// and f7", "White's king and Black's queen on d8".
fn parse_targets(s: &str) -> Option<Vec<Named>> {
    let w = &corpus().words;
    let items: Vec<&str> = s
        .split(w.list_comma.as_str())
        .flat_map(|p| p.split(w.list_and.as_str()))
        .collect();
    let first = items.first()?;
    // "Black's pawns on b7 and f7".
    let mut words = first.split(' ');
    if let (Some(o), Some(roles), Some("on"), Some(sq), None) = (
        words.next().and_then(owner_word),
        words.next(),
        words.next(),
        words.next(),
        words.next(),
    ) && let Some(role) = roles.strip_suffix('s').and_then(role_word)
    {
        let mut out = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let sq: Square = if i == 0 { sq } else { item }.parse().ok()?;
            out.push(Named {
                owner: o,
                role,
                square: Some(sq),
                text: s.to_owned(),
            });
        }
        return (out.len() >= 2).then_some(out);
    }
    let mut out: Vec<Named> = Vec::new();
    for item in items {
        let named = match parse_piece(item, true) {
            Some(n) => n,
            None => {
                // The owner is said once per run of the same owner.
                let owner = out.last()?.owner;
                let mut words = item.split(' ');
                let role = role_word(words.next()?)?;
                let rest: Vec<&str> = words.collect();
                let square = match rest.as_slice() {
                    [] if role == Role::King => None,
                    ["on", sq] if role != Role::King => Some(sq.parse().ok()?),
                    _ => return None,
                };
                Named {
                    owner,
                    role,
                    square,
                    text: item.to_owned(),
                }
            }
        };
        out.push(named);
    }
    Some(out)
}

/// The smallest gain (cp) the words promise: "a knight" is more than two
/// pawns, "about 3 pawns" at least two and a half.
fn parse_gain(s: &str) -> Option<i32> {
    let w = &corpus().words;
    for role in [
        Role::Pawn,
        Role::Knight,
        Role::Bishop,
        Role::Rook,
        Role::Queen,
    ] {
        if s == w.gain_piece.replace("{role}", w.role(role)) {
            return Some(match role {
                Role::Pawn => 100,
                r => board::role_value(r) - 99,
            });
        }
    }
    if s == w.gain_exchange || s == w.gain_pawns.replace("{count}", &w.numbers[2]) {
        return Some(150);
    }
    let (head, tail) = w.gain_material.split_once("{count}")?;
    let n: i32 = s.strip_prefix(head)?.strip_suffix(tail)?.parse().ok()?;
    Some(n * 100 - 50)
}

fn valid_slot(name: &str, s: &str) -> bool {
    let w = &corpus().words;
    match name {
        "move" | "best" | "reply" | "san" => is_san_token(s),
        "line" => s.split(' ').all(is_san_token),
        "piece" => parse_piece(s, true).is_some(),
        "piece_role" => parse_piece(s, false).is_some(),
        "targets" => parse_targets(s).is_some(),
        "square" => s.len() == 2 && s.parse::<Square>().is_ok(),
        "gain" => parse_gain(s).is_some(),
        "n" => !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()),
        "mover" => [&w.you, &w.opponent, &w.white, &w.black]
            .iter()
            .any(|x| x.as_str() == s),
        "mover_s" => [&w.your, &w.their, &w.white_s, &w.black_s]
            .iter()
            .any(|x| x.as_str() == s),
        "from" | "to" => {
            let o = &corpus().outlook;
            [&o.losing, &o.worse, &o.equal, &o.better, &o.winning]
                .iter()
                .any(|x| x.as_str() == s)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------- templates

#[derive(Debug, Clone)]
enum Tok {
    Lit(String),
    Slot(String),
}

fn tokenize(template: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        if open > 0 {
            out.push(Tok::Lit(rest[..open].to_owned()));
        }
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push(Tok::Slot(rest[open + 1..open + close].to_owned()));
        rest = &rest[open + close + 1..];
    }
    if !rest.is_empty() {
        out.push(Tok::Lit(rest.to_owned()));
    }
    out
}

type Bindings = Vec<(String, String)>;

/// Matches `text` against a template, binding every placeholder to its words.
fn match_template(toks: &[Tok], text: &str) -> Option<Bindings> {
    fn rec(toks: &[Tok], text: &str, pos: usize, out: &mut Bindings) -> bool {
        let Some(tok) = toks.first() else {
            return pos == text.len();
        };
        match tok {
            Tok::Lit(l) => {
                text[pos..].starts_with(l.as_str()) && rec(&toks[1..], text, pos + l.len(), out)
            }
            Tok::Slot(name) => {
                let ends: Vec<usize> = match toks.get(1) {
                    None => vec![text.len()],
                    Some(Tok::Lit(l)) => text[pos..]
                        .match_indices(l.as_str())
                        .map(|(i, _)| pos + i)
                        .filter(|&e| e > pos)
                        .collect(),
                    Some(Tok::Slot(_)) => (pos + 1..=text.len())
                        .filter(|&e| text.is_char_boundary(e))
                        .collect(),
                };
                for end in ends {
                    let value = &text[pos..end];
                    if !valid_slot(name, value) {
                        continue;
                    }
                    out.push((name.clone(), value.to_owned()));
                    if rec(&toks[1..], text, end, out) {
                        return true;
                    }
                    out.pop();
                }
                false
            }
        }
    }
    let mut out = Vec::new();
    rec(toks, text, 0, &mut out).then_some(out)
}

/// Every wording a sentence can have: (frame key, flat template).
fn sentence_templates(tag: Option<Tag>) -> Vec<(String, String)> {
    let c = corpus();
    let mut keys: Vec<&String> = c.frame.keys().collect();
    keys.sort();
    let preds: Vec<String> = tag.map_or_else(Vec::new, pred_variants);
    let mut out = Vec::new();
    for key in keys {
        let styled = &c.frame[key];
        let mut wordings: Vec<&String> = styled.neutral.iter().collect();
        wordings.extend(styled.you.iter().flatten());
        wordings.extend(styled.opponent.iter().flatten());
        wordings.dedup();
        for wording in wordings {
            if wording.contains("{pred}") {
                for p in &preds {
                    out.push((key.clone(), wording.replace("{pred}", p)));
                }
            } else {
                out.push((key.clone(), wording.clone()));
            }
        }
    }
    out
}

/// The phrases a tag is said with.
fn pred_variants(tag: Tag) -> Vec<String> {
    let t = &corpus().tag[tag.key()];
    let mut out = vec![t.vp.clone()];
    out.extend(t.vp_long.clone());
    out.push(
        t.wins
            .clone()
            .unwrap_or_else(|| corpus().words.wins_default.replace("{vp}", &t.vp)),
    );
    out.dedup();
    out
}

/// The frame without its classification prefix or difficulty suffix.
fn frame_kind(key: &str) -> &str {
    let key = key
        .strip_suffix("_hard")
        .or_else(|| key.strip_suffix("_natural"))
        .unwrap_or(key);
    for prefix in ["brilliant_", "great_find_", "book_"] {
        if let Some(k) = key.strip_prefix(prefix) {
            return k;
        }
    }
    key
}

/// What a sentence names and where its moves are played.
struct Reading<'b> {
    kind: &'b str,
    bindings: &'b Bindings,
}

impl Reading<'_> {
    fn get(&self, name: &str) -> Option<&str> {
        self.bindings
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The bad move itself is the subject (BASE frames).
    fn base(&self) -> bool {
        self.kind.starts_with("base")
    }

    /// Where `{line}` starts: after the played move for what it allows,
    /// before it for what the played or best move does.
    fn line_start(&self) -> Start {
        if self.base() || self.kind.starts_with("permits") {
            Start::After
        } else {
            Start::Before
        }
    }

    /// Where `{reply}` is played when the sentence spells out no line.
    fn reply_start(&self) -> Start {
        match self.kind {
            "threatens" => Start::NullAfter,
            "misses_threatens" | "best_threatens" => Start::NullBest,
            "prevents" | "misses_prevents" | "best_prevents" => Start::NullBefore,
            _ => self.line_start(),
        }
    }

    /// `{reply}` is the end of a sequence the sentence does not spell out.
    fn long(&self) -> bool {
        self.kind.ends_with("_long")
    }
}

// ---------------------------------------------------------------- the check

type Found = (Kind, String, String);

struct Ctx<'a> {
    input: &'a VerifyInput<'a>,
    after: Chess,
    mover: Color,
    best_move: Option<Move>,
    null_before: Option<Chess>,
    null_after: Option<Chess>,
    null_best: Option<Chess>,
}

/// A move in its position, with what follows it on its line.
struct MoveFacts<'w> {
    before: &'w Chess,
    m: Move,
    after: &'w Chess,
    previous: Option<Move>,
    /// Positions after the moves that follow `m` on its line.
    next: &'w [Chess],
}

impl MoveFacts<'_> {
    fn actor(&self) -> Color {
        self.before.turn()
    }

    fn moved(&self) -> Option<(Square, Piece)> {
        let sq = moved_square(self.m);
        self.after.board().piece_at(sq).map(|p| (sq, p))
    }
}

/// Where a named piece must stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum When {
    Before,
    After,
    Either,
    /// After the other side's answer ("once a pawn takes back").
    Answer,
}

impl<'a> Ctx<'a> {
    fn new(input: &'a VerifyInput<'a>) -> Ctx<'a> {
        let mut after = input.before.clone();
        after.play_unchecked(input.played);
        let best_move = input
            .best
            .first()
            .and_then(|u| board::parse_uci(input.before, u));
        let null_best = best_move.and_then(|b| {
            let mut p = input.before.clone();
            p.play_unchecked(b);
            board::null_move(&p)
        });
        Ctx {
            input,
            null_before: board::null_move(input.before),
            null_after: board::null_move(&after),
            after,
            mover: input.before.turn(),
            best_move,
            null_best,
        }
    }

    fn pos(&self, s: Start) -> Option<&Chess> {
        match s {
            Start::Before => Some(self.input.before),
            Start::After => Some(&self.after),
            Start::NullBefore => self.null_before.as_ref(),
            Start::NullAfter => self.null_after.as_ref(),
            Start::NullBest => self.null_best.as_ref(),
        }
    }

    fn lead(&self, s: Start) -> Option<Move> {
        match s {
            Start::Before => self.input.previous,
            Start::After => Some(self.input.played),
            _ => None,
        }
    }

    fn colour(&self, owner: Owner) -> Option<Color> {
        match (owner, self.input.style) {
            (Owner::White, _) => Some(Color::White),
            (Owner::Black, _) => Some(Color::Black),
            (Owner::Your, Style::You) | (Owner::Their, Style::Opponent) => Some(self.mover),
            (Owner::Their, Style::You) | (Owner::Your, Style::Opponent) => Some(self.mover.other()),
            (_, Style::Neutral) => None,
        }
    }

    /// The event's line on the board, from the position its circumstance
    /// starts it in.
    fn event_walk(&self, e: &CoachEvent) -> Result<(Start, Walk), String> {
        let candidates: &[Start] = match e.chain.first() {
            Some(Circ::Permits) => &[Start::After],
            Some(Circ::Misses | Circ::MissesSame) => match e.chain.get(1) {
                Some(Circ::Threatens) => &[Start::NullBest],
                Some(Circ::Prevents) => &[Start::NullBefore],
                _ => &[Start::Before],
            },
            Some(Circ::Threatens) => &[Start::NullAfter],
            Some(Circ::Prevents) => &[Start::NullBefore],
            // A good move's own line (the move first), or a bad move's reply line.
            _ => &[Start::Before, Start::After],
        };
        let mut why = String::from("no start position");
        for &s in candidates {
            let Some(pos) = self.pos(s) else { continue };
            match Walk::uci(pos, self.lead(s), &e.line) {
                Ok(w) if s == Start::Before && e.chain.first() == Some(&Circ::Base) => {
                    if w.moves.first() == Some(&self.input.played) {
                        return Ok((s, w));
                    }
                }
                Ok(w) => return Ok((s, w)),
                Err(u) => why = format!("{u} is illegal on the event's line"),
            }
        }
        Err(why)
    }

    /// Checks one sentence: the reading with the fewest false claims wins.
    fn sentence(
        &self,
        text: &str,
        arrows: &[&Arrow],
        event: Option<&CoachEvent>,
    ) -> (Vec<Found>, Vec<Found>) {
        let mut readings = Vec::new();
        let lowered = lower_first(text);
        for (key, template) in sentence_templates(event.map(|e| e.tag)) {
            let toks = tokenize(&template);
            for t in [text, lowered.as_str()] {
                if let Some(b) = match_template(&toks, t) {
                    readings.push((key.clone(), b));
                    break;
                }
            }
        }
        if readings.is_empty() {
            let detail = match event {
                Some(e) => format!("no frame says this with the phrase of {}", e.tag.key()),
                None => "no frame says this".to_owned(),
            };
            return (vec![(Kind::Unparsed, text.to_owned(), detail)], Vec::new());
        }
        let mut best: Option<(Vec<Found>, Vec<Found>)> = None;
        for (key, bindings) in &readings {
            let reading = Reading {
                kind: frame_kind(key),
                bindings,
            };
            let result = self.reading(&reading, arrows, event);
            // Fewest false claims, then fewest claims left unchecked.
            let score = |r: &(Vec<Found>, Vec<Found>)| (r.0.len(), r.1.len());
            if best.as_ref().is_none_or(|b| score(&result) < score(b)) {
                best = Some(result);
            }
            if best.as_ref().is_some_and(|b| score(b) == (0, 0)) {
                break;
            }
        }
        best.expect("at least one reading")
    }

    fn reading(
        &self,
        r: &Reading<'_>,
        arrows: &[&Arrow],
        event: Option<&CoachEvent>,
    ) -> (Vec<Found>, Vec<Found>) {
        let mut out: Vec<Found> = Vec::new();
        let mut unchecked = Vec::new();
        let before = self.input.before;
        let w = &corpus().words;

        // Who is talking about whom.
        if let Some(m) = r.get("mover") {
            let expected = match self.input.style {
                Style::You => &w.you,
                Style::Opponent => &w.opponent,
                Style::Neutral => self.mover.fold_wb(&w.white, &w.black),
            };
            if !m.eq_ignore_ascii_case(expected) {
                out.push((Kind::Side, m.to_owned(), format!("the mover is {expected}")));
            }
        }
        if let Some(m) = r.get("mover_s") {
            let ok = owner_word(m).and_then(|o| self.colour(o)) == Some(self.mover);
            if !ok {
                out.push((Kind::Side, m.to_owned(), "names the other side".to_owned()));
            }
        }

        // The moves the sentence names.
        let mut named_moves: Vec<Move> = Vec::new();
        if let Some(t) = r.get("move") {
            match parse_san(before, t) {
                Ok(m) if m == self.input.played => named_moves.push(m),
                Ok(_) => out.push((
                    Kind::Move,
                    t.to_owned(),
                    format!("the move played was {}", san(before, self.input.played)),
                )),
                Err(e) => out.push((Kind::Move, t.to_owned(), e)),
            }
        }
        if let Some(t) = r.get("best") {
            match parse_san(before, t) {
                Ok(m) => {
                    if self.best_move.is_some_and(|b| b != m) {
                        let b = self.best_move.expect("checked");
                        out.push((
                            Kind::Move,
                            t.to_owned(),
                            format!("the engine's best move is {}", san(before, b)),
                        ));
                    }
                    named_moves.push(m);
                }
                Err(e) => out.push((Kind::Move, t.to_owned(), e)),
            }
        }
        if r.kind == "forced" && before.legal_moves().len() != 1 {
            out.push((
                Kind::Move,
                r.get("move").unwrap_or_default().to_owned(),
                format!("{} legal moves, not one", before.legal_moves().len()),
            ));
        }

        // The event's line, which the sentence's own moves must follow.
        let walk = event.map(|e| self.event_walk(e));
        if let Some(Err(why)) = &walk {
            out.push((Kind::Move, "the event's line".to_owned(), why.clone()));
        }
        let walk = walk.and_then(Result::ok);

        // {line} and {reply}, in sequence from where the sentence starts them.
        let mut seq: Option<(Walk, Option<usize>)> = None;
        let mut broken = false;
        if r.get("line").is_some() || r.get("reply").is_some() {
            let start = if r.get("line").is_some() {
                r.line_start()
            } else {
                r.reply_start()
            };
            if let Some(pos) = self.pos(start) {
                let mut text_walk = Walk::empty(pos, self.lead(start));
                let mut tokens: Vec<&str> = r
                    .get("line")
                    .map_or_else(Vec::new, |l| l.split(' ').collect());
                let reply_at = if r.long() {
                    None
                } else {
                    r.get("reply").map(|t| {
                        tokens.push(t);
                        tokens.len() - 1
                    })
                };
                for t in &tokens {
                    match parse_san(text_walk.last(), t) {
                        Ok(m) => {
                            text_walk.push(m);
                            named_moves.push(m);
                        }
                        Err(e) => {
                            out.push((Kind::Move, (*t).to_owned(), e));
                            broken = true;
                            break;
                        }
                    }
                }
                if !broken {
                    // The words follow the engine line of the event.
                    let mut on_event_line = None;
                    if let Some((s, ew)) = &walk {
                        let prefix = *s == start
                            && ew.moves.len() >= text_walk.moves.len()
                            && ew.moves[..text_walk.moves.len()] == text_walk.moves[..];
                        if prefix {
                            on_event_line = Some(ew.clone());
                        } else if !text_walk.moves.is_empty() {
                            out.push((
                                Kind::Move,
                                tokens.join(" "),
                                "not the line the event is on".to_owned(),
                            ));
                        }
                    }
                    let mut reply_at = reply_at;
                    let line = on_event_line.unwrap_or(text_walk);
                    if r.long()
                        && let (Some(t), Some(e)) = (r.get("reply"), event)
                    {
                        let k = e.ply as usize;
                        match line.moves.get(k) {
                            Some(&m) if san(&line.positions[k], m) == t => {
                                named_moves.push(m);
                                reply_at = Some(k);
                            }
                            _ => {
                                out.push((
                                    Kind::Move,
                                    t.to_owned(),
                                    "not the move the line ends with".to_owned(),
                                ));
                                broken = true;
                            }
                        }
                    }
                    seq = Some((line, reply_at));
                }
            } else {
                out.push((
                    Kind::Move,
                    r.get("reply").unwrap_or_default().to_owned(),
                    "no position to play it from (the side to move is in check)".to_owned(),
                ));
                broken = true;
            }
        }

        // The subject of the phrase: the reply, else the best or played move.
        // The event's line when it starts before the move (B's or P's).
        let line_before = |w: &Option<(Start, Walk)>| {
            w.as_ref()
                .filter(|(s, ew)| *s == Start::Before && !ew.moves.is_empty())
                .map(|(_, ew)| ew.clone())
        };
        let subject: Option<(Walk, usize)> = if broken {
            None
        } else if r.base() {
            // The played move, before its reply line (or on its own line).
            match &walk {
                Some((Start::Before, ew)) if !ew.moves.is_empty() => Some((ew.clone(), 0)),
                _ => {
                    let mut w = Walk::empty(before, self.input.previous);
                    w.push(self.input.played);
                    if let Some((Start::After, ew)) = &walk {
                        for &m in &ew.moves {
                            w.push(m);
                        }
                    }
                    Some((w, 0))
                }
            }
        } else if let Some((line, Some(k))) = &seq {
            Some((line.clone(), *k))
        } else if r.kind.starts_with("misses") || r.kind.starts_with("best") {
            line_before(&walk)
                .or_else(|| {
                    self.best_move.map(|b| {
                        let mut w = Walk::empty(before, self.input.previous);
                        w.push(b);
                        w
                    })
                })
                .map(|w| (w, 0))
        } else {
            let mut w = line_before(&walk)
                .filter(|ew| ew.moves.first() == Some(&self.input.played))
                .unwrap_or_else(|| {
                    let mut w = Walk::empty(before, self.input.previous);
                    w.push(self.input.played);
                    w
                });
            if w.moves.is_empty() {
                w.push(self.input.played);
            }
            Some((w, 0))
        };

        if let Some((sw, k)) = &subject
            && let Some(e) = event
        {
            let facts = sw.facts(*k);
            let claims = self.claims(r, &mut out);
            self.tag_claims(e.tag, &facts, &claims, &mut out);
            if r.kind == "misses_same"
                && let Some(p) = &claims.piece
            {
                self.free_piece(&facts, p, &mut out);
            }
            // The gain and the mate are checked on the line the gain is on:
            // for a BASE sentence, the reply line after the move.
            let (gw, gk) = if r.base() {
                match walk.as_ref().filter(|(s, _)| *s == Start::After) {
                    Some((_, ew)) => (ew.clone(), 0),
                    None => (sw.clone(), 1),
                }
            } else {
                (sw.clone(), *k)
            };
            if let Some(min) = claims.gain {
                let gain_text = r.get("gain").unwrap_or_default();
                if let Some(won) = won_on(&gw, gk.min(gw.moves.len()))
                    && won < min
                {
                    out.push((
                        Kind::Material,
                        format!("wins {gain_text}"),
                        format!("the line wins {} at most", pawns(won)),
                    ));
                }
            }
            if let Some(n) = mate_claim(e.tag, &claims) {
                match mate_within(sw, *k, n) {
                    MateCheck::Yes => {}
                    MateCheck::Unknown => unchecked.push((
                        Kind::Mate,
                        mate_text(n),
                        format!("the line ends after {} plies", sw.moves.len()),
                    )),
                    MateCheck::No(why) => out.push((Kind::Mate, mate_text(n), why)),
                }
            }
            // Arrows: attacks and guards around the subject move.
            for a in arrows {
                self.arrow(a, &facts, &named_moves, &mut out);
            }
        } else {
            for a in arrows {
                let ok = match a.kind {
                    ArrowKind::Best => self.best_move.is_some_and(|b| same_squares(b, a)),
                    _ => named_moves.iter().any(|m| same_squares(*m, a)),
                };
                if !ok {
                    out.push((
                        Kind::Geometry,
                        format!("arrow {}-{}", a.from, a.to),
                        "no move of the sentence goes there".to_owned(),
                    ));
                }
            }
        }
        (out, unchecked)
    }

    /// The pieces, squares, gain and moves to mate the sentence names.
    fn claims(&self, r: &Reading<'_>, out: &mut Vec<Found>) -> Claims {
        let mut resolve = |n: Named| -> Option<Resolved> {
            match self.colour(n.owner) {
                Some(color) => Some(Resolved {
                    piece: Piece {
                        color,
                        role: n.role,
                    },
                    square: n.square,
                    text: n.text,
                }),
                None => {
                    out.push((
                        Kind::Side,
                        n.text,
                        "your or their without a player".to_owned(),
                    ));
                    None
                }
            }
        };
        Claims {
            piece: r
                .get("piece")
                .and_then(|s| parse_piece(s, true))
                .and_then(&mut resolve),
            piece_role: r
                .get("piece_role")
                .and_then(|s| parse_piece(s, false))
                .and_then(&mut resolve),
            targets: r
                .get("targets")
                .and_then(parse_targets)
                .map(|ts| ts.into_iter().filter_map(&mut resolve).collect())
                .unwrap_or_default(),
            square: r.get("square").and_then(|s| s.parse().ok()),
            gain: r.get("gain").and_then(parse_gain),
            n: r.get("n").and_then(|s| s.parse().ok()),
        }
    }

    fn arrow(&self, a: &Arrow, f: &MoveFacts<'_>, moves: &[Move], out: &mut Vec<Found>) {
        let claim = format!("arrow {}-{}", a.from, a.to);
        let (Ok(from), Ok(to)) = (a.from.parse::<Square>(), a.to.parse::<Square>()) else {
            out.push((Kind::Geometry, claim, "not squares".to_owned()));
            return;
        };
        let ok = match a.kind {
            ArrowKind::Best => self.best_move.is_some_and(|b| same_squares(b, a)),
            ArrowKind::Move => same_squares(f.m, a) || moves.iter().any(|m| same_squares(*m, a)),
            ArrowKind::Attack | ArrowKind::Defence => {
                xrays(f.after, from, to) || xrays(f.before, from, to)
            }
        };
        if !ok {
            let detail = match a.kind {
                ArrowKind::Attack => "no attack or x-ray along it",
                ArrowKind::Defence => "no guard along it",
                _ => "no move of the sentence goes there",
            };
            out.push((Kind::Geometry, claim, detail.to_owned()));
        }
    }

    /// "wins {piece}": the move takes it.
    fn free_piece(&self, f: &MoveFacts<'_>, p: &Resolved, out: &mut Vec<Found>) {
        let Some(sq) = p.square else { return };
        if !(f.m.is_capture() && board::captured_square(f.m) == sq) {
            out.push((
                Kind::Material,
                format!("wins {}", p.text),
                "the line does not take it".to_owned(),
            ));
        }
    }

    /// What a tag's phrase says about its move, checked on the board.
    fn tag_claims(&self, tag: Tag, f: &MoveFacts<'_>, c: &Claims, out: &mut Vec<Found>) {
        let actor = f.actor();
        let (piece_when, targets_when) = when(tag);
        if let Some(p) = &c.piece {
            stands(f, p, piece_when, out);
        }
        for t in &c.targets {
            stands(f, t, targets_when, out);
        }
        let moved = f.moved();
        let sq_of = |p: &Resolved| p.square;
        let mv = san(f.before, f.m);
        match tag {
            Tag::Fork | Tag::RoyalFork => {
                let ok = c.targets.len() >= 2
                    && moved.is_some_and(|(from, _)| {
                        c.targets.iter().all(|t| {
                            sq_of(t)
                                .or_else(|| f.after.board().king_of(t.piece.color))
                                .is_some_and(|sq| attacks_sq(f.after, from, sq))
                        })
                    });
                geo(
                    out,
                    ok,
                    &format!("{mv} forks"),
                    "the moved piece does not attack every target",
                );
            }
            Tag::Skewer => {
                let ok = match c.targets.as_slice() {
                    [front, behind] => {
                        let squares = (
                            sq_of(front).or_else(|| f.after.board().king_of(front.piece.color)),
                            sq_of(behind),
                        );
                        match squares {
                            (Some(a), Some(b)) => (f.after.board().by_color(actor)
                                & f.after.board().occupied())
                            .into_iter()
                            .any(|s| through(f.after, s, a, b)),
                            _ => false,
                        }
                    }
                    _ => false,
                };
                geo(
                    out,
                    ok,
                    &format!("{mv} skewers"),
                    "no line piece goes through both targets",
                );
            }
            Tag::Pin => {
                let ok = match (c.piece.as_ref().and_then(sq_of), c.targets.as_slice()) {
                    (Some(front), [behind]) => {
                        let b =
                            sq_of(behind).or_else(|| f.after.board().king_of(behind.piece.color));
                        b.is_some_and(|b| {
                            f.after
                                .board()
                                .by_color(actor)
                                .into_iter()
                                .any(|s| through(f.after, s, front, b))
                        })
                    }
                    _ => false,
                };
                geo(
                    out,
                    ok,
                    &format!("{mv} pins"),
                    "no line piece pins it to the target",
                );
            }
            Tag::ExploitsPin => {
                let pinned = |pos: &Chess, sq: Square| {
                    let Some(p) = pos.board().piece_at(sq) else {
                        return false;
                    };
                    pos.board().by_color(p.color.other()).into_iter().any(|s| {
                        (pos.board().by_color(p.color)).into_iter().any(|b| {
                            b != sq
                                && through(pos, s, sq, b)
                                && pos.board().piece_at(b).is_some_and(|x| {
                                    x.role == Role::King
                                        || board::role_value(x.role) > board::role_value(p.role)
                                })
                        })
                    })
                };
                if let Some(sq) = c.piece.as_ref().and_then(sq_of) {
                    geo(
                        out,
                        pinned(f.before, sq) || pinned(f.after, sq),
                        &format!("the pin on {sq}"),
                        "the piece is not pinned",
                    );
                }
            }
            Tag::DiscoveredCheck => {
                let ok = c.piece.as_ref().is_some_and(|p| {
                    let sq = p.square.or_else(|| {
                        f.after
                            .checkers()
                            .into_iter()
                            .find(|s| f.after.board().piece_at(*s) == Some(p.piece))
                    });
                    sq.is_some_and(|sq| {
                        f.after.checkers().contains(sq) && Some(sq) != moved.map(|m| m.0)
                    })
                });
                geo(
                    out,
                    ok,
                    &format!("{mv} uncovers a check"),
                    "no other piece gives check",
                );
            }
            Tag::DoubleCheck => {
                geo(
                    out,
                    f.after.checkers().count() >= 2,
                    &format!("{mv} gives double check"),
                    "fewer than two pieces give check",
                );
            }
            Tag::DiscoveredAttack => {
                let ok = c.piece_role.as_ref().is_some_and(|pr| {
                    (f.after.board().by_piece(pr.piece)).into_iter().any(|s| {
                        Some(s) != moved.map(|m| m.0)
                            && !c.targets.is_empty()
                            && c.targets.iter().all(|t| {
                                let tsq =
                                    t.square.or_else(|| f.after.board().king_of(t.piece.color));
                                tsq.is_some_and(|tsq| {
                                    attacks_sq(f.after, s, tsq) && !attacks_sq(f.before, s, tsq)
                                })
                            })
                    })
                });
                geo(
                    out,
                    ok,
                    &format!("{mv} uncovers an attack"),
                    "no uncovered piece attacks the targets",
                );
            }
            Tag::RemovesDefender => {
                if let Some(sq) = c.piece.as_ref().and_then(sq_of) {
                    geo(
                        out,
                        f.m.is_capture() && board::captured_square(f.m) == sq,
                        &format!("{mv} takes the defender"),
                        "the move does not take it",
                    );
                    for t in &c.targets {
                        if let Some(tsq) = t.square {
                            geo(
                                out,
                                attacks_sq(f.before, sq, tsq),
                                &format!("guarding {}", t.text),
                                "the piece did not guard it",
                            );
                        }
                    }
                }
            }
            Tag::Overloaded => {
                if let Some(sq) = c.piece.as_ref().and_then(sq_of) {
                    for t in &c.targets {
                        if let Some(tsq) = t.square {
                            geo(
                                out,
                                attacks_sq(f.before, sq, tsq) || attacks_sq(f.after, sq, tsq),
                                &format!("guard {}", t.text),
                                "the piece does not guard it",
                            );
                        }
                    }
                }
            }
            Tag::TrappedPiece
            | Tag::Kicks
            | Tag::GainsTempo
            | Tag::StrikesAtCenter
            | Tag::CentralBreak
            | Tag::AttacksPawnChain => {
                if let (Some(p), Some((from, _))) = (&c.piece, moved)
                    && let Some(sq) = p.square
                {
                    let ok = if tag == Tag::TrappedPiece {
                        attacked_by(f.after, sq, actor)
                    } else {
                        attacks_sq(f.after, from, sq)
                    };
                    geo(
                        out,
                        ok,
                        &format!("{mv} attacks {}", p.text),
                        "the moved piece does not attack it",
                    );
                }
                if matches!(
                    tag,
                    Tag::Kicks | Tag::StrikesAtCenter | Tag::CentralBreak | Tag::AttacksPawnChain
                ) && f.m.role() != Role::Pawn
                {
                    out.push((Kind::Piece, mv.clone(), "not a pawn move".to_owned()));
                }
            }
            Tag::CounterThreat => {
                if let (Some(p), Some((from, _))) = (&c.piece, moved)
                    && let Some(sq) = p.square
                {
                    geo(
                        out,
                        attacks_sq(f.after, from, sq),
                        &format!("hits back at {}", p.text),
                        "the moved piece does not attack it",
                    );
                }
                for t in &c.targets {
                    if let Some(sq) = t.square {
                        geo(
                            out,
                            attacked_by(f.after, sq, actor.other()),
                            &format!("instead of saving {}", t.text),
                            "it is not attacked",
                        );
                    }
                }
            }
            Tag::HangsPiece | Tag::HangsPawn | Tag::IgnoresThreat | Tag::OffersPiece => {
                if let Some(p) = &c.piece
                    && let Some(sq) = p.square
                {
                    geo(
                        out,
                        attacked_by(f.after, sq, p.piece.color.other()),
                        &p.text,
                        "nothing attacks it",
                    );
                }
            }
            Tag::MovesPinnedPiece => {
                for t in &c.targets {
                    if let Some(sq) = t.square.or_else(|| f.after.board().king_of(t.piece.color)) {
                        geo(
                            out,
                            attacked_by(f.after, sq, actor.other()),
                            &format!("exposes {}", t.text),
                            "nothing attacks it",
                        );
                    }
                }
            }
            Tag::RemovesOwnDefender => {
                if let (Some(p), Some(from)) = (&c.piece, f.m.from())
                    && let Some(sq) = p.square
                {
                    geo(
                        out,
                        attacks_sq(f.before, from, sq),
                        &format!("stops guarding {}", p.text),
                        "the moved piece did not guard it",
                    );
                }
            }
            Tag::DidNotRecapture => {
                if let Some(sq) = c.piece.as_ref().and_then(sq_of) {
                    let ok = f.previous.is_some_and(|p| p.to() == sq);
                    if !ok {
                        out.push((
                            Kind::Move,
                            format!("does not take back on {sq}"),
                            "the previous move did not land there".to_owned(),
                        ));
                    }
                }
            }
            Tag::DefendsPiece => {
                // It guards the piece, or stands between it and an attacker.
                if let Some(sq) = c.piece.as_ref().and_then(sq_of) {
                    let to = moved_square(f.m);
                    let blocks = f
                        .before
                        .board()
                        .attacks_to(sq, actor.other(), f.before.board().occupied())
                        .into_iter()
                        .any(|a| attacks::between(a, sq).contains(to));
                    let threatened = attacked_by(f.before, sq, actor.other());
                    geo(
                        out,
                        threatened && (attacks_sq(f.after, to, sq) || blocks),
                        &format!("defends {sq}"),
                        "the moved piece neither guards it nor blocks an attack on it",
                    );
                }
            }
            Tag::EvadesCheck | Tag::BlocksCheck => {
                if !f.before.is_check() {
                    out.push((
                        Kind::Move,
                        mv.clone(),
                        "there was no check to answer".to_owned(),
                    ));
                }
                for t in &c.targets {
                    if let Some(sq) = t.square {
                        geo(
                            out,
                            f.before.checkers().contains(sq),
                            &format!("the check from {}", t.text),
                            "it was not giving check",
                        );
                    }
                }
            }
            Tag::Checkmate | Tag::BackRankMate => {
                if !f.after.is_checkmate() {
                    out.push((
                        Kind::Mate,
                        format!("{mv} mates"),
                        "the position after it is not mate".to_owned(),
                    ));
                } else if tag == Tag::BackRankMate {
                    let king = f.after.board().king_of(actor.other());
                    let back = king.is_some_and(|k| relative_rank(k, actor.other()) == 0);
                    if !back {
                        out.push((
                            Kind::Mate,
                            format!("{mv} mates on the back rank"),
                            "the king is not on its back rank".to_owned(),
                        ));
                    }
                }
            }
            Tag::Promotion => {
                let ok = f.m.is_promotion() && c.square.is_none_or(|s| f.m.to() == s);
                if !ok {
                    out.push((
                        Kind::Move,
                        format!("{mv} promotes"),
                        "it is not a promotion there".to_owned(),
                    ));
                }
            }
            Tag::Recapture | Tag::WinsTrade | Tag::EqualTrade | Tag::LosesTrade | Tag::BadTrade => {
                let ok = f.m.is_capture() && c.square.is_none_or(|s| f.m.to() == s);
                if !ok {
                    out.push((
                        Kind::Move,
                        mv.clone(),
                        "it does not capture there".to_owned(),
                    ));
                }
                if tag == Tag::Recapture && !f.previous.is_some_and(|p| p.to() == f.m.to()) {
                    out.push((
                        Kind::Move,
                        format!("{mv} takes back"),
                        "the previous move did not capture there".to_owned(),
                    ));
                }
                if matches!(tag, Tag::LosesTrade | Tag::BadTrade) {
                    for t in &c.targets {
                        if t.square.is_some_and(|s| s != board::captured_square(f.m)) {
                            out.push((
                                Kind::Move,
                                format!("takes {}", t.text),
                                "the move captures elsewhere".to_owned(),
                            ));
                        }
                    }
                }
            }
            Tag::FreePiece => {
                if let Some(p) = &c.piece {
                    self.free_piece(f, p, out);
                }
            }
            Tag::CastlesKingside
            | Tag::CastlesQueenside
            | Tag::CastlesSameSide
            | Tag::CastlesOppositeSide => {
                let side = f.m.castling_side();
                let ok = match tag {
                    Tag::CastlesKingside => side == Some(CastlingSide::KingSide),
                    Tag::CastlesQueenside => side == Some(CastlingSide::QueenSide),
                    _ => side.is_some(),
                };
                if !ok {
                    out.push((Kind::Move, mv.clone(), "not that castling move".to_owned()));
                }
            }
            Tag::ConnectsRooks => {
                let rooks: Vec<Square> = c.targets.iter().filter_map(|t| t.square).collect();
                let ok = matches!(rooks.as_slice(), [a, b] if a.rank() == b.rank()
                    && attacks_sq(f.after, *a, *b));
                geo(
                    out,
                    ok,
                    "connects the rooks",
                    "the rooks do not see each other",
                );
            }
            Tag::MakesPassedPawn => {
                if let Some(sq) = c.square {
                    let ok = f.after.board().piece_at(sq)
                        == Some(Piece {
                            color: actor,
                            role: Role::Pawn,
                        })
                        && pawn_passed(f.after, sq, actor);
                    if !ok {
                        out.push((
                            Kind::Piece,
                            format!("a passed pawn on {sq}"),
                            "no passed pawn there".to_owned(),
                        ));
                    }
                }
            }
            Tag::PushesPassedPawn | Tag::PushesPassedPawnEndgame => {
                let ok = f.m.role() == Role::Pawn && pawn_passed(f.after, f.m.to(), actor);
                if !ok {
                    out.push((
                        Kind::Piece,
                        format!("{mv} advances the passed pawn"),
                        "the moved pawn is not passed".to_owned(),
                    ));
                }
            }
            Tag::IsolatedPawn | Tag::ForcesIsolatedPawn => {
                let pos = if tag == Tag::IsolatedPawn {
                    Some(f.after)
                } else {
                    f.next.first()
                };
                if let (Some(pos), Some(p)) = (pos, &c.piece)
                    && let Some(sq) = p.square
                    && !pawn_isolated(pos, sq, p.piece.color)
                {
                    out.push((
                        Kind::Piece,
                        format!("{} isolated", p.text),
                        "a pawn of its side stands next to it".to_owned(),
                    ));
                }
            }
            Tag::ForcesDoubledPawns => {
                if let Some(pos) = f.next.first() {
                    let files: Vec<_> = c
                        .targets
                        .iter()
                        .filter_map(|t| t.square.map(|s| s.file()))
                        .collect();
                    let doubled = files.len() >= 2 && files.iter().all(|x| *x == files[0]);
                    let there = c.targets.iter().all(|t| {
                        t.square
                            .is_some_and(|s| pos.board().piece_at(s) == Some(t.piece))
                    });
                    if !(doubled && there) {
                        out.push((
                            Kind::Piece,
                            "doubled once a pawn takes back".to_owned(),
                            "the pawns are not doubled after the answer".to_owned(),
                        ));
                    }
                }
            }
            Tag::PreparesCentralPush => {
                // The named pawn stands behind a square the moved pawn now
                // guards, with nothing in between.
                if let (Some(p), Some(sq)) = (&c.piece, c.piece.as_ref().and_then(sq_of)) {
                    let to = moved_square(f.m);
                    let ok = attacks::attacks(
                        to,
                        Piece {
                            color: actor,
                            role: Role::Pawn,
                        },
                        Bitboard::EMPTY,
                    )
                    .into_iter()
                    .any(|t| {
                        t.file() == sq.file()
                            && relative_rank(t, actor) > relative_rank(sq, actor)
                            && relative_rank(t, actor) - relative_rank(sq, actor) <= 2
                            && f.after.board().piece_at(t).is_none()
                            && (attacks::between(sq, t) & f.after.board().occupied()).is_empty()
                    });
                    geo(
                        out,
                        ok,
                        &format!("a central advance with {}", p.text),
                        "it cannot advance to a square the moved pawn guards",
                    );
                }
            }
            Tag::PreparesBishopDevelopment => {
                let from = f.m.from().filter(|_| f.m.role() == Role::Pawn);
                let bishops = f.after.board().by_piece(Piece {
                    color: actor,
                    role: Role::Bishop,
                });
                let ok = from
                    .is_some_and(|from| bishops.into_iter().any(|b| attacks_sq(f.after, b, from)));
                geo(
                    out,
                    ok,
                    &format!("{mv} opens a diagonal"),
                    "no bishop of its side looks through the square the pawn left",
                );
            }
            Tag::RookOnSeventh => {
                let ok = f.m.role() == Role::Rook && relative_rank(f.m.to(), actor) == 6;
                if !ok {
                    out.push((
                        Kind::Piece,
                        format!("{mv} reaches the seventh rank"),
                        "not a rook on the seventh".to_owned(),
                    ));
                }
            }
            Tag::TakesOpenFile => {
                let to = castle_rook(f.m).unwrap_or(f.m.to());
                let pawns = f.after.board().pawns() & Bitboard::from_file(to.file());
                if pawns.any() {
                    out.push((
                        Kind::Piece,
                        format!("{mv} takes an open file"),
                        "the file has pawns".to_owned(),
                    ));
                }
            }
            _ => {}
        }
        // A piece named without its square is the moved one, or one of the
        // side that must exist.
        if let Some(pr) = &c.piece_role {
            let moved_role = matches!(
                tag,
                Tag::EscapesThreat
                    | Tag::RookOnSeventh
                    | Tag::Outpost
                    | Tag::TakesOpenFile
                    | Tag::CentralizesKnight
                    | Tag::MovesToActiveSquare
                    | Tag::BishopGainsScope
                    | Tag::DevelopsCenter
                    | Tag::DevelopsOpening
                    | Tag::DevelopsMiddlegame
                    | Tag::Fianchetto
                    | Tag::KingActivity
                    | Tag::TooEarlyQueen
                    | Tag::MovesSamePiece
                    | Tag::MovesToPassiveSquare
                    | Tag::MovesPinnedPiece
                    | Tag::BadTrade
                    | Tag::Battery
            );
            if moved_role {
                let rook = castle_rook(f.m).and_then(|sq| f.after.board().piece_at(sq));
                if moved.map(|m| m.1) != Some(pr.piece) && rook != Some(pr.piece) {
                    let what = moved.map_or("nothing".to_owned(), |(_, p)| {
                        format!("{:?} {:?}", p.color, p.role).to_lowercase()
                    });
                    out.push((
                        Kind::Piece,
                        pr.text.clone(),
                        format!("the moved piece is a {what}"),
                    ));
                }
            } else if pr.piece.color
                != if tag == Tag::Attraction {
                    actor.other()
                } else {
                    actor
                }
            {
                out.push((
                    Kind::Side,
                    pr.text.clone(),
                    "names the other side's piece".to_owned(),
                ));
            } else if !f.after.board().by_piece(pr.piece).any()
                && !f.before.board().by_piece(pr.piece).any()
            {
                out.push((
                    Kind::Piece,
                    pr.text.clone(),
                    "no such piece on the board".to_owned(),
                ));
            }
            if tag == Tag::Sacrifice
                && let Some(sq) = c.square
                && f.after.board().piece_at(sq) != Some(pr.piece)
            {
                out.push((
                    Kind::Piece,
                    format!("{} on {sq}", pr.text),
                    "it is not there after the move".to_owned(),
                ));
            }
        }
    }
}

fn geo(out: &mut Vec<Found>, ok: bool, claim: &str, detail: &str) {
    if !ok {
        out.push((Kind::Geometry, claim.to_owned(), detail.to_owned()));
    }
}

fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A named piece with its side resolved.
#[derive(Debug, Clone)]
struct Resolved {
    piece: Piece,
    square: Option<Square>,
    text: String,
}

#[derive(Debug, Default)]
struct Claims {
    piece: Option<Resolved>,
    piece_role: Option<Resolved>,
    targets: Vec<Resolved>,
    square: Option<Square>,
    /// The smallest gain the words promise (cp).
    gain: Option<i32>,
    n: Option<u32>,
}

/// When the pieces a tag names stand on their squares: what the move takes
/// or what "was" there is in the position before it, the rest after it.
fn when(tag: Tag) -> (When, When) {
    match tag {
        Tag::FreePiece | Tag::RemovesDefender => (When::Before, When::Before),
        Tag::LosesTrade | Tag::BadTrade => (When::After, When::Before),
        Tag::EvadesCheck => (When::After, When::Before),
        Tag::ExploitsPin | Tag::Overloaded | Tag::Attraction => (When::Either, When::Either),
        Tag::ForcesDoubledPawns | Tag::ForcesIsolatedPawn => (When::Answer, When::Answer),
        _ => (When::After, When::After),
    }
}

fn stands(f: &MoveFacts<'_>, p: &Resolved, when: When, out: &mut Vec<Found>) {
    let Some(sq) = p.square else { return };
    let on = |pos: &Chess| pos.board().piece_at(sq) == Some(p.piece);
    let ok = match when {
        When::Before => on(f.before),
        When::After => on(f.after),
        When::Either => on(f.before) || on(f.after),
        When::Answer => on(f.after) || f.next.first().is_some_and(on),
    };
    if !ok {
        let pos = if when == When::Before {
            f.before
        } else {
            f.after
        };
        let there = pos.board().piece_at(sq).map_or("nothing".to_owned(), |x| {
            format!("{:?} {:?}", x.color, x.role).to_lowercase()
        });
        let at = if when == When::Before {
            "before"
        } else {
            "after"
        };
        out.push((
            Kind::Piece,
            p.text.clone(),
            format!("{at} {} there is {there} on {sq}", san(f.before, f.m)),
        ));
    }
}

/// The most material the line's mover is ahead at a settled point (no
/// capture next) from ply `from` on, against the start of the line, the
/// position before the move that led to it, and the position at `from`.
/// `None` when the line is empty.
fn won_on(w: &Walk, from: usize) -> Option<i32> {
    if w.moves.is_empty() {
        return None;
    }
    let actor = w.positions[0].turn();
    let bal = |pos: &Chess| board::balance(pos.board(), actor);
    let start = bal(&w.positions[0]);
    // The lead took one of the actor's pieces: before it the actor was ahead by that much more.
    let lead = w
        .lead
        .and_then(|m| m.capture())
        .map_or(0, board::role_value);
    let at_from = bal(&w.positions[from.min(w.moves.len())]);
    let refs = [start, start + lead, at_from];
    let mut best = i32::MIN;
    for k in from..w.moves.len() {
        let settled = w.moves.get(k + 1).is_none_or(|m| !m.is_capture());
        if settled {
            let b = bal(&w.positions[k + 1]);
            for r in refs {
                best = best.max(b - r);
            }
        }
    }
    (best > i32::MIN).then_some(best)
}

fn pawns(cp: i32) -> String {
    format!("{:.1} pawns", f64::from(cp) / 100.0)
}

/// A forced mate the sentence promises: `Some(n)` for a mate in n,
/// `Some(None)` for a mate somewhere on the line.
fn mate_claim(tag: Tag, c: &Claims) -> Option<Option<u32>> {
    (tag == Tag::ForcedMate).then_some(c.n)
}

fn mate_text(n: Option<u32>) -> String {
    match n {
        Some(n) if n > 1 => format!("a forced mate in {n}"),
        _ => "a forced mate".to_owned(),
    }
}

enum MateCheck {
    Yes,
    No(String),
    Unknown,
}

/// Longest line the coach keeps (`circ::MAX_LINE_PLIES`): a mate past it
/// cannot be seen.
const KEPT_PLIES: usize = 12;

/// The line mates for the mover at ply `from` within `n` of its moves, or
/// anywhere on the line when `n` is not said.
fn mate_within(w: &Walk, from: usize, n: Option<u32>) -> MateCheck {
    let solver = w.positions[from].turn();
    for k in (from..w.moves.len()).step_by(2) {
        let pos = &w.positions[k + 1];
        if pos.is_checkmate() && pos.turn() == solver.other() {
            let moves = (k - from) / 2 + 1;
            return match n {
                Some(n) if moves > n as usize => {
                    MateCheck::No(format!("the mate on the line comes in {moves}"))
                }
                _ => MateCheck::Yes,
            };
        }
    }
    let needed = from + n.map_or(1, |n| 2 * n as usize - 1);
    if w.moves.len() < needed || w.moves.len() >= KEPT_PLIES {
        MateCheck::Unknown
    } else {
        MateCheck::No("the line does not mate".to_owned())
    }
}

// ---------------------------------------------------------------- show-lines

impl Ctx<'_> {
    fn show_line(&self, i: usize, line: &ShowLine, report: &mut Report) {
        let fail =
            |report: &mut Report, kind: Kind, place: Place, claim: String, detail: String| {
                report.failures.push(Failure {
                    kind,
                    place,
                    claim,
                    detail,
                });
            };
        let place = Place::ShowLine(i);
        let starts = [
            Start::Before,
            Start::After,
            Start::NullBefore,
            Start::NullAfter,
            Start::NullBest,
        ];
        let start = starts.into_iter().find(|s| {
            self.pos(*s)
                .is_some_and(|p| board::fen(p) == line.start_fen)
        });
        let Some(start) = start else {
            fail(
                report,
                Kind::Move,
                place,
                line.label.clone(),
                format!(
                    "starts from {}, a position the note is not about",
                    line.start_fen
                ),
            );
            report.narrations += line.narration.len();
            report.false_narrations += line.narration.len();
            return;
        };
        let pos = self.pos(start).expect("found above");
        let mut walk = Walk::empty(pos, self.lead(start));
        for m in &line.moves {
            match board::parse_uci(walk.last(), &m.uci) {
                Some(mv) => {
                    let written = san(walk.last(), mv);
                    if written != m.san {
                        fail(
                            report,
                            Kind::Move,
                            place,
                            m.san.clone(),
                            format!("{} is written {written}", m.uci),
                        );
                    }
                    walk.push(mv);
                    if board::fen(walk.last()) != m.fen {
                        fail(
                            report,
                            Kind::Move,
                            place,
                            m.san.clone(),
                            "the position after it is wrong".to_owned(),
                        );
                    }
                }
                None => {
                    fail(
                        report,
                        Kind::Move,
                        place,
                        m.san.clone(),
                        format!("{} is illegal here", m.uci),
                    );
                    break;
                }
            }
        }
        // The line is the event's (or, for the best move, the engine's best line).
        let uci: Vec<&str> = line.moves.iter().map(|m| m.uci.as_str()).collect();
        let prefix_of = |l: &[String]| {
            l.len() >= uci.len() && l.iter().zip(&uci).all(|(a, b)| a.as_str() == *b)
        };
        let event_line = line
            .event
            .and_then(|e| self.input.note.events.get(e as usize))
            .map(|e| e.line.as_slice());
        let follows = event_line.is_some_and(prefix_of)
            || (start == Start::Before
                && !self.input.best.is_empty()
                && prefix_of(self.input.best));
        if !follows && (event_line.is_some() || start == Start::Before) {
            fail(
                report,
                Kind::Move,
                place,
                line.label.clone(),
                "not the line of its event".to_owned(),
            );
        }
        if line.narration.len() != line.moves.len() {
            fail(
                report,
                Kind::Narration,
                place,
                line.label.clone(),
                format!(
                    "{} narrations for {} moves",
                    line.narration.len(),
                    line.moves.len()
                ),
            );
        }
        for (k, text) in line.narration.iter().enumerate() {
            report.narrations += 1;
            let place = Place::Narration { line: i, ply: k };
            let failures = if k < walk.moves.len() {
                self.narration(text, &line.moves[k].san, &walk, k)
            } else {
                vec![(
                    Kind::Narration,
                    text.clone(),
                    "no legal move to narrate".to_owned(),
                )]
            };
            if !failures.is_empty() {
                report.false_narrations += 1;
            }
            for (kind, claim, detail) in failures {
                fail(report, kind, place, claim, detail);
            }
        }
    }

    fn narration(&self, text: &str, san_text: &str, walk: &Walk, k: usize) -> Vec<Found> {
        let n = &corpus().narrate;
        let mut templates: Vec<(Option<Tag>, String)> = vec![
            (None, n.capture.clone()),
            (None, n.recapture.clone()),
            (None, n.check.clone()),
            (None, n.forced.clone()),
            (None, n.quiet.clone()),
        ];
        for tag in Tag::ALL {
            templates.push((
                Some(tag),
                n.tactic.replace("{vp}", &corpus().tag[tag.key()].vp),
            ));
        }
        let f = walk.facts(k);
        let mut best: Option<Vec<Found>> = None;
        for (tag, template) in &templates {
            let Some(bindings) = match_template(&tokenize(template), text) else {
                continue;
            };
            let r = Reading {
                kind: "narrate",
                bindings: &bindings,
            };
            let mut out = Vec::new();
            let said = r.get("san").unwrap_or_default();
            if said != san_text {
                out.push((
                    Kind::Narration,
                    text.to_owned(),
                    format!("names {said}, not its move {san_text}"),
                ));
            }
            let mv = san(f.before, f.m);
            if template == &n.capture {
                let ok = self.claims(&r, &mut out).piece.is_some_and(|p| {
                    f.m.is_capture()
                        && p.square == Some(board::captured_square(f.m))
                        && f.before.board().piece_at(board::captured_square(f.m)) == Some(p.piece)
                });
                if !ok {
                    out.push((
                        Kind::Narration,
                        text.to_owned(),
                        format!("{mv} does not take that piece"),
                    ));
                }
            } else if template == &n.recapture {
                let ok = f.m.is_capture() && f.previous.is_some_and(|p| p.to() == f.m.to());
                if !ok {
                    out.push((
                        Kind::Narration,
                        text.to_owned(),
                        format!("{mv} does not take back"),
                    ));
                }
            } else if template == &n.check {
                if !f.after.is_check() {
                    out.push((
                        Kind::Narration,
                        text.to_owned(),
                        format!("{mv} gives no check"),
                    ));
                }
            } else if template == &n.forced {
                if f.before.legal_moves().len() != 1 {
                    out.push((
                        Kind::Narration,
                        text.to_owned(),
                        "there are other legal moves".to_owned(),
                    ));
                }
            } else if let Some(tag) = tag {
                let claims = self.claims(&r, &mut out);
                self.tag_claims(*tag, &f, &claims, &mut out);
                if *tag == Tag::ForcedMate {
                    match mate_within(walk, k, None) {
                        MateCheck::No(why) => out.push((Kind::Mate, text.to_owned(), why)),
                        MateCheck::Yes | MateCheck::Unknown => {}
                    }
                }
            }
            // Narration failures are about the narration, whatever the claim.
            for o in &mut out {
                if o.0 != Kind::Mate {
                    o.0 = Kind::Narration;
                }
            }
            if best.as_ref().is_none_or(|b| out.len() < b.len()) {
                best = Some(out);
            }
            if best.as_ref().is_some_and(Vec::is_empty) {
                break;
            }
        }
        best.unwrap_or_else(|| {
            vec![(
                Kind::Narration,
                text.to_owned(),
                "no narration says this".to_owned(),
            )]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(fen: &str) -> Chess {
        fen.parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap()
    }

    #[test]
    fn templates_match_and_bind() {
        let toks = tokenize("{move} allows {line}, and then {reply} wins {gain}.");
        let b = match_template(
            &toks,
            "Rb1 allows d5 Nxd5 exd5 exd5, and then Nxd5 wins a pawn.",
        )
        .unwrap();
        assert_eq!(b[0], ("move".to_owned(), "Rb1".to_owned()));
        assert_eq!(b[1], ("line".to_owned(), "d5 Nxd5 exd5 exd5".to_owned()));
        assert_eq!(b[2], ("reply".to_owned(), "Nxd5".to_owned()));
        assert_eq!(b[3], ("gain".to_owned(), "a pawn".to_owned()));
        assert!(match_template(&toks, "Rb1 allows d5, and then Nxd5 wins a pawm.").is_none());
    }

    #[test]
    fn piece_lists_parse() {
        let t = parse_targets("Black's queen on d8 and rook on h8").unwrap();
        assert_eq!(t.len(), 2);
        assert_eq!(t[1].owner, Owner::Black);
        assert_eq!(t[1].role, Role::Rook);
        assert_eq!(t[1].square, Some(Square::H8));
        let same = parse_targets("White's pawns on b2, c3 and f2").unwrap();
        assert_eq!(same.len(), 3);
        assert!(
            same.iter()
                .all(|p| p.role == Role::Pawn && p.owner == Owner::White)
        );
        let king = parse_targets("Black's king and queen on d8").unwrap();
        assert_eq!(king[0].square, None);
        assert_eq!(king[1].square, Some(Square::D8));
        assert_eq!(parse_gain("a knight"), Some(201));
        assert_eq!(parse_gain("material worth about 4 pawns"), Some(350));
    }

    #[test]
    fn sans_must_be_legal_and_well_written() {
        let start = Chess::default();
        assert!(parse_san(&start, "e4").is_ok());
        assert!(parse_san(&start, "e5").is_err());
        assert!(parse_san(&start, "Ng1f3").is_err());
        let p = pos("rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2");
        assert_eq!(parse_san(&p, "Qh4").unwrap_err(), "Qh4 is written Qh4#");
    }
}
