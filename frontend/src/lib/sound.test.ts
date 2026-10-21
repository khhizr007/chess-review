import { describe, expect, it } from "vitest";
import type { Classification } from "../api/generated/Classification";
import type { MoveFlags } from "../api/generated/MoveFlags";
import type { PlyReview } from "../api/generated/PlyReview";
import type { Side } from "../api/generated/Side";
import { moveSound, navigationSounds, retrySound, sanFlags, sidelineSounds } from "./sound";

const NONE: MoveFlags = {
  check: false,
  checkmate: false,
  stalemate: false,
  capture: false,
  castle: false,
  promotion: false,
  en_passant: false,
};

describe("moveSound", () => {
  it("plays a plain move without flags", () => {
    expect(moveSound(NONE)).toBe("move");
  });

  it("lets check win over everything, mate included", () => {
    expect(moveSound({ ...NONE, check: true, capture: true, promotion: true })).toBe("check");
    expect(moveSound({ ...NONE, check: true, checkmate: true })).toBe("check");
  });

  it("orders promotion, then capture, then castling", () => {
    expect(moveSound({ ...NONE, promotion: true, capture: true })).toBe("promote");
    expect(moveSound({ ...NONE, capture: true, en_passant: true })).toBe("capture");
    expect(moveSound({ ...NONE, castle: true })).toBe("castle");
  });
});

describe("sanFlags", () => {
  it("reads the flags from SAN", () => {
    expect(moveSound(sanFlags("Nxe5"))).toBe("capture");
    expect(moveSound(sanFlags("O-O-O"))).toBe("castle");
    expect(moveSound(sanFlags("exd8=Q"))).toBe("promote");
    expect(moveSound(sanFlags("Qxf7+"))).toBe("check");
    expect(moveSound(sanFlags("Rd8#"))).toBe("check");
    expect(moveSound(sanFlags("e4"))).toBe("move");
  });
});

describe("navigationSounds", () => {
  const ply = (mover: Side, flags: Partial<MoveFlags> = {}, classification: Classification | null = null) =>
    ({ mover, flags: { ...NONE, ...flags }, classification }) as PlyReview;
  // 1. e4 (move) 1... d5 (move) 2. exd5 (capture, brilliant for the test) 2... Qxd5 (capture) 3. O-O (castle)
  const plies = [ply("white"), ply("black"), ply("white", { capture: true }, "brilliant"), ply("black", { capture: true }), ply("white", { castle: true })];

  it("plays the move arrived at when stepping forward or jumping", () => {
    expect(navigationSounds(plies, 1, 2, null)).toEqual(["move"]);
    expect(navigationSounds(plies, 0, 5, null)).toEqual(["castle"]);
    expect(navigationSounds(plies, 5, 2, null)).toEqual(["move"]);
  });

  it("plays the move taken back when stepping back one", () => {
    expect(navigationSounds(plies, 5, 4, null)).toEqual(["castle"]);
    expect(navigationSounds(plies, 4, 3, null)).toEqual(["capture"]);
  });

  it("is silent at the start position and when nothing moved", () => {
    expect(navigationSounds(plies, 4, 0, null)).toEqual([]);
    expect(navigationSounds(plies, 3, 3, null)).toEqual([]);
  });

  it("adds the brilliant sound when arriving forward on the player's brilliant move", () => {
    expect(navigationSounds(plies, 2, 3, null)).toEqual(["capture", "brilliant"]);
    expect(navigationSounds(plies, 0, 3, "white")).toEqual(["capture", "brilliant"]);
    expect(navigationSounds(plies, 2, 3, "black")).toEqual(["capture"]);
    // Stepping back onto it.
    expect(navigationSounds(plies, 4, 3, null)).toEqual(["capture"]);
  });
});

describe("retrySound", () => {
  it("grades the retry move", () => {
    expect(retrySound("best")).toBe("correct");
    expect(retrySound("great_find")).toBe("correct");
    expect(retrySound("excellent")).toBe("result_good");
    expect(retrySound("good")).toBe("result_ok");
    expect(retrySound("mistake")).toBe("result_bad");
  });
});

describe("sidelineSounds", () => {
  const moves = [{ flags: NONE }, { flags: { ...NONE, capture: true } }, { flags: { ...NONE, check: true } }];
  it("plays the move arrived at, or the move taken back on one step back", () => {
    expect(sidelineSounds(moves, 0, 1)).toEqual(["move"]);
    expect(sidelineSounds(moves, 1, 2)).toEqual(["capture"]);
    expect(sidelineSounds(moves, 3, 2)).toEqual(["check"]);
    expect(sidelineSounds(moves, 1, 0)).toEqual(["move"]);
    expect(sidelineSounds(moves, 3, 1)).toEqual(["move"]);
    expect(sidelineSounds(moves, 2, 2)).toEqual([]);
  });
});
