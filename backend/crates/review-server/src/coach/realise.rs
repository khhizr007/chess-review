//! Turning chosen events into sentences, fragments with board marks, and
//! show-line buttons with a narration per move. All wording comes from
//! `text/en.toml`.

use std::collections::HashMap;
use std::sync::OnceLock;

use review_core::expected_points::ExpectedOutcome;
use serde::Deserialize;
use shakmaty::san::SanPlus;
use shakmaty::{Chess, Color, Move, Position, Role};

use super::circ::{Circ, Event, Gain, Line, LineKind};
use super::detect::{self, Found, Tag};
use super::facts::{PieceAt, value};
use super::geometry::{ArrowKind, Marks};
use super::priority::Selection;
use super::{Fragment, Sentence, ShowLine, Style};
use crate::analysis::to_san_line;
use crate::board;

/// Most moves a show-line replays.
const SHOW_LINE_PLIES: usize = 10;

#[derive(Debug, Deserialize)]
pub struct Corpus {
    pub frame: HashMap<String, Styled>,
    pub tag: HashMap<String, TagText>,
    pub narrate: Narrate,
    pub label: Labels,
    pub words: Words,
    pub outlook: OutlookWords,
}

/// How the game stands for the mover, in the five steps the coach names
/// (the expected-outcome buckets with the extremes folded in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outlook {
    Losing,
    Worse,
    Equal,
    Better,
    Winning,
}

