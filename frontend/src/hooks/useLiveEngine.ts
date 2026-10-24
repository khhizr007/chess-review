import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { lineMoves, nullMoveFen } from "../lib/chess";
import { EngineController, browserWorker, type Analysis, type EngineStatus } from "../lib/engine/controller";

/** Depth the live engine stops at: the review's Standard preset, lower on slow devices (§5.6). */
export const LIVE_DEPTH = 18;
export const SLOW_DEPTH = 16;
/** Further depths offered by "Go deeper"; never an unlimited search. */
export const DEEPER_DEPTHS = [20, 22];
/**
 * Depth of the searches a sideline move is judged on (§5.5): shallow, so the verdict and the coach's
 * note come at once (about 13 ms a position in the browser, Phase 0). A free move only needs a quick
 * opinion; the game's own moves keep the review's full-depth judgement. The coach's threat searches
 * use this depth too when the review's is deeper.
 */
export const JUDGE_DEPTH = 8;
/** Below this speed (about half the browser speed measured in Phase 0) the device counts as slow. */
const SLOW_NPS = 150_000;
/** The speed is judged once a search reaches this depth. */
const SPEED_DEPTH = 14;
/** Rapid stepping searches only the position the reader stops on. */
const DEBOUNCE_MS = 120;
/** Lines of the live searches; the coach's threat searches use one, as on the server. */
const LINES = 3;

// Judged once per page: a slow device stays slow across reviews.
let slowDevice: boolean | null = null;

/** The coach's null-move threat searches around the shown move (as the review makes them). */
export interface CoachSearches {
  /** The shown sideline move (UCI): no best-move threat search when it is the best move. */
  playedUci: string;
  /** The review's threat-search depth (0: no threat searches). */
  depth: number;
  /** Search the position after the best move too (the review's \`best_threat_search\`). */
  best: boolean;
}

/** One threat search: the null-move position and the engine's result for it, if any yet. */
export interface ThreatSearch {
  fen: string;
  analysis: Analysis | null;
}

export interface Threats {
  /** After the move, its mover to move again. */
  after?: ThreatSearch;
  /** Before the move, the opponent to move. */
  before?: ThreatSearch;
  /** After the engine's best move from before, its mover to move again. */
  best?: ThreatSearch;
}

export interface LiveEngine {
  /** "idle" until the first sideline position asks for the engine. */
  status: EngineStatus | "idle";
  error: string | null;
  /** The latest result for the position shown, if any yet. */
  analysis: Analysis | null;
  /** The latest result for the position before the shown move (to judge the move), if any yet. */
  prior: Analysis | null;
  /** The coach's threat searches that apply to the shown move. */
  threats: Threats;
  /** Every threat search that applies has reached its depth. */
  threatsReady: boolean;
  /** Depth the search stops at. */
  target: number;
  /** The shown position has been searched to `target`. */
  finished: boolean;
  /** Searches deeper; null while the search runs or at the deepest depth. */
  deeper: (() => void) | null;
}

type Results = Record<string, Analysis>;

const keyOf = (fen: string, multipv: number) => `${multipv} ${fen}`;

/**
 * The threat-search positions for the shown move: after it (unless the game is over there), before
 * it, and after the best move from before when that is not the played move. The best move is read
 * from the latest result for `prior`, so it follows the search.
 */
function threatPositions(
  fen: string | null,
  prior: string | null,
  results: Results,
  coach: CoachSearches | null,
  chess960: boolean,
): { after?: string; before?: string; best?: string } {
  if (!coach || coach.depth <= 0 || !prior) return {};
  const out: { after?: string; before?: string; best?: string } = {};
  const after = fen ? nullMoveFen(fen) : null;
  if (after) out.after = after;
  const before = nullMoveFen(prior);
  if (before) out.before = before;
  const best = results[keyOf(prior, LINES)]?.lines[0]?.pv[0];
  if (coach.best && best && best !== coach.playedUci) {
    const played = lineMoves(prior, [best], 1, chess960)[0];
    const bestNull = played ? nullMoveFen(played.fen) : null;
    if (bestNull) out.best = bestNull;
  }
  return out;
}

type Step = { kind: "position"; fen: string; depth: number; multipv: number } | { kind: "threats" };

/**
 * Live analysis in the browser of `fen` (a sideline position) and of `prior` (the position before
 * its move, so the move can be judged on the same engine's lines), plus the coach's threat searches
 * around the move. Null stops the engine. Both positions are searched to `JUDGE_DEPTH` and the
 * threat searches made first, so the move is judged at once; then the shown position alone deepens to
 * the depth limit for the eval bar and lines. The worker starts on first use, so a reader who never
 * explores downloads nothing. `game` identifies the game: a new one clears the engine's memory.
 */
