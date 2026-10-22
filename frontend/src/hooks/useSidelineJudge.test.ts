import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SidelineJudgement } from "../api/generated/SidelineJudgement";
import type { Analysis } from "../lib/engine/controller";
import type { Sideline } from "../lib/explore";
import { JUDGE_DEPTH, type LiveEngine, type Threats } from "./useLiveEngine";
import { useSidelineJudge } from "./useSidelineJudge";

const judgeMove = vi.fn();
vi.mock("../api/client", () => ({ api: { judgeMove: (...args: unknown[]) => judgeMove(...args) } }));

afterEach(() => judgeMove.mockReset());

const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
const mv = (uci: string) => ({ san: uci, uci, fen: `fen-${uci}`, from: "a1", to: "a2", flags: NONE });
const analysis = (fen: string, depth: number, multipv = 3, done = false): Analysis => ({
  fen,
  multipv,
  depth,
  nps: 1,
  done,
  lines: [{ rank: 1, eval: { kind: "cp", cp: 20 }, depth, moves: [], pv: ["e2e4", "e7e5"] }],
});
const THREATS: Threats = {
  after: { fen: "after-null", analysis: analysis("after-null", JUDGE_DEPTH, 1, true) },
  before: { fen: "before-null", analysis: analysis("before-null", JUDGE_DEPTH, 1, true) },
};
const engine = (prior: number, shown: number, threatsReady = true): LiveEngine => ({
  status: "ready",
  error: null,
  analysis: analysis("after", shown),
  prior: analysis("before", prior),
  threats: THREATS,
  threatsReady,
  target: 18,
  finished: false,
  deeper: null,
});
const judgement = (classification: SidelineJudgement["classification"], ep_lost: number): SidelineJudgement => ({
  classification,
  miss_kind: null,
  rule: "precision",
  ep_lost,
  ep_before: 0.5,
  ep_after: 0.5,
  played_is_best: false,
  show_best_move: false,
  best_move: null,
  explanation: [],
  coach: { sentences: [], show_lines: [], events: [] },
});
const LINE = { eval: { kind: "cp", cp: 20 }, pv: ["e2e4", "e7e5"] };

describe("useSidelineJudge", () => {
  const line: Sideline = { fromPly: 4, moves: [mv("c7c5"), mv("g1f3")], cursor: 1 };

  it("judges once, as soon as both positions reach the shallow judging depth", async () => {
    judgeMove.mockResolvedValueOnce(judgement("good", 0.03));
    const { result, rerender } = renderHook(({ live }) => useSidelineJudge("r1", line, live, false), {
      initialProps: { live: engine(JUDGE_DEPTH - 1, JUDGE_DEPTH + 2) },
    });
    expect(judgeMove).not.toHaveBeenCalled();

    rerender({ live: engine(JUDGE_DEPTH, JUDGE_DEPTH + 2) });
    await waitFor(() => expect(result.current[0]?.judgement.classification).toBe("good"));
    expect(judgeMove).toHaveBeenCalledWith(
      "r1",
      {
        from_ply: 4,
        moves: ["c7c5"],
        before: [{ ...LINE, depth: JUDGE_DEPTH }],
        after: { ...LINE, depth: JUDGE_DEPTH + 2 },
        previous_ep_lost: undefined,
        // The coach's threat searches, each with the position it searched.
        threats: {
          after: { fen: "after-null", line: { ...LINE, depth: JUDGE_DEPTH } },
          before: { fen: "before-null", line: { ...LINE, depth: JUDGE_DEPTH } },
        },
      },
      expect.any(AbortSignal),
    );

    // The live search deepening afterwards does not ask again.
    rerender({ live: engine(JUDGE_DEPTH + 4, 18) });
    await new Promise((r) => setTimeout(r, 20));
    expect(judgeMove).toHaveBeenCalledTimes(1);
  });

  it("waits for the threat searches the coach needs", async () => {
    judgeMove.mockResolvedValue(judgement("good", 0.03));
    const { rerender } = renderHook(({ live }) => useSidelineJudge("r1", line, live, false), {
      initialProps: { live: engine(JUDGE_DEPTH, JUDGE_DEPTH, false) },
    });
    await new Promise((r) => setTimeout(r, 20));
    expect(judgeMove).not.toHaveBeenCalled();
    rerender({ live: engine(JUDGE_DEPTH, JUDGE_DEPTH, true) });
    await waitFor(() => expect(judgeMove).toHaveBeenCalledTimes(1));
  });

  it("sends the previous sideline move's loss with the next move", async () => {
    judgeMove.mockResolvedValueOnce(judgement("mistake", 0.15)).mockResolvedValueOnce(judgement("best", 0));
    const { result, rerender } = renderHook(({ l }) => useSidelineJudge("r1", l, engine(18, 18), false), {
      initialProps: { l: line },
    });
    await waitFor(() => expect(result.current[0]?.judgement.ep_lost).toBe(0.15));
    rerender({ l: { ...line, cursor: 2 } });
    await waitFor(() => expect(result.current[1]?.judgement.classification).toBe("best"));
    expect(judgeMove.mock.calls[1]![1]).toMatchObject({ moves: ["c7c5", "g1f3"], previous_ep_lost: 0.15 });
  });

  it("judges a move that ends the game without the position after it", async () => {
    judgeMove.mockResolvedValueOnce(judgement("best", 0));
    const live = { ...engine(JUDGE_DEPTH, 0), analysis: null };
    renderHook(() => useSidelineJudge("r1", line, live, true));
    await waitFor(() => expect(judgeMove).toHaveBeenCalledTimes(1));
    expect(judgeMove.mock.calls[0]![1].after).toBeUndefined();
  });
});