impl Outlook {
    pub fn of(outcome: ExpectedOutcome) -> Outlook {
        match outcome {
            ExpectedOutcome::Lost | ExpectedOutcome::Losing => Outlook::Losing,
            ExpectedOutcome::Worse => Outlook::Worse,
            ExpectedOutcome::Equal => Outlook::Equal,
            ExpectedOutcome::Better => Outlook::Better,
            ExpectedOutcome::Winning | ExpectedOutcome::CompletelyWinning => Outlook::Winning,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct OutlookWords {
    pub losing: String,
    pub worse: String,
    pub equal: String,
    pub better: String,
    pub winning: String,
}

impl OutlookWords {
    pub fn word(&self, o: Outlook) -> &str {
        match o {
            Outlook::Losing => &self.losing,
            Outlook::Worse => &self.worse,
            Outlook::Equal => &self.equal,
            Outlook::Better => &self.better,
            Outlook::Winning => &self.winning,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Styled {
    pub you: Option<Vec<String>>,
    pub opponent: Option<Vec<String>>,
    pub neutral: Vec<String>,
}

impl Styled {
    pub fn get(&self, style: Style) -> &[String] {
        let own = match style {
            Style::You => self.you.as_deref(),
            Style::Opponent => self.opponent.as_deref(),
            Style::Neutral => None,
        };
        own.unwrap_or(&self.neutral)
    }
}

#[derive(Debug, Deserialize)]
pub struct TagText {
    pub vp: String,
    pub wins: Option<String>,
    pub vp_long: Option<String>,
    pub label: String,
    /// The lowest rating at which the coach says this tag (default: any).
    #[serde(default)]
    pub min_rating: i32,
}

#[derive(Debug, Deserialize)]
pub struct Narrate {
    pub tactic: String,
    pub capture: String,
    pub recapture: String,
    pub check: String,
    pub forced: String,
    pub quiet: String,
}

#[derive(Debug, Deserialize)]
pub struct Labels {
    pub best: String,
    pub threat: String,
}

#[derive(Debug, Deserialize)]
pub struct Words {
    pub pawn: String,
    pub knight: String,
    pub bishop: String,
    pub rook: String,
    pub queen: String,
    pub king: String,
    pub you: String,
    pub opponent: String,
    pub white: String,
    pub black: String,
    pub your: String,
    pub their: String,
    pub white_s: String,
    pub black_s: String,
    pub piece: String,
    pub piece_role: String,
    pub pieces_same: String,
    pub king_piece: String,
    pub list_comma: String,
    pub list_and: String,
    pub wins_default: String,
    pub gain_piece: String,
    pub gain_exchange: String,
    pub gain_pawns: String,
    pub gain_material: String,
    pub numbers: Vec<String>,
}

impl Words {
    pub fn role(&self, role: Role) -> &str {
        match role {
            Role::Pawn => &self.pawn,
            Role::Knight => &self.knight,
            Role::Bishop => &self.bishop,
            Role::Rook => &self.rook,
            Role::Queen => &self.queen,
            Role::King => &self.king,
        }
    }

    fn number(&self, n: i32) -> String {
        usize::try_from(n)
            .ok()
            .and_then(|i| self.numbers.get(i).cloned())
            .unwrap_or_else(|| n.to_string())
    }
}

/// The parsed text corpus.
pub fn corpus() -> &'static Corpus {
    static CORPUS: OnceLock<Corpus> = OnceLock::new();
    CORPUS.get_or_init(|| toml::from_str(include_str!("text/en.toml")).expect("coach text parses"))
}

/// Who is talking to whom.
#[derive(Debug, Clone, Copy)]
pub struct Voice {
    pub style: Style,
    /// The side that played the reviewed move.
    pub mover: Color,
}

impl Voice {
    fn owner<'w>(&self, color: Color, w: &'w Words) -> &'w str {
        match self.style {
            Style::You if color == self.mover => &w.your,
            Style::You => &w.their,
            Style::Opponent if color == self.mover => &w.their,
            Style::Opponent => &w.your,
            Style::Neutral => color.fold_wb(&w.white_s, &w.black_s),
        }
    }

    fn subject<'w>(&self, w: &'w Words) -> &'w str {
        match self.style {
            Style::You => &w.you,
            Style::Opponent => &w.opponent,
            Style::Neutral => self.mover.fold_wb(&w.white, &w.black),
        }
    }

    pub fn piece(&self, p: &PieceAt, with_square: bool) -> String {
        let w = &corpus().words;
        let template = if p.role == Role::King {
            &w.king_piece
        } else if with_square {
            &w.piece
        } else {
            &w.piece_role
        };
        template
            .replace("{owner}", self.owner(p.color, w))
            .replace("{role}", w.role(p.role))
            .replace("{square}", &p.square.to_string())
    }

    /// "your queen on d8 and rook on h8": the owner is named once per run of
    /// pieces with the same owner.
    pub fn pieces(&self, pieces: &[PieceAt]) -> String {
        let w = &corpus().words;
        let join = |items: Vec<String>| {
            let mut out = String::new();
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(if i + 1 == items.len() {
                        &w.list_and
                    } else {
                        &w.list_comma
                    });
                }
                out.push_str(item);
            }
            out
        };
        // "Black's pawns on b7 and f7".
        if let [first, rest @ ..] = pieces
            && !rest.is_empty()
            && first.role != Role::King
            && rest
                .iter()
                .all(|p| p.color == first.color && p.role == first.role)
        {
            return w
                .pieces_same
                .replace("{owner}", self.owner(first.color, w))
                .replace("{role}", w.role(first.role))
                .replace(
                    "{squares}",
                    &join(pieces.iter().map(|p| p.square.to_string()).collect()),
                );
        }
        let mut out = String::new();
        for (i, p) in pieces.iter().enumerate() {
            if i > 0 {
                out.push_str(if i + 1 == pieces.len() {
                    &w.list_and
                } else {
                    &w.list_comma
                });
            }
            let text = self.piece(p, true);
            if i > 0 && pieces[i - 1].color == p.color {
                let owner = self.owner(p.color, w);
                out.push_str(text.strip_prefix(owner).map_or(&text[..], str::trim_start));
            } else {
                out.push_str(&text);
            }
        }
        out
    }
}

/// "a knight", "the exchange", "two pawns".
pub fn gain_text(gain: &Gain) -> String {
    let w = &corpus().words;
    if let Some(role) = gain.role.filter(|r| *r != Role::King) {
        if (gain.cp - value(role)).abs() < 100 {
            return w.gain_piece.replace("{role}", w.role(role));
        }
        if role == Role::Rook && (150..=250).contains(&gain.cp) {
            return w.gain_exchange.clone();
        }
    }
    let pawns = ((gain.cp + 50) / 100).max(1);
    match pawns {
        1 => w.gain_piece.replace("{role}", &w.pawn),
        2 => w.gain_pawns.replace("{count}", &w.number(2)),
        n => w.gain_material.replace("{count}", &n.to_string()),
    }
}

