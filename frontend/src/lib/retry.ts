import type { CoachEvent } from "../api/generated/CoachEvent";
import type { Eval } from "../api/generated/Eval";
import type { GeometryView } from "../api/generated/GeometryView";
import type { MoveRef } from "../api/generated/MoveRef";
import type { PlyReview } from "../api/generated/PlyReview";
import type { Side } from "../api/generated/Side";
import { uciSquares } from "./fen";
import { tagWords } from "./tags";

/* ------------------------------------------------------------------ accepting a move */

/** Retry accepts moves this close to the best in expected points, as lichess does. */
export const RETRY_TOLERANCE = 0.04;

/** The backend's expected-points curve (`cp_to_expected_points`), from White's view. */
export function cpToExpectedPoints(cp: number, rating: number): number {
  const slope = 0.004 * ((rating * 0.0685 - 12) / 100);
  return 1 / (1 + Math.exp(-slope * cp));
}

/** Expected points of `side` for an evaluation (White's view). */
export function expectedPointsFor(ev: Eval, side: Side, rating: number): number {
  const white = ev.kind === "mate" ? (ev.winner === "white" ? 1 : 0) : cpToExpectedPoints(ev.cp, rating);
  return side === "white" ? white : 1 - white;
}

export interface RetryAnswer {
  move: MoveRef;
  /** The engine's top move, rather than one close enough to it. */
  best: boolean;
}

/**
 * Judges a retry attempt (a dragged piece, `from` → `to`). Accepts the best move, or an
 * engine line's first move that keeps the mover within RETRY_TOLERANCE of the best.
 * Moves outside the engine lines cannot be judged and are rejected.
 */
export function judgeRetry(ply: PlyReview, rating: number, from: string, to: string): RetryAnswer | null {
  const best = ply.best_move;
  if (!best) return null;
  const uci = `${from}${to}`;
  if (best.uci.startsWith(uci)) return { move: best, best: true };
  const line = ply.lines.find((l) => l.moves[0]?.uci.startsWith(uci));
  const move = line?.moves[0];
  if (!line || !move) return null;
  const bestLine = ply.lines.find((l) => l.moves[0]?.uci === best.uci);
  const bestEp = bestLine ? expectedPointsFor(bestLine.eval, ply.mover, rating) : ply.ep_best;
  const ep = expectedPointsFor(line.eval, ply.mover, rating);
  return bestEp - ep <= RETRY_TOLERANCE ? { move, best: false } : null;
}

/* ------------------------------------------------------------------ hint ladder */

/** One step of the retry hint ladder; after the last one, the move itself is revealed. */
export type HintRung =
  | { kind: "area"; squares: string[]; text: string }
  | { kind: "motif"; text: string };

/** Squares an event is about: what is attacked, pinned or defended, not where the move goes. */
export function geometryTargets(g: GeometryView): string[] {
  switch (g.kind) {
    case "fan":
      return g.targets;
    case "converging":
      return [g.target];
    case "x_ray":
      return [g.front, g.behind];
    case "triangle":
      return [g.defended, g.defender];
    case "relation":
      return [g.to];
    case "squares":
      return g.squares;
    default:
      return [];
  }
}

/**
 * The event the hints are built from: one about the best move (what the mover missed)
 * when there is one, else the event of the coach's main sentence.
 */
export function hintEvent(ply: PlyReview): CoachEvent | null {
  const note = ply.coach;
  if (!note || note.events.length === 0) return null;
  const missed = note.events.find((e) => (e.chain[0] === "misses" || e.chain[0] === "misses_same") && e.actor === ply.mover);
  if (missed) return missed;
  const main = note.sentences[0]?.event;
  return (main != null ? note.events[main] : undefined) ?? note.events[0] ?? null;
}

function motifText(e: CoachEvent): string {
  const w = tagWords(e.tag);
  switch (e.chain[0]) {
    case "misses":
    case "misses_same":
      return `Look for ${w.one}.`;
    case "permits":
      return `Make sure your move doesn't allow ${w.one}.`;
    default:
      return w.hint ?? `Think about ${w.one}.`;
  }
}

/**
 * Hints for finding the best move, gentlest first: the area of the board, then the motif.
 * Neither names the move. Moves without a coach event get the piece to move as their only hint.
 */
export function hintLadder(ply: PlyReview): HintRung[] {
  const e = hintEvent(ply);
  const squares = e ? [...new Set(geometryTargets(e.geometry))] : [];
  if (e && squares.length > 0) {
    const ours = e.actor === ply.mover && e.chain[0] !== "base";
    return [
      {
        kind: "area",
        squares,
        text: ours ? "There is something to win around the highlighted squares." : "Look closely at the highlighted squares.",
      },
      { kind: "motif", text: motifText(e) },
    ];
  }
  const from = ply.best_move ? uciSquares(ply.best_move.uci)?.from : undefined;
  return from ? [{ kind: "area", squares: [from], text: "The highlighted piece is the one to move." }] : [];
}

/** What the hint button does next, after `shown` rungs: give another hint or reveal the move. */
export function hintStep(shown: number, ladder: HintRung[]): { label: string; reveal: boolean } {
  if (shown >= ladder.length) return { label: "Show move", reveal: true };
  return { label: shown === 0 ? "Hint" : "More help", reveal: false };
}
