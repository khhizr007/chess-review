import { describe, expect, it } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import { kingSquareToMove, uciSquares } from "./fen";
import { formatEval, formatEvalShort, moveNumberLabel } from "./format";
import { fenAt, matchesFilter, needsAttention, stepFiltered } from "./review";

function ply(n: number, overrides: Partial<PlyReview> = {}): PlyReview {
  return {
    ply: n,
    move_number: Math.ceil(n / 2),
    mover: n % 2 === 1 ? "white" : "black",
    san: `m${n}`,
    uci: "e2e4",
    from: "e2",
    to: "e4",
    fen_before: `before-${n}`,
    fen_after: `after-${n}`,
    flags: { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false },
    eval_before: { kind: "cp", cp: 0 },
    eval_after: { kind: "cp", cp: 0 },
    eval_best: null,
    ep_before: 0.5,
    ep_after: 0.5,
    ep_best: 0.5,
    ep_lost: 0,
    classification: "best",
    miss_kind: null,
    precision: "best",
    rule: "precision",
    best_move: null,
    played_is_best: true,
    show_best_move: false,
    lines: [],
    accuracy: 100,
    phase: "opening",
    is_book: false,
    is_key_moment: false,
    is_critical: false,
    material_lost_cp: null,
    explanation: [],
    coach: { sentences: [], show_lines: [], events: [] },
    pgn_comment: null,
    ...overrides,
  };
}

function review(plies: PlyReview[]): ReviewResult {
  return { game: { start_fen: "start" }, plies } as unknown as ReviewResult;
}

describe("formatEval", () => {
  it("formats centipawns from White's view", () => {
    expect(formatEval({ kind: "cp", cp: 124 })).toBe("+1.24");
    expect(formatEval({ kind: "cp", cp: -30 })).toBe("−0.30");
    expect(formatEval({ kind: "cp", cp: 0 })).toBe("0.00");
  });

  it("formats mates and finished games", () => {
    expect(formatEval({ kind: "mate", winner: "white", moves: 3 })).toBe("M3");
    expect(formatEval({ kind: "mate", winner: "black", moves: 2 })).toBe("−M2");
    expect(formatEval({ kind: "mate", winner: "black", moves: 0 })).toBe("0-1");
    expect(formatEvalShort({ kind: "mate", winner: "white", moves: 0 })).toBe("#");
    expect(formatEvalShort({ kind: "cp", cp: 1530 })).toBe("15");
  });

  it("labels move numbers", () => {
    expect(moveNumberLabel(12, "white")).toBe("12.");
    expect(moveNumberLabel(12, "black")).toBe("12…");
  });
});

describe("review navigation", () => {
  const r = review([
    ply(1, { phase: "opening" }),
    ply(2, { classification: "mistake", phase: "middlegame" }),
    ply(3, { classification: "brilliant", is_critical: true, phase: "middlegame" }),
    ply(4, { classification: "blunder", phase: "endgame" }),
  ]);

  it("maps plies to positions", () => {
    expect(fenAt(r, 0)).toBe("start");
    expect(fenAt(r, 2)).toBe("after-2");
    expect(fenAt(r, 99)).toBe("after-4");
  });

  it("filters like the reference client", () => {
    expect(needsAttention(r.plies[1]!)).toBe(true);
    expect(needsAttention(r.plies[2]!)).toBe(false);
    expect(matchesFilter(r.plies[2]!, "key")).toBe(true);
    expect(matchesFilter(r.plies[3]!, "endgame")).toBe(true);
    expect(matchesFilter(r.plies[0]!, "best")).toBe(true);
  });

  it("steps through filtered moves", () => {
    expect(stepFiltered(r, 0, 1, "attention")).toBe(2);
    expect(stepFiltered(r, 2, 1, "attention")).toBe(4);
    expect(stepFiltered(r, 4, 1, "attention")).toBeNull();
    expect(stepFiltered(r, 4, -1, "attention")).toBe(2);
  });
});

describe("fen helpers", () => {
  it("finds the king of the side to move", () => {
    expect(kingSquareToMove("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")).toBe("e1");
    expect(kingSquareToMove("6k1/5ppp/8/8/8/8/5PPP/3R2K1 b - - 0 1")).toBe("g8");
  });

  it("parses UCI squares", () => {
    expect(uciSquares("e7e8q")).toEqual({ from: "e7", to: "e8" });
    expect(uciSquares("bad")).toBeNull();
  });
});
