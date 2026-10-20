//! Wrong notes for testing the checker: each mutation changes one claim of a
//! sentence or narration, and the checker should find it false.
//!
//! Not every mutation must be caught: a pawn moved to the next file may meet
//! another pawn of the same side there, and a gain made larger may still be
//! won by the line. The rates are reported, not required, except for the
//! mutations no board can make true again (a different move, a narration of
//! another move).

use shakmaty::Square;

use super::super::{CoachNote, Fragment};
use super::is_san_token;

/// What a mutation changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mutation {
    /// A move of the sentence goes to the next file.
    Move,
    /// A named piece stands on the next file.
    Square,
    /// White's piece becomes Black's, or the other way.
    Owner,
    /// The gain grows: a pawn becomes a queen, a knight becomes a queen.
    Gain,
    /// A mate in N becomes a mate in N - 1.
    Mate,
    /// A narration names the next move of its line.
    Narration,
}

impl Mutation {
    pub const ALL: [Mutation; 6] = [
        Mutation::Move,
        Mutation::Square,
        Mutation::Owner,
        Mutation::Gain,
        Mutation::Mate,
        Mutation::Narration,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Mutation::Move => "move",
            Mutation::Square => "square",
            Mutation::Owner => "owner",
            Mutation::Gain => "gain",
            Mutation::Mate => "mate",
            Mutation::Narration => "narration",
        }
    }
}

/// Where the mutated claim is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Sentence(usize),
    Narration { line: usize, ply: usize },
}

/// Every single-claim mutation of a note: what changed, where, and the note.
pub fn mutations(note: &CoachNote) -> Vec<(Mutation, Target, CoachNote)> {
    let mut out = Vec::new();
    for (i, s) in note.sentences.iter().enumerate() {
        for m in [
            Mutation::Move,
            Mutation::Square,
            Mutation::Owner,
            Mutation::Gain,
            Mutation::Mate,
        ] {
            if let Some(fragments) = mutate_fragments(&s.fragments, m) {
                let mut changed = note.clone();
                changed.sentences[i].fragments = fragments;
                out.push((m, Target::Sentence(i), changed));
            }
        }
    }
    for (l, line) in note.show_lines.iter().enumerate() {
        for k in 0..line.narration.len() {
            let (Some(own), Some(other)) = (
                line.moves.get(k).map(|m| m.san.as_str()),
                line.moves.get(k + 1).map(|m| m.san.as_str()),
            ) else {
                continue;
            };
            let Some(rest) = line.narration[k].strip_prefix(own) else {
                continue;
            };
            if own == other {
                continue;
            }
            let mut changed = note.clone();
            changed.show_lines[l].narration[k] = format!("{other}{rest}");
            out.push((
                Mutation::Narration,
                Target::Narration { line: l, ply: k },
                changed,
            ));
        }
    }
    out
}

/// Applies `m` to the first place in the fragments where it applies.
fn mutate_fragments(fragments: &[Fragment], m: Mutation) -> Option<Vec<Fragment>> {
    for (i, f) in fragments.iter().enumerate() {
        if let Some(text) = mutate_text(&f.text, m) {
            let mut out = fragments.to_vec();
            out[i].text = text;
            return Some(out);
        }
    }
    None
}

fn next_file(sq: Square) -> Square {
    let file = (u32::from(sq.file()) + 1) % 8;
    Square::from_coords(shakmaty::File::new(file), sq.rank())
}

/// The text with its last square name moved one file on.
fn shift_last_square(word: &str) -> Option<String> {
    let bytes = word.as_bytes();
    let at = (0..bytes.len().saturating_sub(1))
        .rev()
        .find(|&i| (b'a'..=b'h').contains(&bytes[i]) && (b'1'..=b'8').contains(&bytes[i + 1]))?;
    let sq: Square = word[at..at + 2].parse().ok()?;
    Some(format!(
        "{}{}{}",
        &word[..at],
        next_file(sq),
        &word[at + 2..]
    ))
}

fn mutate_text(text: &str, m: Mutation) -> Option<String> {
    match m {
        Mutation::Move => {
            // The first word that is a SAN with a square.
            let mut start = 0;
            for word in text.split_inclusive([' ', ',', ':', ';']) {
                let token = word.trim_end_matches([' ', ',', ':', ';', '.']);
                if is_san_token(token)
                    && let Some(changed) = shift_last_square(token)
                {
                    return Some(format!(
                        "{}{changed}{}",
                        &text[..start],
                        &text[start + token.len()..]
                    ));
                }
                start += word.len();
            }
            None
        }
        Mutation::Square => {
            let at = text.find(" on ")? + 4;
            let sq: Square = text.get(at..at + 2)?.parse().ok()?;
            Some(format!(
                "{}{}{}",
                &text[..at],
                next_file(sq),
                &text[at + 2..]
            ))
        }
        Mutation::Owner => {
            let (from, to) = if let Some(i) = text.find("White's") {
                (i, "Black's")
            } else {
                (text.find("Black's")?, "White's")
            };
            Some(format!("{}{to}{}", &text[..from], &text[from + 7..]))
        }
        Mutation::Gain => {
            for (from, to) in [
                ("a pawn", "a queen"),
                ("two pawns", "a queen"),
                ("the exchange", "a queen"),
                ("a knight", "a queen"),
                ("a bishop", "a queen"),
                ("a rook", "a queen"),
                ("a queen", "material worth about 20 pawns"),
            ] {
                for verb in ["wins ", "winning "] {
                    let phrase = format!("{verb}{from}");
                    if let Some(i) = text.find(&phrase) {
                        return Some(format!(
                            "{}{verb}{to}{}",
                            &text[..i],
                            &text[i + phrase.len()..]
                        ));
                    }
                }
            }
            let i = text.find("about ")? + 6;
            let digits = text[i..].bytes().take_while(u8::is_ascii_digit).count();
            let n: u32 = text[i..i + digits].parse().ok()?;
            Some(format!("{}{}{}", &text[..i], n + 5, &text[i + digits..]))
        }
        Mutation::Mate => {
            let i = text.find("mate in ")? + 8;
            let digits = text[i..].bytes().take_while(u8::is_ascii_digit).count();
            let n: u32 = text[i..i + digits].parse().ok()?;
            (n >= 2).then(|| format!("{}{}{}", &text[..i], n - 1, &text[i + digits..]))
        }
        Mutation::Narration => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_mutate_one_claim() {
        assert_eq!(
            mutate_text("After Nf6, Qxf7# is checkmate.", Mutation::Move).as_deref(),
            Some("After Ng6, Qxf7# is checkmate.")
        );
        assert_eq!(
            mutate_text("Black's knight on h6", Mutation::Square).as_deref(),
            Some("Black's knight on a6")
        );
        assert_eq!(
            mutate_text(" wins a pawn.", Mutation::Gain).as_deref(),
            Some(" wins a queen.")
        );
        assert_eq!(
            mutate_text("starts a forced mate in 3.", Mutation::Mate).as_deref(),
            Some("starts a forced mate in 2.")
        );
        assert_eq!(mutate_text("O-O.", Mutation::Move), None);
    }
}
