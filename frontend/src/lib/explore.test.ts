import { describe, expect, it } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { PlayedMove } from "./chess";
import { back, inSideline, listedBranch, moveLabel, nextMoves, play, sidelineLabels, sidelineOf, type BoardState } from "./explore";

const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
const mv = (uci: string): PlayedMove => ({ san: uci, uci, fen: `fen-${uci}`, from: uci.slice(0, 2), to: uci.slice(2, 4), flags: NONE });
// The game the rules were observed on (Chess.com, 2026-10-22): 1. d4 c6 2. Nf3 d5 3. Bf4 (only the UCI matters here).
const plies = ["d2d4", "c7c6", "g1f3", "d7d5", "c1f4"].map((uci) => ({ uci }) as PlyReview);
const start: BoardState = { ply: 2, path: [], explored: {} };
const playAll = (s: BoardState, ucis: string[]) => ucis.reduce((b, u) => play(b, plies, mv(u)), s);
const ucis = (s: BoardState) => sidelineOf(s)?.moves.map((m) => m.uci);
// After 1. d4 c6, the reader explored 2. e4 d5 3. exd5 instead of 2. Nf3.
const explored = playAll(start, ["e2e4", "d7d5", "e4d5"]);

describe("play", () => {
  it("follows the game when the game's own move is played", () => {
    expect(play(start, plies, mv("g1f3"))).toEqual({ ply: 3, path: [], explored: {} });
  });

  it("starts a line at the shown game position for any other move", () => {
    const s = play(start, plies, mv("e2e4"));
    expect(s).toMatchObject({ ply: 2, path: [0] });
    expect(inSideline(s)).toBe(true);
    expect(sidelineOf(s)).toEqual({ fromPly: 2, moves: [mv("e2e4")], cursor: 1 });
  });

  it("extends the line at its end", () => {
    expect(explored.path).toEqual([0, 0, 0]);
    expect(ucis(explored)).toEqual(["e2e4", "d7d5", "e4d5"]);
  });

  it("re-enters an explored move played again, with everything after it", () => {
    const left = { ...explored, path: [] };
    const again = play(left, plies, mv("e2e4"));
    expect(again.path).toEqual([0]);
    expect(again.explored).toBe(explored.explored);
    expect(sidelineOf(again)).toMatchObject({ moves: [mv("e2e4"), mv("d7d5"), mv("e4d5")], cursor: 1 });
  });

  it("adds another line from the same position, keeping the first", () => {
    const s = play({ ...explored, path: [] }, plies, mv("f2f4"));
    expect(s.path).toEqual([1]);
    expect(s.explored[2]?.map((n) => n.move.uci)).toEqual(["e2e4", "f2f4"]);
    expect(ucis({ ...s, path: [0] })).toEqual(["e2e4", "d7d5", "e4d5"]);
  });

  it("inside a line, adds a different move as a variation instead of dropping the rest", () => {
    const s = play({ ...explored, path: [0] }, plies, mv("e7e5"));
    expect(s.path).toEqual([0, 1]);
    expect(ucis(s)).toEqual(["e2e4", "e7e5"]);
    // The line itself is kept: from 2. e4, its own move 2... d5 still comes first.
    expect(ucis({ ...s, path: [0] })).toEqual(["e2e4", "d7d5", "e4d5"]);
    expect(nextMoves({ ...s, path: [0] })?.map((n) => n.move.uci)).toEqual(["d7d5", "e7e5"]);
  });

  it("keeps lines from different game positions apart", () => {
    const s = playAll({ ...explored, ply: 3, path: [] }, ["g7g6"]);
    expect(s).toMatchObject({ ply: 3, path: [0] });
    expect(Object.keys(s.explored)).toEqual(["2", "3"]);
  });

  it("branches from the final position, where there is no game move to follow", () => {
    expect(play({ ply: 5, path: [], explored: {} }, plies, mv("b8d7"))).toMatchObject({ ply: 5, path: [0] });
  });
});

describe("→ (nextMoves)", () => {
  it("on a game position follows the game, also where lines were explored from it", () => {
    // The user's report: back at the divergence point, → continues the game review.
    expect(nextMoves({ ...explored, path: [] })).toBeNull();
  });

  it("inside a line, goes on along it and stops at its end", () => {
    expect(nextMoves({ ...explored, path: [0] })?.map((n) => n.move.uci)).toEqual(["d7d5"]);
    expect(nextMoves(explored)).toEqual([]);
  });

  it("offers a choice where variations were played, the line's own move first", () => {
    const s = play({ ...explored, path: [0] }, plies, mv("e7e5"));
    const options = nextMoves({ ...s, path: [0] });
    expect(options?.map((n) => n.move.uci)).toEqual(["d7d5", "e7e5"]);
  });
});

describe("← (back)", () => {
  it("steps back inside a line", () => {
    expect(back(explored)?.path).toEqual([0, 0]);
  });

  it("from a line's first move lands on its branch, a game position", () => {
    const branch = back({ ...explored, path: [0] })!;
    expect(branch).toEqual({ ply: 2, path: [], explored: explored.explored });
    expect(inSideline(branch)).toBe(false);
    expect(sidelineOf(branch)).toBeNull();
  });

  it("returns null on a game position, where the game steps back", () => {
    expect(back(start)).toBeNull();
  });
});

describe("listedBranch", () => {
  it("lists the lines while the board is in one of them", () => {
    expect(listedBranch(explored)).toBe(2);
  });

  it("lists them on the game move they replace, and hides them elsewhere", () => {
    expect(listedBranch({ ...explored, ply: 2, path: [] })).toBeNull();
    expect(listedBranch({ ...explored, ply: 3, path: [] })).toBe(2);
    expect(listedBranch({ ...explored, ply: 4, path: [] })).toBeNull();
  });
});

describe("move labels", () => {
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

  it("numbers Black's move that opens a variation, as in 2. e4 d5 (2... e5)", () => {
    const branch = "x w KQkq - 0 2";
    expect(moveLabel(branch, 1, false)).toEqual({ number: null, mover: "black" });
    expect(moveLabel(branch, 1, true)).toEqual({ number: "2...", mover: "black" });
    expect(moveLabel(branch, 2, false)).toEqual({ number: "3.", mover: "white" });
  });
});

describe("★ Best (back, then play the best move)", () => {
  it("adds the best move beside the explored one, as Chess.com's 2... e5 (2... d5)", () => {
    const shown = playAll(start, ["e2e4", "e7e5"]);
    const best = play(back(shown)!, plies, mv("d7d5"));
    expect(ucis(best)).toEqual(["e2e4", "d7d5"]);
    expect(nextMoves({ ...best, path: [0] })?.map((n) => n.move.uci)).toEqual(["e7e5", "d7d5"]);
  });

  it("follows the game when the best move from the branch is the game's own", () => {
    const shown = play(start, plies, mv("e2e4"));
    expect(play(back(shown)!, plies, mv("g1f3"))).toMatchObject({ ply: 3, path: [] });
  });
});
