import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SidelineJudgement } from "../api/generated/SidelineJudgement";
import type { Analysis } from "../lib/engine/controller";
import type { Sideline } from "../lib/explore";
import type { LiveEngine } from "./useLiveEngine";
import { useSidelineJudge } from "./useSidelineJudge";

const judgeMove = vi.fn();
vi.mock("../api/client", () => ({ api: { judgeMove: (...args: unknown[]) => judgeMove(...args) } }));

afterEach(() => judgeMove.mockReset());

const NONE = { check: false, checkmate: false, stalemate: false, capture: false, castle: false, promotion: false, en_passant: false };
const mv = (uci: string) => ({ san: uci, uci, fen: `fen-${uci}`, from: "a1", to: "a2", flags: NONE });
const analysis = (fen: string, depth: number): Analysis => ({
  fen,
  depth,
  nps: 1,
  done: false,
  lines: [{ rank: 1, eval: { kind: "cp", cp: 20 }, depth, moves: [], pv: ["e2e4", "e7e5"] }],
});
const engine = (prior: number, shown: number, finished = false): LiveEngine => ({
  status: "ready",
  error: null,
  analysis: analysis("after", shown),
  prior: analysis("before", prior),
  target: 18,
  finished,
  priorFinished: finished,
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
});

describe("useSidelineJudge", () => {
  const line: Sideline = { fromPly: 4, moves: [mv("c7c5"), mv("g1f3")], cursor: 1 };

  it("waits for both positions to reach the judging depth, then judges provisionally and finally", async () => {
    judgeMove.mockResolvedValueOnce(judgement("good", 0.03)).mockResolvedValueOnce(judgement("excellent", 0.01));
    const { result, rerender } = renderHook(({ live }) => useSidelineJudge("r1", line, live, false), {
      initialProps: { live: engine(10, 16) },
    });
    expect(judgeMove).not.toHaveBeenCalled();

    rerender({ live: engine(14, 16) });
    await waitFor(() => expect(result.current[0]?.judgement.classification).toBe("good"));
    expect(result.current[0]?.final).toBe(false);
    expect(judgeMove).toHaveBeenCalledWith(
      "r1",
      {
        from_ply: 4,
        moves: ["c7c5"],
        before: [{ eval: { kind: "cp", cp: 20 }, depth: 14, pv: ["e2e4", "e7e5"] }],
        after: { eval: { kind: "cp", cp: 20 }, depth: 16, pv: ["e2e4", "e7e5"] },
        previous_ep_lost: undefined,
      },
      expect.any(AbortSignal),
    );

    // Deeper provisional updates do not ask again; the final depth does.
    rerender({ live: engine(15, 17) });
    rerender({ live: engine(18, 18, true) });
    await waitFor(() => expect(result.current[0]).toMatchObject({ final: true, judgement: { classification: "excellent" } }));
    expect(judgeMove).toHaveBeenCalledTimes(2);
  });

  it("sends the previous sideline move's loss with the next move", async () => {
    judgeMove.mockResolvedValueOnce(judgement("mistake", 0.15)).mockResolvedValueOnce(judgement("best", 0));
    const { result, rerender } = renderHook(({ l }) => useSidelineJudge("r1", l, engine(18, 18, true), false), {
      initialProps: { l: line },
    });
    await waitFor(() => expect(result.current[0]?.judgement.ep_lost).toBe(0.15));
    rerender({ l: { ...line, cursor: 2 } });
    await waitFor(() => expect(result.current[1]?.judgement.classification).toBe("best"));
    expect(judgeMove.mock.calls[1]![1]).toMatchObject({ moves: ["c7c5", "g1f3"], previous_ep_lost: 0.15 });
  });

  it("judges a move that ends the game without the position after it", async () => {
    judgeMove.mockResolvedValueOnce(judgement("best", 0));
    const live = { ...engine(18, 0, true), analysis: null, finished: false };
    renderHook(() => useSidelineJudge("r1", line, live, true));
    await waitFor(() => expect(judgeMove).toHaveBeenCalledTimes(1));
    expect(judgeMove.mock.calls[0]![1].after).toBeUndefined();
  });
});
