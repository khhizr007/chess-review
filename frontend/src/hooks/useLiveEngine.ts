import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EngineController, browserWorker, type Analysis, type EngineStatus } from "../lib/engine/controller";

/** Depth the live engine stops at: the review's Standard preset, lower on slow devices (§5.6). */
export const LIVE_DEPTH = 18;
export const SLOW_DEPTH = 16;
/** Further depths offered by "Go deeper"; never an unlimited search. */
export const DEEPER_DEPTHS = [20, 22];
/** Depth at which a sideline move gets its provisional classification (§5.5). */
export const JUDGE_DEPTH = 14;
/** Below this speed (about half the browser speed measured in Phase 0) the device counts as slow. */
const SLOW_NPS = 150_000;
/** The speed is judged once a search reaches this depth. */
const SPEED_DEPTH = 14;
/** Rapid stepping searches only the position the reader stops on. */
const DEBOUNCE_MS = 120;

// Judged once per page: a slow device stays slow across reviews.
let slowDevice: boolean | null = null;

export interface LiveEngine {
  /** "idle" until the first sideline position asks for the engine. */
  status: EngineStatus | "idle";
  error: string | null;
  /** The latest result for the position shown, if any yet. */
  analysis: Analysis | null;
  /** The latest result for the position before the shown move (to judge the move), if any yet. */
  prior: Analysis | null;
  /** Depth the search stops at. */
  target: number;
  /** The shown position, and the one before, have been searched to `target`. */
  finished: boolean;
  priorFinished: boolean;
  /** Searches deeper; null while the search runs or at the deepest depth. */
  deeper: (() => void) | null;
}

/**
 * Live analysis in the browser of `fen` (a sideline position) and of `prior` (the position before
 * its move, so the move can be judged on the same engine's lines). Null stops the engine. The
 * shown position is searched first: both to `JUDGE_DEPTH`, then both to the depth limit. The
 * worker starts on first use, so a reader who never explores downloads nothing. `game` identifies
 * the game: a new one clears the engine's memory.
 */
export function useLiveEngine(fen: string | null, prior: string | null, game: object, chess960: boolean): LiveEngine {
  const controller = useRef<EngineController | null>(null);
  const [status, setStatus] = useState<LiveEngine["status"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [results, setResults] = useState<Record<string, Analysis>>({});
  // "fen|depth" of each plan step searched to its end.
  const [finishedSteps, setFinishedSteps] = useState<ReadonlySet<string>>(new Set());
  const [base, setBase] = useState(() => (slowDevice ? SLOW_DEPTH : LIVE_DEPTH));
  const [extra, setExtra] = useState<{ fen: string; step: number } | null>(null);
  const [visible, setVisible] = useState(() => document.visibilityState !== "hidden");

  const step = extra && extra.fen === fen ? extra.step : 0;
  const target = step > 0 ? DEEPER_DEPTHS[step - 1]! : base;

  const engine = useCallback(() => {
    if (!controller.current) {
      controller.current = new EngineController(browserWorker, {}, (s, message) => {
        setStatus(s);
        setError(message ?? null);
      });
      controller.current.setGame(chess960);
    }
    return controller.current;
  }, [chess960]);

  useEffect(() => {
    controller.current?.setGame(chess960);
    setResults({});
    setFinishedSteps(new Set());
  }, [game, chess960]);

  useEffect(() => {
    const onVisibility = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  useEffect(() => () => controller.current?.dispose(), []);

  // The searches, in order: shown and prior positions to the judging depth, then to the limit.
  const plan = useMemo(() => {
    const early = Math.min(JUDGE_DEPTH, target);
    const steps: [string, number][] = [];
    if (fen && prior) steps.push([fen, early], [prior, early]);
    else if (prior) steps.push([prior, early]);
    if (fen) steps.push([fen, target]);
    if (prior) steps.push([prior, target]);
    return steps;
  }, [fen, prior, target]);

  useEffect(() => {
    if (plan.length === 0 || !visible) {
      controller.current?.stop();
      return;
    }
    let cancelled = false;
    const run = (i: number) => {
      const step = plan[i];
      if (cancelled || !step) return;
      const [at, depth] = step;
      engine().analyze(at, depth, (a) => {
        if (cancelled) return;
        setResults((r) => ({ ...r, [a.fen]: a }));
        if (slowDevice === null && a.depth >= SPEED_DEPTH && a.nps > 0) {
          slowDevice = a.nps < SLOW_NPS;
          if (slowDevice) setBase(SLOW_DEPTH);
        }
        if (a.done && a.fen === at) {
          setFinishedSteps((s) => new Set(s).add(`${at}|${depth}`));
          // The next step starts outside the controller's callback (a cached result answers at once).
          queueMicrotask(() => run(i + 1));
        }
      });
    };
    const timer = setTimeout(() => run(0), DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [plan, visible, engine]);

  const analysis = fen ? (results[fen] ?? null) : null;
  const priorAnalysis = prior ? (results[prior] ?? null) : null;
  const finished = !!fen && finishedSteps.has(`${fen}|${target}`);
  const priorFinished = !!prior && finishedSteps.has(`${prior}|${target}`);
  const deeper =
    fen && finished && step < DEEPER_DEPTHS.length ? () => setExtra({ fen, step: step + 1 }) : null;
  return { status, error, analysis, prior: priorAnalysis, target, finished, priorFinished, deeper };
}
