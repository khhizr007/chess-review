import type { CoachEvent } from "../api/generated/CoachEvent";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Side } from "../api/generated/Side";
import { LEARNABLE } from "./coach";
import { tagFamily, tagWords } from "./tags";

/** What a takeaway counts: tactics the mover let through or missed, own errors, or tactics found. */
export type TakeawayKind = "error" | "allowed" | "missed" | "found";

export interface Takeaway {
  side: Side;
  kind: TakeawayKind;
  /** "Allowed 3 forks". */
  text: string;
  /** The moves it is about (1-based ply indices, in game order). */
  plies: number[];
}

// Lessons before praise when counts and cost tie.
const KIND_ORDER: TakeawayKind[] = ["error", "allowed", "missed", "found"];

/** How a takeaway counts an event of a move, or null when the event is not a lesson. */
function kindOf(ply: PlyReview, e: CoachEvent): TakeawayKind | null {
  const bad = !!ply.classification && LEARNABLE.has(ply.classification);
  switch (e.chain[0]) {
    case "permits":
      return bad ? "allowed" : null;
    case "misses":
    case "misses_same":
      return bad ? "missed" : null;
    case "base":
      if (bad) return tagWords(e.tag).error ? "error" : null;
      return e.actor === ply.mover ? "found" : null;
    default:
      return null;
  }
}

/** Events a move's sentences say (those are the ones the coach stood behind). */
function saidEvents(ply: PlyReview): CoachEvent[] {
  const note = ply.coach;
  if (!note) return [];
  const seen = new Set<number>();
  const out: CoachEvent[] = [];
  for (const s of note.sentences) {
    const e = s.event == null ? undefined : note.events[s.event];
    if (e && s.event != null && !seen.has(s.event)) {
      seen.add(s.event);
      out.push(e);
    }
  }
  return out;
}

function phrase(kind: TakeawayKind, e: CoachEvent, n: number): string {
  const w = tagWords(e.tag);
  if (kind === "error" && w.error) return n === 1 ? w.error.one : w.error.many(n);
  const verb = kind === "allowed" ? "Allowed" : kind === "missed" ? "Missed" : "Found";
  return n === 1 ? `${verb} ${w.one}` : `${verb} ${n} ${w.many}`;
}

/**
 * End-of-game lessons aggregated from the coach's events, e.g. "Allowed 3 forks",
 * most frequent first, then most costly. Each counts a move once, however many sentences say it.
 */
export function gameTakeaways(review: ReviewResult, max = 4): Takeaway[] {
  const groups = new Map<string, { side: Side; kind: TakeawayKind; event: CoachEvent; plies: number[]; lost: number }>();
  for (const ply of review.plies) {
    for (const e of saidEvents(ply)) {
      const kind = kindOf(ply, e);
      if (!kind) continue;
      const key = `${ply.mover}|${kind}|${tagFamily(e.tag)}`;
      const g = groups.get(key);
      if (!g) groups.set(key, { side: ply.mover, kind, event: e, plies: [ply.ply], lost: ply.ep_lost });
      else if (!g.plies.includes(ply.ply)) {
        g.plies.push(ply.ply);
        g.lost += ply.ep_lost;
      }
    }
  }
  return [...groups.values()]
    .sort(
      (a, b) =>
        b.plies.length - a.plies.length ||
        b.lost - a.lost ||
        KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind) ||
        a.plies[0]! - b.plies[0]!,
    )
    .slice(0, max)
    .map((g) => ({ side: g.side, kind: g.kind, text: phrase(g.kind, g.event, g.plies.length), plies: g.plies }));
}
