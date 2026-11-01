import { describe, expect, it } from "vitest";
import type { Circ } from "../api/generated/Circ";
import type { Classification } from "../api/generated/Classification";
import type { CoachEvent } from "../api/generated/CoachEvent";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";
import type { Tag } from "../api/generated/Tag";
import { gameTakeaways } from "./takeaways";

function event(circ: Circ, tag: Tag, actor: Side): CoachEvent {
  return {
    chain: [circ],
    tag,
    piece: null,
    square: null,
    actor,
    geometry: { kind: "squares", squares: [] },
    line: [],
    ply: 0,
    gain_cp: null,
    mate_in: null,
  };
}

/** A move whose sentences say `events` (one sentence each). */
function move(n: number, classification: Classification, events: CoachEvent[], epLost = 0.1): PlyReview {
  return {
    ply: n,
    move_number: Math.ceil(n / 2),
    mover: n % 2 === 1 ? "white" : "black",
    san: `M${n}`,
    classification,
    ep_lost: epLost,
    coach: {
      sentences: events.map((_, i) => ({ fragments: [], event: i as number | null })),
      show_lines: [],
      events,
    },
  } as unknown as PlyReview;
}

const review = (plies: PlyReview[]) => ({ plies }) as unknown as ReviewResult;

describe("gameTakeaways", () => {
  it("counts each kind of lesson per side, most frequent first, linking its moves", () => {
    const t = gameTakeaways(
      review([
        move(2, "mistake", [event("permits", "fork", "white")]),
        move(4, "blunder", [event("permits", "royal_fork", "white"), event("misses_same", "free_piece", "black")]),
        move(6, "miss", [event("misses", "free_piece", "black")]),
        move(8, "mistake", [event("permits", "fork", "white")]),
        move(9, "inaccuracy", [event("base", "hangs_piece", "white")]),
      ]),
    );
    expect(t).toEqual([
      { side: "black", kind: "allowed", text: "Allowed 3 forks", plies: [2, 4, 8] },
      { side: "black", kind: "missed", text: "Missed 2 free pieces", plies: [4, 6] },
      { side: "white", kind: "error", text: "Left a piece hanging", plies: [9] },
    ]);
  });

  it("counts a move once even when two sentences say the same family", () => {
    const t = gameTakeaways(review([move(2, "blunder", [event("permits", "back_rank_mate", "white"), event("permits", "forced_mate", "white")])]));
    expect(t).toEqual([{ side: "black", kind: "allowed", text: "Allowed a back-rank mate", plies: [2] }]);
  });

  it("orders ties by cost and keeps at most four", () => {
    const t = gameTakeaways(
      review([
        move(2, "mistake", [event("permits", "skewer", "white")], 0.1),
        move(4, "blunder", [event("permits", "pin", "white")], 0.4),
        move(6, "mistake", [event("permits", "trapped_piece", "white")], 0.2),
        move(8, "mistake", [event("permits", "fork", "white")], 0.15),
        move(10, "mistake", [event("permits", "double_check", "white")], 0.05),
      ]),
    );
    expect(t.map((x) => x.text)).toEqual(["Allowed a pin", "Allowed a trapped piece", "Allowed a fork", "Allowed a skewer"]);
  });

  it("praises tactics found on good moves, and ignores what is not a lesson", () => {
    const t = gameTakeaways(
      review([
        move(1, "best", [event("base", "fork", "white")]),
        move(3, "brilliant", [event("base", "fork", "white")]),
        // A good move's PERMITS event, and an unknown circumstance, are not counted.
        move(5, "good", [event("permits", "fork", "black")]),
        move(7, "mistake", [event("leads_to" as Circ, "pin", "black")]),
        // An error tag on a bad move without wording, e.g. a tactic tag in BASE.
        move(9, "mistake", [event("base", "skewer", "white")]),
      ]),
    );
    expect(t).toEqual([{ side: "white", kind: "found", text: "Found 2 forks", plies: [1, 3] }]);
  });

  it("does not praise routine moves, but counts what the best move would have done", () => {
    const t = gameTakeaways(
      review([
        move(1, "best", [event("base", "recapture", "white")]),
        move(3, "good", [event("base", "evades_check", "white")]),
        move(5, "brilliant", [event("base", "sacrifice", "white")]),
        move(7, "excellent", [event("base", "offers_piece", "white")]),
        move(2, "mistake", [event("misses", "kicks", "black")]),
        move(4, "mistake", [event("misses", "gains_tempo", "black")]),
        move(6, "inaccuracy", [event("base", "slide", "black")]),
      ]),
    );
    expect(t).toEqual([
      { side: "black", kind: "missed", text: "Missed 2 pawn moves that chase pieces", plies: [2, 4] },
      { side: "white", kind: "found", text: "Found 2 sacrifices", plies: [5, 7] },
      { side: "black", kind: "error", text: "Let the position slip", plies: [6] },
    ]);
  });

  it("tolerates moves without a coach note or with sentences that say no event", () => {
    const bare = { ply: 1, mover: "white", classification: "blunder", ep_lost: 0.3 } as PlyReview;
    const noEvent = move(2, "mistake", []);
    noEvent.coach.sentences.push({ fragments: [], event: null });
    expect(gameTakeaways(review([bare, noEvent]))).toEqual([]);
  });
});
