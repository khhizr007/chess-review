import type { Classification } from "../api/generated/Classification";
import type { Eval } from "../api/generated/Eval";
import type { MissKind } from "../api/generated/MissKind";
import type { Phase } from "../api/generated/Phase";
import type { Side } from "../api/generated/Side";

export interface ClassificationMeta {
  label: string;
  /** CSS custom property holding the colour. */
  color: string;
  /** Completes "<move> is …" in the coach headline. */
  phrase: string;
  /** Marked with an icon and colour in the move list (the rest stay plain). */
  notable: boolean;
}

export const CLASSIFICATIONS: Record<Classification, ClassificationMeta> = {
  brilliant: { label: "Brilliant", color: "var(--c-brilliant)", phrase: "brilliant", notable: true },
  great_find: { label: "Great find", color: "var(--c-great)", phrase: "a great find", notable: true },
  best: { label: "Best", color: "var(--c-best)", phrase: "best", notable: false },
  excellent: { label: "Excellent", color: "var(--c-excellent)", phrase: "excellent", notable: false },
  good: { label: "Good", color: "var(--c-good)", phrase: "good", notable: false },
  book: { label: "Book", color: "var(--c-book)", phrase: "a book move", notable: true },
  inaccuracy: { label: "Inaccuracy", color: "var(--c-inaccuracy)", phrase: "an inaccuracy", notable: true },
  mistake: { label: "Mistake", color: "var(--c-mistake)", phrase: "a mistake", notable: true },
  miss: { label: "Miss", color: "var(--c-miss)", phrase: "a miss", notable: true },
  blunder: { label: "Blunder", color: "var(--c-blunder)", phrase: "a blunder", notable: true },
  forced: { label: "Forced", color: "var(--c-forced)", phrase: "the only move", notable: false },
  tricky: { label: "Tricky", color: "var(--c-tricky)", phrase: "a tricky move", notable: true },
};

/** Report rows in the reference summary's order (Book sits after Great there). */
export const REPORT_ORDER: Classification[] = [
  "brilliant",
  "great_find",
  "book",
  "best",
  "excellent",
  "good",
  "inaccuracy",
  "mistake",
  "miss",
  "blunder",
];

/** Rows competing for the report's three unfolded "good move" slots, in the reference's priority. */
const SHOWN_GOOD: Classification[] = ["brilliant", "great_find", "best", "excellent", "good", "book"];

/**
 * Report rows folded behind the chevron, by the reference's rule (read from its review bundle):
 * Mistake, Miss and Blunder always show and Inaccuracy always folds. Of the good-move rows, the first
 * three that either player earned show, topped up in priority order with empty rows when fewer than
 * three were earned; the rest fold.
 */
export function foldedReportRows(total: (c: Classification) => number): Set<Classification> {
  const earned = SHOWN_GOOD.filter((c) => total(c) > 0);
  const ranked = [...earned, ...SHOWN_GOOD.filter((c) => total(c) <= 0)];
  return new Set<Classification>(["inaccuracy", ...ranked.slice(3)]);
}

export const MISS_KINDS: Record<MissKind, string> = {
  win: "missed a winning continuation",
  faster_mate: "missed a faster mate",
  mate: "missed a forced mate",
  free_piece: "missed a free piece",
  tactic: "missed a tactic that wins material",
  comeback: "missed a way back into the game",
  brilliant: "missed a brilliant sacrifice",
  opportunity: "missed the chance to punish the opponent's error",
};

export const PHASES: Record<Phase, string> = {
  opening: "Opening",
  middlegame: "Middlegame",
  endgame: "Endgame",
};

/** Evaluation text from White's point of view, e.g. "+1.24", "-0.30", "M3", "-M2", "#". */
export function formatEval(ev: Eval): string {
  if (ev.kind === "cp") {
    const pawns = ev.cp / 100;
    if (Math.abs(pawns) < 0.005) return "0.00";
    return `${pawns > 0 ? "+" : "−"}${Math.abs(pawns).toFixed(2)}`;
  }
  if (ev.moves === 0) return ev.winner === "white" ? "1-0" : "0-1";
  return `${ev.winner === "white" ? "" : "−"}M${ev.moves}`;
}

/** Short eval for compact places (bar label). */
export function formatEvalShort(ev: Eval): string {
  if (ev.kind === "cp") {
    const pawns = Math.abs(ev.cp / 100);
    return pawns >= 10 ? pawns.toFixed(0) : pawns.toFixed(1);
  }
  return ev.moves === 0 ? "#" : `M${ev.moves}`;
}

/** Which side the evaluation favours (null for dead equal). */
export function favouredSide(ev: Eval): Side | null {
  if (ev.kind === "mate") return ev.winner;
  if (ev.cp > 0) return "white";
  if (ev.cp < 0) return "black";
  return null;
}

/** Evaluation for compact pills: "+0.38", "−1.65", "M3", "−M2", "1-0". */
export function formatEvalPill(ev: Eval): string {
  if (ev.kind === "cp") {
    const pawns = ev.cp / 100;
    if (Math.abs(pawns) < 0.005) return "0.00";
    return `${pawns > 0 ? "+" : "−"}${Math.abs(pawns).toFixed(Math.abs(pawns) >= 10 ? 1 : 2)}`;
  }
  return formatEval(ev);
}

export function percent(x: number, digits = 0): string {
  return `${(x * 100).toFixed(digits)}%`;
}

export function formatAccuracy(x: number | null | undefined): string {
  return x == null ? "—" : x.toFixed(1);
}

export function sideLabel(side: Side): string {
  return side === "white" ? "White" : "Black";
}

/** "12." for White's move, "12…" for Black's. */
export function moveNumberLabel(moveNumber: number, mover: Side): string {
  return mover === "white" ? `${moveNumber}.` : `${moveNumber}…`;
}
