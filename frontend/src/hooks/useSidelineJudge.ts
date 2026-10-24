import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api/client";
import type { JudgeLine } from "../api/generated/JudgeLine";
import type { JudgeThreats } from "../api/generated/JudgeThreats";
import type { SidelineJudgement } from "../api/generated/SidelineJudgement";
import type { PlayedMove } from "../lib/chess";
import type { LiveLine } from "../lib/engine/controller";
import type { Sideline } from "../lib/explore";
import { JUDGE_DEPTH, type LiveEngine, type Threats } from "./useLiveEngine";

export interface Judged {
  judgement: SidelineJudgement;
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

/** Key of an explored move: the review, the branch ply and the moves from there up to it. */
function keyOf(reviewId: string, fromPly: number, moves: readonly PlayedMove[]): string {
  return `${reviewId}|${fromPly}|${moves.map((m) => m.uci).join(" ")}`;
}

/** The judgement of the last of `moves`, explored from game ply `fromPly`, if it is known. */
export type JudgedOf = (fromPly: number, moves: readonly PlayedMove[]) => Judged | undefined;

/**
 * Classifies the shown sideline move with the review's rules on the server, and gets the coach's
 * note for it (docs/explore-mode.md §5.5), from the browser engine's lines of the positions before
 * and after it and its threat searches, once, as soon as both positions reach the shallow
 * `JUDGE_DEPTH`. Returns a lookup of every explored move judged so far, in any line.
 */
export function useSidelineJudge(
  reviewId: string,
  line: Sideline | null,
  live: LiveEngine,
  /** The game is over after the shown move, so its position is not searched. */
  over: boolean,
): JudgedOf {
  const [judged, setJudged] = useState<Record<string, Judged>>({});
  const cursor = line?.cursor ?? 0;
  const key = line && cursor > 0 ? keyOf(reviewId, line.fromPly, line.moves.slice(0, cursor)) : null;

  const before = live.prior;
  const after = over ? null : live.analysis;
  const ready = !!before && before.lines.length > 0 && (over || (!!after && after.lines.length > 0)) && live.threatsReady;
  const depth = ready ? Math.min(before!.depth, over ? Infinity : after!.depth) : 0;
  const due = ready && depth >= JUDGE_DEPTH;

  // The latest lines, read when a stage is reached (they change with every engine update).
  const latest = useRef({ before, after, threats: live.threats, line, judged });
  useEffect(() => {
    latest.current = { before, after, threats: live.threats, line, judged };
  });

  useEffect(() => {
    if (!key || !due) return;
    const { before, after, threats, line, judged } = latest.current;
    if (!before || !line || judged[key]) return;
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
          previous_ep_lost: k > 1 ? judged[keyOf(reviewId, line.fromPly, line.moves.slice(0, k - 1))]?.judgement.ep_lost : undefined,
          threats: judgeThreats(threats),
        },
        controller.signal,
      )
      .then((judgement) => setJudged((s) => ({ ...s, [key]: { judgement } })))
      // A review the server no longer holds, or a cancelled request, leaves the move unmarked.
      .catch(() => {});
    return () => controller.abort();
  }, [key, due, reviewId]);

  return useCallback<JudgedOf>((fromPly, moves) => judged[keyOf(reviewId, fromPly, moves)], [judged, reviewId]);
}