/// A placeholder's text and the board marks its fragment carries.
#[derive(Debug, Clone, Default)]
struct Value {
    text: String,
    marks: Marks,
}

impl Value {
    fn plain(text: impl Into<String>) -> Value {
        Value {
            text: text.into(),
            marks: Marks::default(),
        }
    }

    fn marked(text: impl Into<String>, marks: Marks) -> Value {
        Value {
            text: text.into(),
            marks,
        }
    }
}

type Values = HashMap<&'static str, Value>;

/// Renders a sentence from `template`, splicing the rendered `pred` in place
/// of `{pred}`, and capitalises it.
fn render(template: &str, values: &Values, pred: Option<&str>) -> Vec<Fragment> {
    let mut out = render_part(template, values, pred);
    // A sentence opening with a move keeps the move as written ("exd5 ...").
    let opens_with_move = ["{move}", "{reply}", "{best}", "{line}", "{san}"]
        .iter()
        .any(|p| template.starts_with(p));
    if !opens_with_move {
        capitalise(&mut out);
    }
    out
}

fn render_part(template: &str, values: &Values, pred: Option<&str>) -> Vec<Fragment> {
    let mut out: Vec<Fragment> = Vec::new();
    let push = |out: &mut Vec<Fragment>, text: &str, marks: &Marks| {
        if text.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(last) if marks.is_empty() && last.arrows.is_empty() && last.squares.is_empty() => {
                last.text.push_str(text);
            }
            _ => out.push(Fragment {
                text: text.to_owned(),
                arrows: marks.arrows.clone(),
                squares: marks.squares.clone(),
            }),
        }
    };
    let none = Marks::default();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        push(&mut out, &rest[..open], &none);
        let Some(close) = rest[open..].find('}') else {
            push(&mut out, &rest[open..], &none);
            rest = "";
            break;
        };
        let name = &rest[open + 1..open + close];
        if name == "pred"
            && let Some(p) = pred
        {
            for f in render_part(p, values, None) {
                let marks = Marks {
                    arrows: f.arrows.clone(),
                    squares: f.squares.clone(),
                };
                push(&mut out, &f.text, &marks);
            }
        } else {
            match values.get(name) {
                Some(v) => push(&mut out, &v.text, &v.marks),
                None => push(&mut out, &rest[open..=open + close], &none),
            }
        }
        rest = &rest[open + close + 1..];
    }
    push(&mut out, rest, &none);
    out
}

fn capitalise(fragments: &mut [Fragment]) {
    if let Some(first) = fragments.iter_mut().find(|f| !f.text.is_empty()) {
        let mut chars = first.text.chars();
        if let Some(c) = chars.next() {
            first.text = c.to_uppercase().chain(chars).collect();
        }
    }
}

fn plain_text(fragments: &[Fragment]) -> String {
    fragments.iter().map(|f| f.text.as_str()).collect()
}

/// SAN of every move of a line.
fn sans(line: &Line) -> Vec<String> {
    line.moves
        .iter()
        .zip(&line.positions)
        .map(|(m, pos)| san(pos, *m))
        .collect()
}

fn san(pos: &Chess, m: Move) -> String {
    let mut p = pos.clone();
    SanPlus::from_move_and_play_unchecked(&mut p, m).to_string()
}

/// Classifications with wording of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Special {
    Brilliant,
    GreatFind,
    Book,
}

impl Special {
    /// Prefix of its frames in `text/en.toml`.
    fn key(self) -> &'static str {
        match self {
            Special::Brilliant => "brilliant",
            Special::GreatFind => "great_find",
            Special::Book => "book",
        }
    }
}

