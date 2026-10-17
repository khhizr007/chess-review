//! Opening book (fallback for the reference engine's embedded book).
//!
//! Loads the public-domain `lichess-org/chess-openings` TSV files
//! (`eco<TAB>name<TAB>pgn`). Every position along every named line is "book";
//! the final position of a line carries its ECO code and name.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use shakmaty::san::San;
use shakmaty::{Chess, Position};
use ts_rs::TS;

use crate::board;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct OpeningName {
    pub eco: String,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct OpeningBook {
    positions: HashMap<String, Option<OpeningName>>,
}

impl OpeningBook {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Loads every `*.tsv` file in `dir`. Malformed lines are skipped and counted.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, usize)> {
        let mut book = Self::default();
        let mut skipped = 0;
        let mut files: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "tsv"))
            .collect();
        files.sort();
        for file in files {
            let text = std::fs::read_to_string(&file)?;
            skipped += book.add_tsv(&text);
        }
        Ok((book, skipped))
    }

    /// Adds TSV content; returns the number of lines that could not be used.
    pub fn add_tsv(&mut self, text: &str) -> usize {
        let mut skipped = 0;
        for line in text.lines() {
            let mut cols = line.splitn(3, '\t');
            let (Some(eco), Some(name), Some(pgn)) = (cols.next(), cols.next(), cols.next()) else {
                skipped += 1;
                continue;
            };
            if eco == "eco" {
                continue; // header
            }
            if !self.add_line(eco, name, pgn) {
                skipped += 1;
            }
        }
        skipped
    }

    fn add_line(&mut self, eco: &str, name: &str, movetext: &str) -> bool {
        let mut pos = Chess::default();
        let mut keys = Vec::new();
        for token in movetext.split_whitespace() {
            let token = token.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
            if token.is_empty() {
                continue;
            }
            let Ok(san) = token.parse::<San>() else {
                return false;
            };
            let Ok(m) = san.to_move(&pos) else {
                return false;
            };
            pos.play_unchecked(m);
            keys.push(board::epd(&pos));
        }
        let Some(last) = keys.pop() else { return false };
        for key in keys {
            self.positions.entry(key).or_insert(None);
        }
        self.positions.insert(
            last,
            Some(OpeningName {
                eco: eco.to_owned(),
                name: name.to_owned(),
            }),
        );
        true
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn contains(&self, pos: &Chess) -> bool {
        self.positions.contains_key(&board::epd(pos))
    }

    pub fn name(&self, pos: &Chess) -> Option<&OpeningName> {
        self.positions
            .get(&board::epd(pos))
            .and_then(Option::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_along_lines_are_book() {
        let mut book = OpeningBook::empty();
        let skipped = book
            .add_tsv("eco\tname\tpgn\nC20\tKing's Pawn Game\t1. e4 e5\nB00\tBroken\t1. e4 Kxe8\n");
        assert_eq!(skipped, 1);
        let mut pos = Chess::default();
        pos.play_unchecked("e4".parse::<San>().unwrap().to_move(&pos).unwrap());
        assert!(book.contains(&pos));
        assert!(book.name(&pos).is_none());
        pos.play_unchecked("e5".parse::<San>().unwrap().to_move(&pos).unwrap());
        assert_eq!(book.name(&pos).map(|n| n.eco.as_str()), Some("C20"));
    }
}
