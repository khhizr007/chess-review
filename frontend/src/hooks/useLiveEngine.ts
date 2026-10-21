import { useCallback, useEffect, useRef, useState } from "react";
import { EngineController, browserWorker, type Analysis, type EngineStatus } from "../lib/engine/controller";

/** Depth the live engine stops at: the review's Standard preset, lower on slow devices (§5.6). */
export const LIVE_DEPTH = 18;
export const SLOW_DEPTH = 16;
/** Further depths offered by "Go deeper"; never an unlimited search. */
export const DEEPER_DEPTHS = [20, 22];
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
  /** The latest result for the position asked for, if any yet. */
  analysis: Analysis | null;
  /** Depth the search stops at. */
  target: number;
  /** Searches the position deeper; null when the search is still running or at its deepest. */
  deeper: (() => void) | null;
}

/**
 * Live analysis of `fen` (a sideline position) in the browser; null stops the engine. The worker
 * starts on first use, so a reader who never explores downloads nothing. `game` identifies the game:
 * a new one clears the engine's memory.
 */
export function useLiveEngine(fen: string | null, game: object, chess960: boolean): LiveEngine {
  const controller = useRef<EngineController | null>(null);
  const [status, setStatus] = useState<LiveEngine["status"]>("idle");
  const [error, setError] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<Analysis | null>(null);
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
  }, [game, chess960]);

  useEffect(() => {
    const onVisibility = () => setVisible(document.visibilityState !== "hidden");
    document.addEventListener("visibilitychange", onVisibility);
    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  useEffect(() => () => controller.current?.dispose(), []);

  useEffect(() => {
    if (!fen || !visible) {
      controller.current?.stop();
      return;
    }
    const timer = setTimeout(() => {
      engine().analyze(fen, target, (a) => {
        setAnalysis(a);
        if (slowDevice === null && a.depth >= SPEED_DEPTH && a.nps > 0) {
          slowDevice = a.nps < SLOW_NPS;
          if (slowDevice) setBase(SLOW_DEPTH);
        }
      });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [fen, target, visible, engine]);

  const current = analysis && analysis.fen === fen ? analysis : null;
  const deeper =
    fen && current?.done && step < DEEPER_DEPTHS.length ? () => setExtra({ fen, step: step + 1 }) : null;
  return { status, error, analysis: current, target, deeper };
}
