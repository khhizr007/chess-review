import { describe, expect, it } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { PlayedMove } from "./chess";
import { inSideline, play, sidelineLabels, stepSideline, type BoardState } from "./explore";

const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
const mv = (uci: string): PlayedMove => ({ san: uci, uci, fen: `fen-${uci}`, from: uci.slice(0, 2), to: uci.slice(2, 4), flags: NONE });
// The game: 1. e4 e5 2. Nf3 (only the UCI matters here).
const plies = ["e2e4", "e7e5", "g1f3"].map((uci) => ({ uci }) as PlyReview);

describe("play", () => {
  it("follows the game when the game's own move is played", () => {
    expect(play({ ply: 1, sideline: null }, plies, mv("e7e5"))).toEqual({ ply: 2, sideline: null });
  });

  it("starts a sideline at the shown game position for any other move", () => {
    const s = play({ ply: 1, sideline: null }, plies, mv("c7c5"));
    expect(s).toEqual({ ply: 1, sideline: { fromPly: 1, moves: [mv("c7c5")], cursor: 1 } });
    expect(inSideline(s)).toBe(true);
  });

  it("extends the sideline, and replaces what follows the cursor", () => {
    let s: BoardState = play({ ply: 1, sideline: null }, plies, mv("c7c5"));
    s = play(s, plies, mv("g1f3"));
    s = play(s, plies, mv("d7d6"));
    expect(s.sideline?.moves.map((m) => m.uci)).toEqual(["c7c5", "g1f3", "d7d6"]);
    s = { ...s, sideline: stepSideline(stepSideline(s.sideline!, -1)!, -1) };
    s = play(s, plies, mv("b1c3"));
    expect(s.sideline?.moves.map((m) => m.uci)).toEqual(["c7c5", "b1c3"]);
    expect(s.sideline?.cursor).toBe(2);
  });

  it("keeps the rest of the sideline when its own next move is played again", () => {
    let s: BoardState = play({ ply: 1, sideline: null }, plies, mv("c7c5"));
    s = play(s, plies, mv("g1f3"));
    s = { ...s, sideline: stepSideline(s.sideline!, -1) };
    s = play(s, plies, mv("g1f3"));
    expect(s.sideline).toEqual({ fromPly: 1, moves: [mv("c7c5"), mv("g1f3")], cursor: 2 });
  });

  it("from the branch position, re-enters the sideline or starts a new one", () => {
    const open: BoardState = { ply: 1, sideline: { fromPly: 1, moves: [mv("c7c5"), mv("g1f3")], cursor: 0 } };
    expect(inSideline(open)).toBe(false);
    expect(play(open, plies, mv("c7c5")).sideline?.cursor).toBe(1);
    expect(play(open, plies, mv("e7e6")).sideline).toEqual({ fromPly: 1, moves: [mv("e7e6")], cursor: 1 });
    expect(play(open, plies, mv("e7e5"))).toEqual({ ply: 2, sideline: null });
  });

  it("branches from the final position, where there is no game move to follow", () => {
    expect(play({ ply: 3, sideline: null }, plies, mv("b8c6")).sideline?.fromPly).toBe(3);
  });
});

describe("stepSideline", () => {
  const line = { fromPly: 1, moves: [mv("c7c5"), mv("g1f3")], cursor: 1 };
  it("steps inside the sideline and down to its branch position", () => {
    expect(stepSideline(line, 1)?.cursor).toBe(2);
    expect(stepSideline(line, -1)?.cursor).toBe(0);
  });
  it("returns null past either end", () => {
    expect(stepSideline({ ...line, cursor: 0 }, -1)).toBeNull();
    expect(stepSideline({ ...line, cursor: 2 }, 1)).toBeNull();
  });
});

describe("sidelineLabels", () => {
  it("numbers moves from the branch position's side and move number", () => {
    expect(sidelineLabels("x w KQkq - 0 1", 3)).toEqual([
      { number: "1.", mover: "white" },
      { number: null, mover: "black" },
      { number: "2.", mover: "white" },
    ]);
    expect(sidelineLabels("x b KQkq - 0 12", 3)).toEqual([
      { number: "12...", mover: "black" },
      { number: "13.", mover: "white" },
      { number: null, mover: "black" },
    ]);
  });
});
