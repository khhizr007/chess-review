// Display-only material count from a FEN (no chess rules).
import type { Side } from "../api/generated/Side";

export type PieceKind = "p" | "n" | "b" | "r" | "q";

const START: Record<PieceKind, number> = { p: 8, n: 2, b: 2, r: 2, q: 1 };
const VALUE: Record<PieceKind, number> = { p: 1, n: 3, b: 3, r: 5, q: 9 };
/** Order captured pieces are listed in (cheapest first, like the reference). */
const ORDER: PieceKind[] = ["p", "n", "b", "r", "q"];

export interface MaterialSide {
  /** Opponent pieces this side has captured, cheapest first. */
  captured: PieceKind[];
  /** Material lead in pawns (0 when not ahead). */
  lead: number;
}

function count(placement: string): Record<Side, Record<PieceKind, number>> {
  const out = {
    white: { p: 0, n: 0, b: 0, r: 0, q: 0 },
    black: { p: 0, n: 0, b: 0, r: 0, q: 0 },
  };
  for (const ch of placement) {
    const kind = ch.toLowerCase() as PieceKind;
    if (!(kind in START)) continue;
    out[ch === kind ? "black" : "white"][kind] += 1;
  }
  return out;
}

/** Captured pieces and material lead for both sides of a position. */
export function materialFromFen(fen: string): Record<Side, MaterialSide> {
  const counts = count(fen.split(" ")[0] ?? "");
  const score = (side: Side) => ORDER.reduce((sum, k) => sum + counts[side][k] * VALUE[k], 0);
  const diff = score("white") - score("black");
  const captured = (by: Side): PieceKind[] => {
    const victim = counts[by === "white" ? "black" : "white"];
    // Promotions can push a count above its start value; never show negatives.
    return ORDER.flatMap((k) => Array<PieceKind>(Math.max(0, START[k] - victim[k])).fill(k));
  };
  return {
    white: { captured: captured("white"), lead: Math.max(0, diff) },
    black: { captured: captured("black"), lead: Math.max(0, -diff) },
  };
}
