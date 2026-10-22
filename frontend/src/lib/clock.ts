// Player clocks from the PGN's [%clk] tags, as Chess.com shows them in the player strips.
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";

/** Starting time (ms) from a `TimeControl` header such as "600" or "180+2"; null for daily or unknown games. */
export function startingClock(timeControl: string | null | undefined): number | null {
  const m = /^\s*(\d+)(?:\+\d+(?:\.\d+)?)?\s*$/.exec(timeControl ?? "");
  return m ? Number(m[1]) * 1000 : null;
}

/**
 * Each side's clock in the position after `ply` plies: the time left after its last move, or the
 * starting time before it has moved. Null when the game carries no clocks at all.
 */
export function clocksAt(review: ReviewResult, ply: number): Record<Side, number | null> | null {
  // Servers from before clocks were parsed omit the field.
  if (!review.plies.some((p) => p.clock_ms != null)) return null;
  const header = review.game.headers.find((h) => h.name.toLowerCase() === "timecontrol");
  const start = startingClock(header?.value);
  const clocks: Record<Side, number | null> = { white: start, black: start };
  for (const p of review.plies.slice(0, ply)) {
    if (p.clock_ms != null) clocks[p.mover] = p.clock_ms;
  }
  return clocks;
}

/** "9:57", "1:05:00", and tenths under 20 seconds ("0:04.2"), as on Chess.com. Time is rounded down. */
export function formatClock(ms: number): string {
  const tenths = Math.floor(Math.max(0, ms) / 100);
  const total = Math.floor(tenths / 10);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  if (h > 0) return `${h}:${String(m).padStart(2, "0")}:${s}`;
  return total < 20 ? `${m}:${s}.${tenths % 10}` : `${m}:${s}`;
}
