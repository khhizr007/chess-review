import { useEffect, useRef, useState } from "react";
import { api } from "../api/client";
import type { JudgeLine } from "../api/generated/JudgeLine";
import type { JudgeThreats } from "../api/generated/JudgeThreats";
import type { SidelineJudgement } from "../api/generated/SidelineJudgement";
import type { LiveLine } from "../lib/engine/controller";
import type { Sideline } from "../lib/explore";
import { JUDGE_DEPTH, type LiveEngine, type Threats } from "./useLiveEngine";

export interface Judged {
  judgement: SidelineJudgement;
  /** Judged on lines searched to the depth limit, rather than the provisional depth. */
  final: boolean;
}

function judgeLine(line: LiveLine): JudgeLine {
  return { eval: line.eval, depth: line.depth, pv: line.pv };
}

/** The finished threat searches, as the judge endpoint takes them. */
function judgeThreats(threats: Threats): JudgeThreats {
  const out: JudgeThreats = {};
  for (const kind of ["after", "before", "best"] as const) {
    const t = threats[kind];
    const line = t?.analysis?.done ? t.analysis.lines[0] : undefined;
    if (t && line) out[kind] = { fen: t.fen, line: judgeLine(line) };
  }
  return out;
}

/** Key of sideline move `k` (1-based): the review, the branch ply and the moves up to it. */
function keyOf(reviewId: string, line: Sideline, k: number): string {
  return `${reviewId}|${line.fromPly}|${line.moves
    .slice(0, k)
    .map((m) => m.uci)
    .join(" ")}`;
}

/**
 * Classifies the shown sideline move with the review's rules on the server, and gets the coach's
 * note for it (docs/explore-mode.md §5.5), from the browser engine's lines of the positions before
 * and after it and its threat searches: once both positions reach `JUDGE_DEPTH` (provisional), and
 * again at the depth limit (final). Returns the judgement of each sideline move known so far
 * (index k - 1 for move k).
 */
export function useSidelineJudge(
  reviewId: string,
  line: Sideline | null,
  live: LiveEngine,
  /** The game is over after the shown move, so its position is not searched. */
  over: boolean,
): (Judged | undefined)[] {
  const [judged, setJudged] = useState<Record<string, Judged>>({});
  const cursor = line?.cursor ?? 0;
  const key = line && cursor > 0 ? keyOf(reviewId, line, cursor) : null;

  const before = live.prior;
  const after = over ? null : live.analysis;
  const ready = !!before && before.lines.length > 0 && (over || (!!after && after.lines.length > 0)) && live.threatsReady;
  const depth = ready ? Math.min(before!.depth, over ? Infinity : after!.depth) : 0;
  const final = ready && live.priorFinished && (over || live.finished);
  const stage = !ready || depth < JUDGE_DEPTH ? null : final ? "final" : "provisional";

  // The latest lines, read when a stage is reached (they change with every engine update).
  const latest = useRef({ before, after, threats: live.threats, line, judged });
  useEffect(() => {
    latest.current = { before, after, threats: live.threats, line, judged };
  });

  useEffect(() => {
    if (!key || !stage) return;
    const { before, after, threats, line, judged } = latest.current;
    const known = judged[key];
    if (!before || !line || known?.final || (known && stage === "provisional")) return;
    const k = line.cursor;
    const controller = new AbortController();
    api
      .judgeMove(
        reviewId,
        {
          from_ply: line.fromPly,
          moves: line.moves.slice(0, k).map((m) => m.uci),
          before: before.lines.map(judgeLine),
          after: after?.lines[0] ? judgeLine(after.lines[0]) : undefined,
          previous_ep_lost: k > 1 ? judged[keyOf(reviewId, line, k - 1)]?.judgement.ep_lost : undefined,
          threats: judgeThreats(threats),
        },
        controller.signal,
      )
      .then((judgement) => setJudged((s) => ({ ...s, [key]: { judgement, final: stage === "final" } })))
      // A review the server no longer holds, or a cancelled request, leaves the move unmarked.
      .catch(() => {});
    return () => controller.abort();
  }, [key, stage, reviewId]);

  return line ? line.moves.map((_, i) => judged[keyOf(reviewId, line, i + 1)]) : [];
}
