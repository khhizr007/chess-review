//! Where an event happens on the board, and the arrows and squares that show it.

use serde::Serialize;
use shakmaty::{Move, Square};
use ts_rs::TS;

use crate::board;

/// The shape of an event, in the position after its move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Geometry {
    /// One piece attacking several (forks).
    Fan {
        source: Square,
        targets: Vec<Square>,
    },
    /// Several pieces aimed at one square (double check, mate).
    Converging {
        target: Square,
        sources: Vec<Square>,
    },
    /// A line piece through `front` to `behind` (pins, skewers).
    XRay {
        attacker: Square,
        front: Square,
        behind: Square,
    },
    /// A defender, what it guards, and the piece attacking that.
    Triangle {
        defended: Square,
        defender: Square,
        attacker: Square,
    },
    /// One piece acting on another.
    Relation {
        from: Square,
        to: Square,
    },
    Squares(Vec<Square>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ArrowKind {
    /// A move of the line.
    Move,
    /// An attack, a pin or an x-ray.
    Attack,
    /// A piece guarding another.
    Defence,
    /// The engine's best move.
    Best,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Arrow {
    pub from: String,
    pub to: String,
    pub kind: ArrowKind,
}

/// Arrows plus highlighted squares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub arrows: Vec<Arrow>,
    pub squares: Vec<String>,
}

impl Marks {
    pub fn arrow(&mut self, from: Square, to: Square, kind: ArrowKind) {
        if from == to {
            return;
        }
        let arrow = Arrow {
            from: from.to_string(),
            to: to.to_string(),
            kind,
        };
        if !self
            .arrows
            .iter()
            .any(|a| a.from == arrow.from && a.to == arrow.to)
        {
            self.arrows.push(arrow);
        }
    }

    pub fn square(&mut self, sq: Square) {
        let name = sq.to_string();
        if !self.squares.contains(&name) {
            self.squares.push(name);
        }
    }

    pub fn of_move(m: Move, kind: ArrowKind) -> Marks {
        let mut marks = Marks::default();
        let (from, to) = board::move_squares(m);
        if let Some(from) = from {
            marks.arrow(from, to, kind);
        }
        marks
    }

    pub fn add(&mut self, other: &Marks) {
        for a in &other.arrows {
            if !self.arrows.iter().any(|b| a.from == b.from && a.to == b.to) {
                self.arrows.push(a.clone());
            }
        }
        for s in &other.squares {
            if !self.squares.contains(s) {
                self.squares.push(s.clone());
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.arrows.is_empty() && self.squares.is_empty()
    }
}

impl Geometry {
    pub fn marks(&self) -> Marks {
        let mut m = Marks::default();
        match self {
            Geometry::Fan { source, targets } => {
                for t in targets {
                    m.arrow(*source, *t, ArrowKind::Attack);
                    m.square(*t);
                }
            }
            Geometry::Converging { target, sources } => {
                for s in sources {
                    m.arrow(*s, *target, ArrowKind::Attack);
                }
                m.square(*target);
            }
            Geometry::XRay {
                attacker,
                front,
                behind,
            } => {
                m.arrow(*attacker, *behind, ArrowKind::Attack);
                m.square(*front);
                m.square(*behind);
            }
            Geometry::Triangle {
                defended,
                defender,
                attacker,
            } => {
                m.arrow(*defender, *defended, ArrowKind::Defence);
                m.arrow(*attacker, *defended, ArrowKind::Attack);
                m.square(*defended);
            }
            Geometry::Relation { from, to } => {
                m.arrow(*from, *to, ArrowKind::Attack);
                m.square(*to);
            }
            Geometry::Squares(squares) => {
                for s in squares {
                    m.square(*s);
                }
            }
        }
        m
    }

    pub fn view(&self) -> GeometryView {
        let s = |sq: &Square| sq.to_string();
        let all = |v: &[Square]| v.iter().map(s).collect();
        match self {
            Geometry::Fan { source, targets } => GeometryView::Fan {
                source: s(source),
                targets: all(targets),
            },
            Geometry::Converging { target, sources } => GeometryView::Converging {
                target: s(target),
                sources: all(sources),
            },
            Geometry::XRay {
                attacker,
                front,
                behind,
            } => GeometryView::XRay {
                attacker: s(attacker),
                front: s(front),
                behind: s(behind),
            },
            Geometry::Triangle {
                defended,
                defender,
                attacker,
            } => GeometryView::Triangle {
                defended: s(defended),
                defender: s(defender),
                attacker: s(attacker),
            },
            Geometry::Relation { from, to } => GeometryView::Relation {
                from: s(from),
                to: s(to),
            },
            Geometry::Squares(squares) => GeometryView::Squares {
                squares: all(squares),
            },
        }
    }
}

/// [`Geometry`] as sent to the frontend (square names).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum GeometryView {
    Fan {
        source: String,
        targets: Vec<String>,
    },
    Converging {
        target: String,
        sources: Vec<String>,
    },
    #[serde(rename = "x_ray")]
    XRay {
        attacker: String,
        front: String,
        behind: String,
    },
    Triangle {
        defended: String,
        defender: String,
        attacker: String,
    },
    Relation {
        from: String,
        to: String,
    },
    Squares {
        squares: Vec<String>,
    },
}
