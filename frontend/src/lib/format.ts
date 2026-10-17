import type { Classification } from "../api/generated/Classification";
import type { Eval } from "../api/generated/Eval";
import type { MissKind } from "../api/generated/MissKind";
import type { Phase } from "../api/generated/Phase";
import type { Side } from "../api/generated/Side";

export interface ClassificationMeta {
  label: string;
  /** Short annotation symbol shown in badges. */
  symbol: string;
  /** CSS custom property holding the colour. */
  color: string;
}

export const CLASSIFICATIONS: Record<Classification, ClassificationMeta> = {
  brilliant: { label: "Brilliant", symbol: "!!", color: "var(--c-brilliant)" },
  great_find: { label: "Great find", symbol: "!", color: "var(--c-great)" },
  best: { label: "Best", symbol: "★", color: "var(--c-best)" },
  excellent: { label: "Excellent", symbol: "✓", color: "var(--c-excellent)" },
  good: { label: "Good", symbol: "✓", color: "var(--c-good)" },
  book: { label: "Book", symbol: "≡", color: "var(--c-book)" },
  inaccuracy: { label: "Inaccuracy", symbol: "?!", color: "var(--c-inaccuracy)" },
  mistake: { label: "Mistake", symbol: "?", color: "var(--c-mistake)" },
  miss: { label: "Miss", symbol: "✕", color: "var(--c-miss)" },
  blunder: { label: "Blunder", symbol: "??", color: "var(--c-blunder)" },
  forced: { label: "Forced", symbol: "→", color: "var(--c-forced)" },
  tricky: { label: "Tricky", symbol: "⁈", color: "var(--c-great)" },
};

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