/// Everything the realiser needs about one reviewed move.
pub struct Context<'a> {
    pub voice: Voice,
    pub seed: u32,
    pub before: &'a Chess,
    pub played: Move,
    pub previous: Option<Move>,
    pub reply: Option<&'a Line>,
    pub best: Option<&'a Line>,
    /// The played move's own line.
    pub own: Option<&'a Line>,
    /// N: the null-move line after the played move.
    pub threat: Option<&'a Line>,
    /// N_prev: the null-move line before it.
    pub prev_threat: Option<&'a Line>,
    /// N_B: the null-move line after the best move.
    pub best_threat: Option<&'a Line>,
    /// The expected outcome for the mover before and after the move.
    pub swing: Option<(ExpectedOutcome, ExpectedOutcome)>,
    /// The move is a Miss: a missed idea is said as "missed", not "better was".
    pub miss: bool,
    /// The move is good: no "better was" sentence.
    pub good: bool,
    pub special: Option<Special>,
    /// The best move is hard to find at the player's level: what was missed
    /// is said gently (frames ending `_hard`).
    pub hard: bool,
    /// The bad move is a natural one at the player's level: the sentence
    /// leads with the check it skipped (frames ending `_natural`).
    pub natural: bool,
}

impl Context<'_> {
    pub fn line(&self, kind: LineKind) -> Option<&Line> {
        match kind {
            LineKind::Reply => self.reply,
            LineKind::Best => self.best,
            LineKind::Played => self.own,
            LineKind::Threat => self.threat,
            LineKind::PrevThreat => self.prev_threat,
            LineKind::BestThreat => self.best_threat,
        }
    }

    /// The move before a line's first move (none after a null move).
    fn previous_for(&self, kind: LineKind) -> Option<Move> {
        match kind {
            LineKind::Reply => Some(self.played),
            LineKind::Best | LineKind::Played => self.previous,
            LineKind::Threat | LineKind::PrevThreat | LineKind::BestThreat => None,
        }
    }

    fn has_frame(&self, key: &str) -> bool {
        corpus().frame.contains_key(key)
    }

    /// The frame `key` in its difficulty wording, when it has one: `_hard`
    /// for what was missed when the best move is hard to find, `_natural`
    /// for what the move allows when it is a natural move.
    fn variant(&self, key: &str, e: Option<&Event>) -> String {
        let missed = e.is_none_or(|e| matches!(e.first(), Circ::Misses | Circ::MissesSame));
        let allowed = e.is_some_and(|e| matches!(e.first(), Circ::Base | Circ::Permits));
        let suffix = if self.hard && missed {
            "_hard"
        } else if self.natural && allowed {
            "_natural"
        } else {
            ""
        };
        let wanted = format!("{key}{suffix}");
        if self.has_frame(&wanted) {
            wanted
        } else {
            key.to_owned()
        }
    }

    fn frame(&self, key: &str) -> &'static str {
        frame(key, self.voice.style, self.seed)
    }

    fn common_values(&self) -> Values {
        let w = &corpus().words;
        let mut v = Values::new();
        v.insert("move", Value::plain(san(self.before, self.played)));
        v.insert("mover", Value::plain(self.voice.subject(w)));
        v.insert(
            "mover_s",
            Value::plain(self.voice.owner(self.voice.mover, w)),
        );
        if let Some((from, to)) = self.swing {
            let o = &corpus().outlook;
            v.insert("from", Value::plain(o.word(Outlook::of(from))));
            v.insert("to", Value::plain(o.word(Outlook::of(to))));
        }
        if let Some(best) = self.best.and_then(|b| b.moves.first().map(|m| (b, *m))) {
            v.insert(
                "best",
                Value::marked(
                    san(&best.0.start, best.1),
                    Marks::of_move(best.1, ArrowKind::Best),
                ),
            );
        }
        v
    }

    fn found_values(&self, v: &mut Values, found: &Found, marks: &Marks) {
        if let Some(focus) = &found.focus {
            v.insert(
                "piece",
                Value::marked(self.voice.piece(focus, true), marks.clone()),
            );
            v.insert(
                "piece_role",
                Value::marked(self.voice.piece(focus, false), marks.clone()),
            );
            let mut square = Marks::default();
            square.square(focus.square);
            v.insert("square", Value::marked(focus.square.to_string(), square));
        }
        v.insert(
            "targets",
            Value::marked(self.voice.pieces(&found.targets), marks.clone()),
        );
    }

    /// Placeholder values for an event on its line.
    fn event_values(&self, e: &Event, line: &Line) -> Values {
        let mut v = self.common_values();
        let sans = sans(line);
        let mut marks = e.found.geometry.marks();
        // A bad move's BASE event is about the move before its (reply) line.
        let before_line = e.first() == Circ::Base && e.line == LineKind::Reply;
        if !before_line && let Some(m) = line.moves.get(e.ply) {
            let mut with_move = Marks::of_move(*m, ArrowKind::Move);
            with_move.add(&marks);
            marks = with_move;
        }
        let reply_ply = if before_line { 0 } else { e.ply };
        if let Some(s) = sans.get(reply_ply) {
            let reply_marks = if before_line {
                Marks::of_move(line.moves[0], ArrowKind::Move)
            } else {
                marks.clone()
            };
            v.insert("reply", Value::marked(s.clone(), reply_marks));
        }
        v.insert(
            "line",
            Value::plain(sans[..e.ply.min(sans.len())].join(" ")),
        );
        self.found_values(&mut v, &e.found, &marks);
        if let Some(g) = &e.gain {
            v.insert("gain", Value::plain(gain_text(g)));
        }
        if let Some(n) = e.mate_in {
            v.insert("n", Value::plain(n.to_string()));
        }
        v
    }

    fn pred(&self, e: &Event) -> String {
        let t = tag_text(e.tag());
        if e.tag().is_mate() && e.mate_in.is_some_and(|n| n > 1) {
            return t.vp_long.clone().unwrap_or_else(|| t.vp.clone());
        }
        let wins = e
            .chain
            .iter()
            .any(|c| matches!(c, Circ::WinsMaterial | Circ::LeadsTo));
        if wins && e.gain.is_some() {
            return t
                .wins
                .clone()
                .unwrap_or_else(|| corpus().words.wins_default.replace("{vp}", &t.vp));
        }
        t.vp.clone()
    }

    fn sentence(&self, key: &str, e: &Event, line: &Line, index: usize) -> Sentence {
        let values = self.event_values(e, line);
        Sentence {
            fragments: render(self.frame(key), &values, Some(&self.pred(e))),
            event: Some(index as u32),
        }
    }

    fn narrate(&self, line: &Line, kind: LineKind, ply: usize, sans: &[String]) -> String {
        let n = &corpus().narrate;
        let ctx = line.ctx(ply, self.previous_for(kind));
        let san = &sans[ply];
        let mut values = Values::new();
        values.insert("san", Value::plain(san.clone()));
        if let Some(found) = detect::best_tactic(&ctx) {
            self.found_values(&mut values, &found, &Marks::default());
            let template = n.tactic.replace("{vp}", &tag_text(found.tag).vp);
            return plain_text(&render(&template, &values, None));
        }
        let m = ctx.m;
        let template = if let Some(role) = m.capture() {
            let recapture = ctx
                .previous
                .is_some_and(|p| p.is_capture() && p.to() == m.to());
            if recapture {
                &n.recapture
            } else {
                let captured = PieceAt {
                    role,
                    color: ctx.mover().other(),
                    square: board::captured_square(m),
                };
                values.insert("piece", Value::plain(self.voice.piece(&captured, true)));
                &n.capture
            }
        } else if ctx.after.is_check() {
            &n.check
        } else if ctx.before.legal_moves().len() == 1 {
            &n.forced
        } else {
            &n.quiet
        };
        plain_text(&render(template, &values, None))
    }

    fn show_line(
        &self,
        label: &str,
        e: Option<&Event>,
        kind: LineKind,
        index: Option<usize>,
    ) -> Option<ShowLine> {
        let line = self.line(kind)?;
        if line.moves.is_empty() {
            return None;
        }
        // Through the mate, or two plies past the event and the gain.
        let wanted = match e {
            Some(e) if e.mate_in.is_some() => line.moves.len(),
            Some(e) => e.ply.max(e.gain.map_or(0, |g| g.ply)) + 2,
            None => 6,
        };
        let end = wanted.clamp(1, line.moves.len().min(SHOW_LINE_PLIES));
        let sans = sans(line);
        Some(ShowLine {
            label: label.to_owned(),
            start_fen: board::fen(&line.start),
            moves: to_san_line(&line.start, &line.uci, end),
            narration: (0..end)
                .map(|k| self.narrate(line, kind, k, &sans))
                .collect(),
            event: index.map(|i| i as u32),
        })
    }
}

