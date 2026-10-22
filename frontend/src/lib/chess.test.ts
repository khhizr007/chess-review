import { describe, expect, it } from "vitest";
import { canMoveFrom, ending, legalMoveCount, legalTargets, lineMoves, needsPromotion, nullMoveFen, playMove } from "./chess";

const START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const CASTLE = "r3k2r/pppqbppp/2np1n2/4p1B1/2B1P1b1/2NP1N2/PPPQ1PPP/R3K2R w KQkq - 6 8";

describe("playMove", () => {
  it("plays a legal move with SAN, UCI and the FEN after it", () => {
    const m = playMove(START, "e2", "e4");
    expect(m).toMatchObject({ san: "e4", uci: "e2e4", from: "e2", to: "e4" });
    expect(m?.fen).toBe("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1");
    expect(m?.flags.capture).toBe(false);
  });

  it("rejects illegal moves and moves of the side not to move", () => {
    expect(playMove(START, "e2", "e5")).toBeNull();
    expect(playMove(START, "e7", "e5")).toBeNull();
    expect(playMove(START, "z9", "e4")).toBeNull();
  });

  it("castles from a king drop on its target square or on its rook, with the backend's UCI", () => {
    for (const to of ["g1", "h1"]) {
      const m = playMove(CASTLE, "e1", to);
      expect(m).toMatchObject({ san: "O-O", uci: "e1g1", from: "e1", to: "g1" });
      expect(m?.flags.castle).toBe(true);
      expect(m?.flags.capture).toBe(false);
    }
    expect(playMove(CASTLE, "e1", "c1")).toMatchObject({ san: "O-O-O", uci: "e1c1", to: "c1" });
  });

  it("writes Chess960 castling as king takes rook", () => {
    expect(playMove(CASTLE, "e1", "h1", undefined, true)?.uci).toBe("e1h1");
  });

  it("flags captures, en passant, checks and mates", () => {
    expect(playMove("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2", "e5", "d6")?.flags).toMatchObject({ capture: true, en_passant: true });
    expect(playMove("4k3/8/8/8/8/8/8/R3K3 w - - 0 1", "a1", "a8")?.flags).toMatchObject({ check: true, checkmate: false });
    const mate = playMove("rnbqkbnr/ppppp2p/5p2/6p1/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 3", "d1", "h5");
    expect(mate).toMatchObject({ san: "Qh5#" });
    expect(mate?.flags.checkmate).toBe(true);
  });

  it("promotes to the chosen piece", () => {
    const fen = "8/4P1k1/8/8/8/8/8/4K3 w - - 0 1";
    expect(needsPromotion(fen, "e7", "e8")).toBe(true);
    expect(needsPromotion(fen, "e1", "e2")).toBe(false);
    expect(playMove(fen, "e7", "e8", "n")).toMatchObject({ san: "e8=N+", uci: "e7e8n" });
    expect(playMove(fen, "e7", "e8", "q")?.flags.promotion).toBe(true);
  });
});

describe("canMoveFrom", () => {
  it("allows only the side to move's pieces with a legal move", () => {
    expect(canMoveFrom(START, "g1")).toBe(true);
    expect(canMoveFrom(START, "a1")).toBe(false);
    expect(canMoveFrom(START, "g8")).toBe(false);
    expect(canMoveFrom(START, "e4")).toBe(false);
  });
});

describe("ending", () => {
  it("names how a position ends the game", () => {
    expect(ending(START)).toBeNull();
    expect(ending("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")).toBe("checkmate");
    expect(ending("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1")).toBe("stalemate");
    expect(ending("8/8/4k3/8/8/2K5/8/8 w - - 0 1")).toBe("insufficient_material");
    expect(ending("8/8/4k3/8/8/2K5/8/R7 w - - 100 80")).toBe("fifty_moves");
  });
});

describe("legalMoveCount", () => {
  it("counts moves as the engine does, a promotion as four", () => {
    expect(legalMoveCount(START)).toBe(20);
    expect(legalMoveCount("8/4P1k1/8/8/8/8/8/4K3 w - - 0 1")).toBe(4 + 5);
    expect(legalMoveCount("rnb1kbnr/pppp1ppp/8/4p3/6Pq/5P2/PPPPP2P/RNBQKBNR w KQkq - 1 3")).toBe(0);
  });
});

describe("lineMoves", () => {
  it("plays an engine line into SAN and FENs, castling included", () => {
    const line = lineMoves(CASTLE, ["e1g1", "e8c8", "g5f6"], 12);
    expect(line.map((m) => m.san)).toEqual(["O-O", "O-O-O", "Bxf6"]);
    expect(line[1]?.fen.split(" ")[0]).toBe("2kr3r/pppqbppp/2np1n2/4p1B1/2B1P1b1/2NP1N2/PPPQ1PPP/R4RK1");
  });

  it("stops at the limit and at an illegal move", () => {
    expect(lineMoves(START, ["e2e4", "e7e5", "g1f3"], 2)).toHaveLength(2);
    expect(lineMoves(START, ["e2e4", "e2e4", "g1f3"], 12)).toHaveLength(1);
  });
});

describe("nullMoveFen", () => {
  it("passes the move to the other side, without an en passant square", () => {
    expect(nullMoveFen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")).toBe(
      "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 1",
    );
  });

  it("refuses a side in check and a side left without moves", () => {
    // Black to move is in check from the rook.
    expect(nullMoveFen("4k3/8/8/8/8/8/8/K3R3 b - - 0 1")).toBeNull();
    // Passing would leave White, with only a boxed-in king and a blocked pawn, without a legal move.
    expect(nullMoveFen("8/8/8/8/8/1k6/p7/K7 b - - 0 1")).toBeNull();
  });
});

describe("legalTargets", () => {
  const sorted = (t: { square: string; capture: boolean }[]) => [...t].sort((a, b) => a.square.localeCompare(b.square));

  it("lists the squares a piece can go to, marking captures", () => {
    expect(sorted(legalTargets(START, "g1"))).toEqual([
      { square: "f3", capture: false },
      { square: "h3", capture: false },
    ]);
    // After 1. e4 d5 the e4 pawn can push or take on d5.
    const fen = "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2";
    expect(sorted(legalTargets(fen, "e4"))).toEqual([
      { square: "d5", capture: true },
      { square: "e5", capture: false },
    ]);
  });

  it("shows a castling king where it lands, or on its rook in Chess960", () => {
    const king = legalTargets(CASTLE, "e1").map((t) => t.square);
    expect(king).toContain("g1");
    expect(king).toContain("c1");
    expect(king).not.toContain("h1");
    expect(legalTargets(CASTLE, "e1", true).map((t) => t.square)).toContain("h1");
  });

  it("has nothing for an empty square or the side not to move", () => {
    expect(legalTargets(START, "e4")).toEqual([]);
    expect(legalTargets(START, "e7")).toEqual([]);
  });
});
