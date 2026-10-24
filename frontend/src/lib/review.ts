import type { Classification } from "../api/generated/Classification";
import type { Phase } from "../api/generated/Phase";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";

/** Board position shown at a ply (0 = start position, n = after the n-th move). */
export function fenAt(review: ReviewResult, ply: number): string {
  if (ply <= 0) return review.game.start_fen;
  return review.plies[Math.min(ply, review.plies.length) - 1]?.fen_after ?? review.game.start_fen;
}

export function plyAt(review: ReviewResult, ply: number): PlyReview | undefined {
  return ply > 0 ? review.plies[ply - 1] : undefined;
}

/** Analysis of the position currently shown (lines for the next move), if any. */
export function nextPlyAt(review: ReviewResult, ply: number): PlyReview | undefined {
  return review.plies[ply];
}

export type MoveFilter = "all" | "attention" | "key" | "best" | Phase;

export const FILTERS: { id: MoveFilter; label: string }[] = [
  { id: "all", label: "Every move" },
  { id: "attention", label: "Mistakes only" },
  { id: "key", label: "Key moments" },
  { id: "best", label: "Best & missed" },
  { id: "opening", label: "Opening" },
  { id: "middlegame", label: "Middlegame" },
  { id: "endgame", label: "Endgame" },
];

/** Classes the move list always marks with their icon. */
const LIST_ICONS: ReadonlySet<Classification> = new Set(["brilliant", "great_find", "mistake", "blunder"]);

/** The game's last book move (its ply), where the move list marks the end of the opening book. */
export function lastBookPly(plies: PlyReview[]): number | null {
  return plies.findLast((p) => p.classification === "book")?.ply ?? null;
}

/**
 * Whether the move list marks a move with its icon, as Chess.com does (docs/polish-gaps.md #14): the
 * brilliant, great, mistake and blunder classes, best moves at key moments, and the last book move.
 * Inaccuracies, misses and other book moves stay plain.
 */
export function showsListIcon(p: PlyReview, lastBook: number | null): boolean {
  if (!p.classification) return false;
  if (p.classification === "book") return p.ply === lastBook;
  return LIST_ICONS.has(p.classification) || (p.classification === "best" && p.is_key_moment);
}

const GOOD_ENOUGH: ReadonlySet<Classification> = new Set([
  "brilliant",
  "great_find",
  "best",
  "book",
  "excellent",
  "good",
  "forced",
]);

/** Reference client's predicate: anything not "good enough" needs attention. */
export function needsAttention(p: PlyReview): boolean {
  return p.classification !== null && !GOOD_ENOUGH.has(p.classification);
}

export function matchesFilter(p: PlyReview, filter: MoveFilter): boolean {
  switch (filter) {
    case "all":
      return true;
    case "attention":
      return needsAttention(p);
    case "key":
      return p.is_key_moment || p.is_critical;
    case "best":
      return (
        p.classification === "brilliant" ||
        p.classification === "great_find" ||
        p.classification === "best" ||
        p.classification === "miss"
      );
    default:
      return p.phase === filter;
  }
}

/** Step to the previous/next ply that matches a filter (wraps to null at the ends). */
export function stepFiltered(review: ReviewResult, from: number, dir: 1 | -1, filter: MoveFilter): number | null {
  for (let ply = from + dir; ply >= 1 && ply <= review.plies.length; ply += dir) {
    const p = review.plies[ply - 1];
    if (p && matchesFilter(p, filter)) return ply;
  }
  return null;
}

/** First ply (1-based) where `side` played a move of this classification, or null. */
export function firstPlyOf(review: ReviewResult, side: Side, classification: Classification): number | null {
  const p = review.plies.find((m) => m.mover === side && m.classification === classification);
  return p ? p.ply : null;
}
