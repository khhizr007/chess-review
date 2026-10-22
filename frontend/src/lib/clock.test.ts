import { describe, expect, it } from "vitest";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import { clocksAt, formatClock, startingClock } from "./clock";

function review(clocks: (number | null | undefined)[], timeControl: string | null = "600"): ReviewResult {
  const plies = clocks.map((clock_ms, i) => {
    const ply = { ply: i + 1, mover: i % 2 === 0 ? "white" : "black" } as Partial<PlyReview>;
    // An older server leaves the field out altogether.
    if (clock_ms !== undefined) ply.clock_ms = clock_ms;
    return ply;
  });
  const headers = timeControl == null ? [] : [{ name: "TimeControl", value: timeControl }];
  return { game: { headers }, plies } as unknown as ReviewResult;
}

describe("startingClock", () => {
  it("reads the base time of the TimeControl header", () => {
    expect(startingClock("600")).toBe(600_000);
    expect(startingClock("180+2")).toBe(180_000);
    expect(startingClock("1/259200")).toBeNull();
    expect(startingClock("-")).toBeNull();
    expect(startingClock(undefined)).toBeNull();
  });
});

describe("clocksAt", () => {
  const game = review([597_500, 597_900, 590_700]);

  it("shows the starting time before the first move", () => {
    expect(clocksAt(game, 0)).toEqual({ white: 600_000, black: 600_000 });
  });

  it("keeps each side's time from its last move", () => {
    expect(clocksAt(game, 1)).toEqual({ white: 597_500, black: 600_000 });
    expect(clocksAt(game, 2)).toEqual({ white: 597_500, black: 597_900 });
    expect(clocksAt(game, 3)).toEqual({ white: 590_700, black: 597_900 });
  });

  it("is null for games without clocks, and for servers that omit the field", () => {
    expect(clocksAt(review([null, null]), 1)).toBeNull();
    expect(clocksAt(review([undefined, undefined]), 1)).toBeNull();
  });

  it("has no starting time without a TimeControl header", () => {
    expect(clocksAt(review([597_500, 597_900], null), 1)).toEqual({ white: 597_500, black: null });
  });
});

describe("formatClock", () => {
  it("formats like Chess.com, rounding down", () => {
    expect(formatClock(597_500)).toBe("9:57");
    expect(formatClock(600_000)).toBe("10:00");
    expect(formatClock(3_723_000)).toBe("1:02:03");
    expect(formatClock(20_000)).toBe("0:20");
    expect(formatClock(4_250)).toBe("0:04.2");
    expect(formatClock(0)).toBe("0:00.0");
  });
});
