import { describe, expect, it } from "vitest";
import type { CoachEvent } from "../api/generated/CoachEvent";
import type { Line } from "../api/generated/Line";
import type { PlyReview } from "../api/generated/PlyReview";
import { cpToExpectedPoints, geometryTargets, hintEvent, hintLadder, hintStep, judgeRetry } from "./retry";

const cp = (n: number) => ({ kind: "cp" as const, cp: n });
const line = (uci: string, san: string, white: number): Line => ({ rank: 1, eval: cp(white), depth: 18, moves: [{ uci, san, fen: `fen-${san}` }] });

function ply(overrides: Partial<PlyReview> = {}): PlyReview {
  return {
    ply: 26,
    mover: "black",
    san: "Rxd7",
    uci: "d8d7",
    ep_best: 0.02,
    best_move: { san: "Nxd7", uci: "f6d7", fen: "fen-Nxd7" },
    lines: [line("f6d7", "Nxd7", 540), line("e7d7", "Qxd7", 612), line("e8d7", "Kxd7", 900)],
    coach: { sentences: [], show_lines: [], events: [] },
    ...overrides,
  } as PlyReview;
}

// 6...Nf6 in the sample game: the reply Qb3 forks b7 and f7.
const fork: CoachEvent = {
  chain: ["permits", "wins_material"],
  tag: "fork",
  piece: "queen",
  square: "b3",
  actor: "white",
  geometry: { kind: "fan", source: "b3", targets: ["b7", "f7"] },
  line: ["f3b3"],
  ply: 0,
  gain_cp: 100,
  mate_in: null,
};

// 13...Rxd7: Nxd7 would have taken White's rook for free.
const freeRook: CoachEvent = {
  chain: ["misses_same"],
  tag: "free_piece",
  piece: "rook",
  square: "d7",
  actor: "black",
  geometry: { kind: "relation", from: "f6", to: "d7" },
  line: ["f6d7"],
  ply: 0,
  gain_cp: null,
  mate_in: null,
};

const said = (event: number) => ({ fragments: [{ text: "x", arrows: [], squares: [] }], event });

describe("judgeRetry", () => {
  it("uses the backend's expected-points curve", () => {
    expect(cpToExpectedPoints(0, 2800)).toBeCloseTo(0.5);
    expect(cpToExpectedPoints(116, 2800)).toBeCloseTo(0.697, 3);
  });

  it("accepts the best move, promotions included", () => {
    expect(judgeRetry(ply(), 2800, "f6", "d7")).toEqual({ move: ply().best_move, best: true });
    const promo = ply({ best_move: { san: "e8=Q", uci: "e7e8q", fen: "x" } });
    expect(judgeRetry(promo, 2800, "e7", "e8")?.best).toBe(true);
  });

  it("accepts an engine line within 0.04 expected points of the best", () => {
    // In a lost position Qxd7 costs Black under 1% more.
    expect(judgeRetry(ply(), 2800, "e7", "d7")).toEqual({ move: { uci: "e7d7", san: "Qxd7", fen: "fen-Qxd7" }, best: false });
    // From a level position, a move that drops a pawn is too far off.
    const level = ply({ lines: [line("f6d7", "Nxd7", 0), line("e7d7", "Qxd7", 100)] });
    expect(judgeRetry(level, 2800, "e7", "d7")).toBeNull();
  });

  it("rejects close-call alternatives just outside the tolerance, and moves the engine did not look at", () => {
    // 6...Nf6: Qf6 (+1.16) best, Qd7 (+1.47) costs Black about 0.045.
    const nf6 = ply({
      best_move: { san: "Qf6", uci: "d8f6", fen: "x" },
      lines: [line("d8f6", "Qf6", 116), line("d8d7", "Qd7", 147)],
    });
    expect(judgeRetry(nf6, 2800, "d8", "d7")).toBeNull();
    expect(judgeRetry(nf6, 2800, "a7", "a6")).toBeNull();
  });
});

describe("hint ladder", () => {
  it("points at what the missed move wins, then names the motif", () => {
    const p = ply({ coach: { sentences: [said(0), said(1)], show_lines: [], events: [fork, freeRook] } });
    expect(hintEvent(p)).toBe(freeRook);
    expect(hintLadder(p)).toEqual([
      { kind: "area", squares: ["d7"], text: "There is something to win around the highlighted squares." },
      { kind: "motif", text: "Look for a free piece." },
    ]);
  });

  it("prefers the best move's idea that a sentence says, and names the new motifs", () => {
    // The best move both defends a piece (said) and stops a threat (not said).
    const stops: CoachEvent = { ...freeRook, chain: ["misses", "prevents", "wins_material"], actor: "black" };
    const defends: CoachEvent = {
      ...freeRook,
      chain: ["misses"],
      tag: "defends_piece",
      geometry: { kind: "triangle", defended: "d4", defender: "c3", attacker: "d8" },
    };
    const p = ply({ coach: { sentences: [said(0), said(2)], show_lines: [], events: [fork, stops, defends] } });
    expect(hintEvent(p)).toBe(defends);
    expect(hintLadder(p)[1]).toEqual({ kind: "motif", text: "Look for a way to defend a piece." });
  });

  it("warns about the tactic the move allowed when nothing was missed", () => {
    const p = ply({ mover: "black", coach: { sentences: [said(0)], show_lines: [], events: [fork] } });
    const ladder = hintLadder(p);
    expect(ladder[0]).toEqual({ kind: "area", squares: ["b7", "f7"], text: "Look closely at the highlighted squares." });
    expect(ladder[1]).toEqual({ kind: "motif", text: "Make sure your move doesn't allow a fork." });
    // Neither rung names the move.
    expect(JSON.stringify(ladder)).not.toContain("Nxd7");
  });

  it("uses the error tag's own hint for BASE events", () => {
    const threat: CoachEvent = { ...fork, chain: ["base"], tag: "ignores_threat", actor: "black", geometry: { kind: "relation", from: "d4", to: "e5" } };
    const p = ply({ coach: { sentences: [said(0)], show_lines: [], events: [threat] } });
    expect(hintLadder(p).map((r) => r.text)).toEqual(["Look closely at the highlighted squares.", "Something of yours is under attack."]);
  });

  it("falls back to the piece to move without a coach note", () => {
    const ladder = hintLadder(ply({ coach: undefined as unknown as PlyReview["coach"] }));
    expect(ladder).toEqual([{ kind: "area", squares: ["f6"], text: "The highlighted piece is the one to move." }]);
    expect(hintLadder(ply({ best_move: null }))).toEqual([]);
  });

  it("steps hint, more help, then reveal", () => {
    const ladder = hintLadder(ply({ coach: { sentences: [said(0)], show_lines: [], events: [freeRook] } }));
    expect([0, 1, 2].map((n) => hintStep(n, ladder))).toEqual([
      { label: "Hint", reveal: false },
      { label: "More help", reveal: false },
      { label: "Show move", reveal: true },
    ]);
    expect(hintStep(0, [])).toEqual({ label: "Show move", reveal: true });
  });

  it("reads target squares from every geometry", () => {
    expect(geometryTargets({ kind: "converging", target: "e5", sources: ["d4", "f4"] })).toEqual(["e5"]);
    expect(geometryTargets({ kind: "x_ray", attacker: "b5", front: "d7", behind: "e8" })).toEqual(["d7", "e8"]);
    expect(geometryTargets({ kind: "triangle", defended: "d8", defender: "e7", attacker: "g5" })).toEqual(["d8", "e7"]);
    expect(geometryTargets({ kind: "squares", squares: ["e8"] })).toEqual(["e8"]);
  });
});