/// One wording of a frame in a style, picked by the seed.
fn frame(key: &str, style: Style, seed: u32) -> &'static str {
    let variants = corpus()
        .frame
        .get(key)
        .unwrap_or_else(|| panic!("frame {key} in en.toml"))
        .get(style);
    let i = seed as usize % variants.len().max(1);
    variants.get(i).map_or("", String::as_str)
}

/// The note for a forced move: it was the only legal move.
pub fn forced(before: &Chess, played: Move, voice: Voice, seed: u32) -> Sentence {
    let mut values = Values::new();
    values.insert(
        "move",
        Value::marked(san(before, played), Marks::of_move(played, ArrowKind::Move)),
    );
    values.insert("mover", Value::plain(voice.subject(&corpus().words)));
    Sentence {
        fragments: render(frame("forced", voice.style, seed), &values, None),
        event: None,
    }
}

fn tag_text(tag: detect::Tag) -> &'static TagText {
    corpus()
        .tag
        .get(tag.key())
        .unwrap_or_else(|| panic!("tag {} in en.toml", tag.key()))
}

/// The lowest rating at which the coach says `tag`.
pub fn min_rating(tag: detect::Tag) -> i32 {
    tag_text(tag).min_rating
}

/// Lines leading to the event longer than this are not spelled out in the
/// sentence (the show-line has them).
const LONG_LEAD_PLIES: usize = 4;