export function useLiveEngine(
  fen: string | null,
  prior: string | null,
  game: object,
  chess960: boolean,
  coach: CoachSearches | null = null,
): LiveEngine {
  const controller = useRef<EngineController | null>(null);
  const [status, setStatus] = useState<LiveEngine["status"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<Results>({});
  // The same results, readable inside a running plan (the best-move threat needs the latest).
  const latest = useRef<Results>({});
  const [base, setBase] = useState(() => (slowDevice ? SLOW_DEPTH : LIVE_DEPTH));
  const [extra, setExtra] = useState<{ fen: string; step: number } | null>(null);
  const [visible, setVisible] = useState(() => document.visibilityState !== "hidden");

  const step = extra && extra.fen === fen ? extra.step : 0;
  const target = step > 0 ? DEEPER_DEPTHS[step - 1]! : base;
  const playedUci = coach?.playedUci ?? "";
  // The review's threat-search depth, capped at the shallow judging depth.
  const threatDepth = Math.min(coach?.depth ?? 0, JUDGE_DEPTH);
  const bestThreat = coach?.best ?? false;

  const engine = useCallback(() => {
    if (!controller.current) {
      controller.current = new EngineController(browserWorker, { multipv: LINES }, (s, message) => {
        setStatus(s);
        setError(message ?? null);
      });
      controller.current.setGame(chess960);
    }
    return controller.current;
  }, [chess960]);

  useEffect(() => {
    controller.current?.setGame(chess960);
    latest.current = {};
    setResults({});
  }, [game, chess960]);

  useEffect(() => {
    const onVisibility = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  useEffect(() => () => controller.current?.dispose(), []);

  // The searches, in order: both positions to the shallow judging depth and the threat searches
  // (enough to judge the move), then the shown position alone to the limit (the eval bar and lines).
  const plan = useMemo(() => {
    const pos = (at: string, depth: number): Step => ({ kind: "position", fen: at, depth, multipv: LINES });
    const steps: Step[] = [];
    if (fen) steps.push(pos(fen, JUDGE_DEPTH));
    if (prior) steps.push(pos(prior, JUDGE_DEPTH));
    if (prior && threatDepth > 0) steps.push({ kind: "threats" });
    if (fen) steps.push(pos(fen, target));
    return steps;
  }, [fen, prior, target, threatDepth]);

  useEffect(() => {
    if (plan.length === 0 || !visible) {
      controller.current?.stop();
      return;
    }
    let cancelled = false;
    const queue = [...plan];
    const next = () => {
      const step = queue.shift();
      if (cancelled || !step) return;
      if (step.kind === "threats") {
        const at = threatPositions(fen, prior, latest.current, { playedUci, depth: threatDepth, best: bestThreat }, chess960);
        const fens = [...new Set([at.after, at.before, at.best].filter((f): f is string => !!f))];
        queue.unshift(...fens.map((f): Step => ({ kind: "position", fen: f, depth: threatDepth, multipv: 1 })));
        return next();
      }
      engine().analyze(
        step.fen,
        step.depth,
        (a) => {
          if (cancelled) return;
          latest.current = { ...latest.current, [keyOf(a.fen, a.multipv)]: a };
          setResults(latest.current);
          if (slowDevice === null && a.depth >= SPEED_DEPTH && a.nps > 0) {
            slowDevice = a.nps < SLOW_NPS;
            if (slowDevice) setBase(SLOW_DEPTH);
          }
          // The next step starts outside the controller's callback (a cached result answers at once).
          if (a.done && a.fen === step.fen) queueMicrotask(next);
        },
        step.multipv,
      );
    };
    const timer = setTimeout(next, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [plan, visible, engine, fen, prior, playedUci, threatDepth, bestThreat, chess960]);

  const atDepth = (at: string | null, multipv: number, depth: number) => {
    const a = at ? results[keyOf(at, multipv)] : undefined;
    return !!a && a.depth >= depth && (a.done || a.depth > depth);
  };
  const analysis = fen ? (results[keyOf(fen, LINES)] ?? null) : null;
  const priorAnalysis = prior ? (results[keyOf(prior, LINES)] ?? null) : null;
  const finished = atDepth(fen, LINES, target);

  const positions = threatPositions(fen, prior, results, coach && { ...coach, depth: threatDepth }, chess960);
  const threats: Threats = {};
  for (const kind of ["after", "before", "best"] as const) {
    const at = positions[kind];
    if (at) threats[kind] = { fen: at, analysis: results[keyOf(at, 1)] ?? null };
  }
  const threatsReady = Object.values(positions).every((at) => atDepth(at, 1, threatDepth));

  const deeper =
    fen && finished && step < DEEPER_DEPTHS.length ? () => setExtra({ fen, step: step + 1 }) : null;
  return { status, error, analysis, prior: priorAnalysis, threats, threatsReady, target, finished, deeper };
}
