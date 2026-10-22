import { describe, expect, it } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import { coachMessage, coachMood, moveArrows, reportSummary } from "./coach";
import { materialFromFen } from "./material";

function ply(overrides: Partial<PlyReview> = {}): PlyReview {
  return {
    ply: 23,
    move_number: 12,
    mover: "white",
    san: "O-O",
    flags: { check: false, checkmate: false, stalemate: false, capture: false, castle: true, promotion: false, en_passant: false },
    ep_before: 0.51,
    ep_after: 0.41,
    ep_lost: 0.1,
    classification: "miss",
    miss_kind: "brilliant",
    best_move: { san: "Bxb7", uci: "e4b7", fen: "x" },
    played_is_best: false,
    explanation: ["The engine's top choice was Bxb7.", "Expected score for White: 51% before the move, 41% after."],
    coach: { sentences: [], show_lines: [], events: [] },
    ...overrides,
  } as PlyReview;
}

const review = {
  white: { name: "khhizr7", accuracy: { overall: 77.4 } },
  black: { name: "Sheep", accuracy: { overall: 83.6 } },
  game: { result: "0-1", opening: { eco: "C55", name: "Two Knights Defense" } },
  plies: [] as PlyReview[],
  biggest_swings: [] as number[],
} as unknown as ReviewResult;

describe("coachMessage", () => {
  it("names the miss, the player and the best move, without repeating restated facts", () => {
    const m = coachMessage(review, ply(), undefined);
    expect(m.verdict).toBe("is a miss");
    expect(m.advice).toEqual(["khhizr7 missed a brilliant sacrifice.", "Bxb7 was the move to play."]);
    expect(m.facts).toEqual([]);
    expect(m.swing).toEqual({ before: 0.51, after: 0.41 });
  });

  it("calls out the last book move", () => {
    const book = ply({ classification: "book", explanation: ["Book move: Italian Game (C50)."], ep_after: 0.51 });
    expect(coachMessage(review, book, ply({ classification: "best" })).verdict).toBe("is the last book move");
    expect(coachMessage(review, book, ply({ classification: "book" })).verdict).toBe("is a book move");
    expect(coachMessage(review, book, undefined).advice[0]).toBe("Opening theory: Italian Game (C50).");
  });

  it("describes a mistake by what it did to the position", () => {
    const losing = ply({ classification: "mistake", ep_before: 0.2, ep_after: 0.17, best_move: { san: "f5", uci: "f4f5", fen: "x" } });
    expect(coachMessage(review, losing, undefined).advice[0]).toBe("This makes a difficult position worse.");
    const winning = ply({ classification: "blunder", ep_before: 0.8, ep_after: 0.2 });
    expect(coachMessage(review, winning, undefined).advice[0]).toBe("A serious error. This throws away a winning position.");
  });

  it("moves engine facts into the advice for mistakes but keeps them in details otherwise", () => {
    const fact = "In the engine line after this move, White loses about 9.0 pawns of material.";
    expect(coachMessage(review, ply({ classification: "blunder", explanation: [fact] }), undefined).advice).toContain(fact);
    const good = coachMessage(review, ply({ classification: "good", explanation: [fact] }), undefined);
    expect(good.advice).not.toContain(fact);
    expect(good.facts).toEqual([fact]);
  });

  it("says the backend coach's sentences instead of the generic advice when it explains the move", () => {
    const fact = "In the engine line after this move, Black loses about 5.0 pawns of material.";
    const fork = {
      text: "Nc7+",
      arrows: [
        { from: "b5", to: "c7", kind: "move" as const },
        { from: "c7", to: "a8", kind: "attack" as const },
      ],
      squares: ["a8"],
    };
    const coach = {
      sentences: [
        {
          fragments: [{ text: "Ke8 allows ", arrows: [], squares: [] }, fork, { text: ", which forks Black's king and rook on a8.", arrows: [], squares: [] }],
          event: 0,
        },
        { fragments: [{ text: "Better was Kc8.", arrows: [], squares: [] }], event: null },
      ],
      show_lines: [],
      events: [],
    };
    const blunder = ply({ classification: "blunder", explanation: [fact], coach, uci: "d8e8", show_best_move: true });
    const m = coachMessage(review, blunder, undefined);
    expect(m.sentences).toHaveLength(2);
    expect(m.advice).toEqual([]);
    expect(m.facts).toEqual([fact]);
    // The board shows the missed best move and one arrow of the main sentence.
    const best = { from: "e4", to: "b7", kind: "best" };
    expect(moveArrows(blunder)).toEqual([best, { from: "b5", to: "c7", kind: "move" }]);
    expect(moveArrows(blunder, false)).toEqual([{ from: "b5", to: "c7", kind: "move" }]);
    // Never an arrow along the move just played, and none on book or best moves.
    expect(moveArrows({ ...blunder, uci: "b5c7" })).toEqual([best, { from: "c7", to: "a8", kind: "attack" }]);
    expect(moveArrows({ ...blunder, classification: "book" })).toEqual([]);
    expect(moveArrows({ ...blunder, classification: "best", played_is_best: true })).toEqual([]);
    expect(moveArrows({ ...blunder, show_best_move: false })).toEqual([{ from: "b5", to: "c7", kind: "move" }]);
    // A good move shows the coach's sentences and keeps its own short advice.
    const good = coachMessage(review, ply({ classification: "good", coach }), undefined);
    expect(good.sentences).toHaveLength(2);
    expect(good.advice).not.toEqual([]);
  });
});

describe("coachMood", () => {
  it("follows the classification", () => {
    expect(coachMood("brilliant")).toBe("impressed");
    expect(coachMood("great_find")).toBe("impressed");
    expect(coachMood("best")).toBe("happy");
    expect(coachMood("mistake")).toBe("concerned");
    expect(coachMood("miss")).toBe("concerned");
    expect(coachMood("blunder")).toBe("sad");
    expect(coachMood("good")).toBe("neutral");
    expect(coachMood(null)).toBe("neutral");
  });
});

describe("reportSummary", () => {
  it("states the result and who played more accurately", () => {
    const s = reportSummary(review);
    expect(s[0]).toBe("Sheep won the game.");
    expect(s[1]).toBe("Sheep played more accurately than khhizr7 (83.6 vs 77.4).");
    expect(s).toContain("Opening: Two Knights Defense.");
  });
});

describe("materialFromFen", () => {
  it("lists captured pieces and the material lead", () => {
    // White is missing a knight and a pawn; Black is missing a knight.
    const m = materialFromFen("r1bqkb1r/ppp2ppp/2n5/4p3/2B1p3/5N2/PPPP1PPP/R1BQK2R w KQkq - 0 7");
    expect(m.black.captured).toEqual(["p", "n"]);
    expect(m.white.captured).toEqual(["n"]);
    expect(m.black.lead).toBe(1);
    expect(m.white.lead).toBe(0);
  });
});