/// The best move's idea is on a null-move line (what it threatens or stops).
fn on_null_line(e: &Event) -> bool {
    matches!(e.chain.get(1), Some(Circ::Threatens | Circ::Prevents))
}

fn main_frame(e: &Event) -> &'static str {
    let long = e.ply > LONG_LEAD_PLIES;
    let missed = matches!(e.first(), Circ::Misses | Circ::MissesSame);
    // Events about the move as a whole have frames of their own.
    match e.tag() {
        Tag::Sacrifice | Tag::OffersPiece if missed => return "misses_sacrifice",
        Tag::Sacrifice | Tag::OffersPiece => return "sacrifice",
        Tag::OnlyMove if missed => return "misses_only",
        Tag::OnlyMove => return "only",
        Tag::Slide => return "slide",
        _ => {}
    }
    if e.line == LineKind::Played {
        return match e.chain.as_slice() {
            [_, Circ::LeadsTo, ..] if long => "good_long",
            [_, Circ::LeadsTo, ..] => "good_line",
            _ => "good",
        };
    }
    match e.chain.as_slice() {
        // Said when the piece is lost in an exchange, not for free (a free
        // piece the reply takes is said instead).
        [Circ::Base]
            if e.gain.is_some() && matches!(e.tag(), Tag::IgnoresThreat | Tag::BadTrade) =>
        {
            "base_loses"
        }
        [Circ::Base] if e.gain.is_some() => "base_refuted",
        [Circ::Base, ..] => "base",
        [Circ::Threatens, ..] => "threatens",
        [Circ::Prevents, ..] => "prevents",
        [Circ::Permits, Circ::LeadsTo, ..] if long => "permits_long",
        [Circ::Permits, Circ::LeadsTo, ..] => "permits_line",
        [Circ::Permits, ..] => "permits",
        [Circ::Misses, Circ::LeadsTo, ..] if long => "misses_long",
        [Circ::Misses, Circ::LeadsTo, ..] => "misses_line",
        [Circ::Misses, Circ::Threatens, ..] => "misses_threatens",
        [Circ::Misses, Circ::Prevents, ..] => "misses_prevents",
        [Circ::MissesSame, ..] => "misses_same",
        _ => "misses",
    }
}

