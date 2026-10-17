//! PGN import: headers, SAN resolution, start positions, comments.

use std::ops::ControlFlow;

use pgn_reader::{Nag, Outcome, RawComment, RawTag, Reader, SanPlus, Skip, Visitor};
use serde::Serialize;
use shakmaty::fen::Fen;
use shakmaty::san::SanPlus as CanonicalSan;
use shakmaty::{CastlingMode, Chess, Color, Move, Position};
use thiserror::Error;
use ts_rs::TS;

use crate::board;

/// A successfully parsed game.
#[derive(Debug, Clone)]
pub struct ParsedGame {
    /// Zero-based index of the game in the submitted PGN.
    pub index: usize,
    pub headers: Vec<(String, String)>,
    pub castling_mode: CastlingMode,
    /// Positions before each move and after the last one (`moves.len() + 1`).
    pub positions: Vec<Chess>,
    pub moves: Vec<ParsedMove>,
    /// Comment before the first move, if any.
    pub prelude_comment: Option<String>,
    /// Result token from the movetext (`1-0`, `0-1`, `1/2-1/2`, `*`).
    pub movetext_result: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedMove {
    pub mv: Move,
    /// Canonical SAN with check/mate suffix.
    pub san: String,
    pub uci: String,
    pub comment: Option<String>,
    pub nags: Vec<u8>,
}

impl ParsedGame {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .filter(|v| !v.is_empty() && *v != "?" && *v != "-")
    }

    pub fn elo(&self, color: Color) -> Option<u32> {
        let tag = if color == Color::White {
            "WhiteElo"
        } else {
            "BlackElo"
        };
        self.header(tag)
            .and_then(|v| v.trim().parse().ok())
            .filter(|elo| *elo > 0)
    }

    pub fn start_fen(&self) -> String {
        board::fen(&self.positions[0])
    }

    pub fn final_position(&self) -> &Chess {
        self.positions
            .last()
            .expect("at least the initial position")
    }
}

/// Error for one game, or for the whole input.
#[derive(Debug, Clone, Error, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum PgnError {
    #[error("the input contains no games")]
    Empty,
    #[error("the input could not be read: {message}")]
    Unreadable { message: String },
    #[error("game {game}: invalid starting position {fen:?}: {reason}")]
    InvalidStartPosition {
        game: u32,
        fen: String,
        reason: String,
    },
    #[error("game {game}: unsupported variant {variant:?}")]
    UnsupportedVariant { game: u32, variant: String },
    #[error("game {game}, ply {ply} ({move_number}{dots}): {san:?} is not a legal move: {reason}")]
    IllegalMove {
        game: u32,
        ply: u32,
        move_number: u32,
        dots: String,
        san: String,
        reason: String,
    },
}

/// Result of parsing a PGN file: games in order, each either parsed or failed.
pub fn parse_pgn(text: &str) -> Result<Vec<Result<ParsedGame, PgnError>>, PgnError> {
    let mut reader = Reader::new(text.as_bytes());
    let mut visitor = GameVisitor { next_index: 0 };
    let mut games = Vec::new();
    loop {
        match reader.read_game(&mut visitor) {
            Ok(Some(game)) => games.push(game),
            Ok(None) => break,
            Err(e) => {
                return Err(PgnError::Unreadable {
                    message: e.to_string(),
                });
            }
        }
    }
    // A file with only whitespace or comments yields one empty pseudo-game.
    games.retain(|g| !matches!(g, Ok(game) if game.moves.is_empty() && game.headers.is_empty()));
    if games.is_empty() {
        return Err(PgnError::Empty);
    }
    Ok(games)
}

struct GameVisitor {
    next_index: usize,
}

struct Movetext {
    game: ParsedGame,
    position: Chess,
    error: Option<PgnError>,
    pending_comment: Vec<String>,
}

fn decode_tag(value: RawTag<'_>) -> String {
    value.decode_utf8_lossy().into_owned()
}

fn comment_text(comment: RawComment<'_>) -> String {
    String::from_utf8_lossy(comment.as_bytes())
        .trim()
        .to_owned()
}