fn best_frame(e: &Event) -> &'static str {
    match e.tag() {
        Tag::Sacrifice | Tag::OffersPiece => return "best_sacrifice",
        Tag::OnlyMove => return "best_only",
        _ => {}
    }
    match e.chain.as_slice() {
        [Circ::Misses, Circ::LeadsTo, ..] if e.ply > LONG_LEAD_PLIES => "best_long",
        [Circ::Misses, Circ::LeadsTo, ..] => "best_line",
        [Circ::Misses, Circ::Threatens, ..] => "best_threatens",
        [Circ::Misses, Circ::Prevents, ..] => "best_prevents",
        _ => "best_event",
    }
}

/// Sentences and show-lines for the selected events.
pub fn realise(
    events: &[Event],
    selection: Selection,
    ctx: &Context<'_>,
) -> (Vec<Sentence>, Vec<ShowLine>) {
    let mut sentences = Vec::new();
    let mut show_lines = Vec::new();
    // A Brilliant or Great move said in a sentence of its own, when the
    // event's frame has no wording for it.
    let special_sentence = |sentences: &mut Vec<Sentence>| {
        if let Some(s) = ctx.special {
            let mut values = ctx.common_values();
            values.insert(
                "move",
                Value::marked(
                    san(ctx.before, ctx.played),
                    Marks::of_move(ctx.played, ArrowKind::Move),
                ),
            );
            sentences.push(Sentence {
                fragments: render(ctx.frame(s.key()), &values, None),
                event: None,
            });
        }
    };
    let main = selection
        .main
        .and_then(|i| ctx.line(events[i].line).map(|line| (i, &events[i], line)));
    let Some((main_index, main, main_line)) = main else {
        special_sentence(&mut sentences);
        return (sentences, show_lines);
    };
    // Only a Miss is told it "missed" something; otherwise the idea is what was better.
    let missed = matches!(main.first(), Circ::Misses | Circ::MissesSame);
    let frame = if missed && !ctx.miss {
        best_frame(main)
    } else {
        main_frame(main)
    };
    let special_frame = ctx
        .special
        .map(|s| format!("{}_{frame}", s.key()))
        .filter(|k| ctx.has_frame(k));
    match special_frame {
        Some(key) => sentences.push(ctx.sentence(&key, main, main_line, main_index)),
        None => {
            special_sentence(&mut sentences);
            let key = ctx.variant(frame, Some(main));
            sentences.push(ctx.sentence(&key, main, main_line, main_index));
        }
    }
    let label = if matches!(main.first(), Circ::Threatens | Circ::Prevents) || on_null_line(main) {
        &corpus().label.threat
    } else {
        &tag_text(main.tag()).label
    };
    show_lines.extend(ctx.show_line(label, Some(main), main.line, Some(main_index)));

    // What a Brilliant sacrifice leads to.
    if let Some(i) = selection.follow {
        let e = &events[i];
        if let Some(line) = ctx.line(e.line) {
            sentences.push(ctx.sentence(main_frame(e), e, line, i));
            if e.line != main.line {
                show_lines.extend(ctx.show_line(
                    &tag_text(e.tag()).label,
                    Some(e),
                    e.line,
                    Some(i),
                ));
            }
        }
    }

    let main_is_miss = matches!(main.first(), Circ::Misses | Circ::MissesSame);
    let best_label = &corpus().label.best;
    match selection.best {
        Some(i) => {
            let e = &events[i];
            if let Some(line) = ctx.line(e.line) {
                let key = ctx.variant(best_frame(e), Some(e));
                sentences.push(ctx.sentence(&key, e, line, i));
                // A threat or a defence of the best move is shown on the
                // best line itself.
                show_lines.extend(if on_null_line(e) {
                    ctx.show_line(best_label, None, LineKind::Best, Some(i))
                } else {
                    ctx.show_line(best_label, Some(e), e.line, Some(i))
                });
            }
        }
        None if !ctx.good && !main_is_miss && ctx.best.is_some() => {
            let values = ctx.common_values();
            if values.contains_key("best") {
                sentences.push(Sentence {
                    fragments: render(ctx.frame(&ctx.variant("best", None)), &values, None),
                    event: None,
                });
                show_lines.extend(ctx.show_line(best_label, None, LineKind::Best, None));
            }
        }
        None => {}
    }
    (sentences, show_lines)
}