impl Visitor for GameVisitor {
    type Tags = Vec<(String, String)>;
    type Movetext = Movetext;
    type Output = Result<ParsedGame, PgnError>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(Vec::new())
    }

    fn tag(
        &mut self,
        tags: &mut Self::Tags,
        name: &[u8],
        value: RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        tags.push((
            String::from_utf8_lossy(name).into_owned(),
            decode_tag(value),
        ));
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        let index = self.next_index;
        self.next_index += 1;
        let game_no = index as u32 + 1;
        let find = |name: &str| {
            tags.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.trim().to_owned())
        };

        let variant = find("Variant").unwrap_or_default();
        let castling_mode = match variant.to_ascii_lowercase().as_str() {
            "" | "standard" | "chess" | "from position" => CastlingMode::Standard,
            "chess960" | "chess 960" | "fischerandom" | "fischer random" | "freestyle" => {
                CastlingMode::Chess960
            }
            _ => {
                return ControlFlow::Break(Err(PgnError::UnsupportedVariant {
                    game: game_no,
                    variant,
                }));
            }
        };

        let position = match find("FEN") {
            Some(fen_text) if !fen_text.is_empty() => {
                let parsed = fen_text
                    .parse::<Fen>()
                    .map_err(|e| e.to_string())
                    .and_then(|fen| {
                        fen.into_position::<Chess>(castling_mode)
                            .map_err(|e| e.to_string())
                    });
                match parsed {
                    Ok(pos) => pos,
                    Err(reason) => {
                        return ControlFlow::Break(Err(PgnError::InvalidStartPosition {
                            game: game_no,
                            fen: fen_text,
                            reason,
                        }));
                    }
                }
            }
            _ => Chess::default(),
        };

        ControlFlow::Continue(Movetext {
            game: ParsedGame {
                index,
                headers: tags,
                castling_mode,
                positions: vec![position.clone()],
                moves: Vec::new(),
                prelude_comment: None,
                movetext_result: None,
            },
            position,
            error: None,
            pending_comment: Vec::new(),
        })
    }

    fn san(&mut self, mt: &mut Self::Movetext, san_plus: SanPlus) -> ControlFlow<Self::Output> {
        if mt.error.is_some() {
            return ControlFlow::Continue(());
        }
        let ply = mt.game.moves.len() as u32 + 1;
        match san_plus.san.to_move(&mt.position) {
            Ok(m) => {
                let uci = board::uci(m, mt.game.castling_mode);
                let san =
                    CanonicalSan::from_move_and_play_unchecked(&mut mt.position, m).to_string();
                mt.game.positions.push(mt.position.clone());
                mt.game.moves.push(ParsedMove {
                    mv: m,
                    san,
                    uci,
                    comment: None,
                    nags: Vec::new(),
                });
            }
            Err(e) => {
                let turn = mt.position.turn();
                mt.error = Some(PgnError::IllegalMove {
                    game: mt.game.index as u32 + 1,
                    ply,
                    move_number: mt.position.fullmoves().get(),
                    dots: if turn == Color::White {
                        ".".into()
                    } else {
                        "...".into()
                    },
                    san: san_plus.to_string(),
                    reason: e.to_string(),
                });
            }
        }
        ControlFlow::Continue(())
    }

    fn nag(&mut self, mt: &mut Self::Movetext, nag: Nag) -> ControlFlow<Self::Output> {
        if let Some(last) = mt.game.moves.last_mut() {
            last.nags.push(nag.0);
        }
        ControlFlow::Continue(())
    }

    fn comment(
        &mut self,
        mt: &mut Self::Movetext,
        comment: RawComment<'_>,
    ) -> ControlFlow<Self::Output> {
        let text = comment_text(comment);
        if text.is_empty() {
            return ControlFlow::Continue(());
        }
        match mt.game.moves.last_mut() {
            Some(last) => match &mut last.comment {
                Some(existing) => {
                    existing.push(' ');
                    existing.push_str(&text);
                }
                None => last.comment = Some(text),
            },
            None => mt.pending_comment.push(text),
        }
        ControlFlow::Continue(())
    }

    fn begin_variation(&mut self, _mt: &mut Self::Movetext) -> ControlFlow<Self::Output, Skip> {
        // Sidelines are not reviewed.
        ControlFlow::Continue(Skip(true))
    }

    fn outcome(&mut self, mt: &mut Self::Movetext, outcome: Outcome) -> ControlFlow<Self::Output> {
        mt.game.movetext_result = Some(outcome.to_string());
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, mut mt: Self::Movetext) -> Self::Output {
        if let Some(err) = mt.error {
            return Err(err);
        }
        if !mt.pending_comment.is_empty() {
            mt.game.prelude_comment = Some(mt.pending_comment.join(" "));
        }
        Ok(mt.game)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headers_moves_and_comments() {
        let pgn = r#"[Event "Casual"]
[White "Alice"]
[Black "Bob"]
[WhiteElo "1500"]
[Result "1-0"]

{Opening} 1. e4 e5 2. Qh5 $2 Nc6 {defends} 3. Bc4 Nf6?? 4. Qxf7# 1-0"#;
        let games = parse_pgn(pgn).unwrap();
        let g = games[0].as_ref().unwrap();
        assert_eq!(g.header("White"), Some("Alice"));
        assert_eq!(g.elo(Color::White), Some(1500));
        assert_eq!(g.elo(Color::Black), None);
        assert_eq!(g.moves.len(), 7);
        assert_eq!(g.positions.len(), 8);
        assert_eq!(g.moves[6].san, "Qxf7#");
        assert_eq!(g.moves[6].uci, "h5f7");
        assert_eq!(g.moves[2].nags, vec![2]);
        assert_eq!(g.moves[3].comment.as_deref(), Some("defends"));
        assert_eq!(g.prelude_comment.as_deref(), Some("Opening"));
        assert!(g.final_position().is_checkmate());
        assert_eq!(g.movetext_result.as_deref(), Some("1-0"));
    }

    #[test]
    fn reports_illegal_moves_with_location() {
        let err = parse_pgn("1. e4 e5 2. Ke3 *")
            .unwrap()
            .remove(0)
            .unwrap_err();
        match err {
            PgnError::IllegalMove {
                ply,
                move_number,
                san,
                ..
            } => {
                assert_eq!((ply, move_number, san.as_str()), (3, 2, "Ke3"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn supports_fen_start_and_multiple_games() {
        let pgn = r#"[SetUp "1"]
[FEN "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"]

1. e4 Kd7 *

[Event "Second"]

1. d4 d5 *"#;
        let games = parse_pgn(pgn).unwrap();
        assert_eq!(games.len(), 2);
        let first = games[0].as_ref().unwrap();
        assert_eq!(first.start_fen(), "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
        assert_eq!(first.moves.len(), 2);
        assert_eq!(games[1].as_ref().unwrap().index, 1);
    }

    #[test]
    fn rejects_empty_and_bad_fen() {
        assert!(matches!(parse_pgn("   \n "), Err(PgnError::Empty)));
        let bad = parse_pgn("[FEN \"not a fen\"]\n\n1. e4 *")
            .unwrap()
            .remove(0);
        assert!(matches!(bad, Err(PgnError::InvalidStartPosition { .. })));
    }
}
