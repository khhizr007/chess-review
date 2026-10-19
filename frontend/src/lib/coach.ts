import type { ArrowKind } from "../api/generated/ArrowKind";
import type { Classification } from "../api/generated/Classification";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Sentence } from "../api/generated/Sentence";
import type { Side } from "../api/generated/Side";
import { CLASSIFICATIONS, MISS_KINDS, formatAccuracy, moveNumberLabel } from "./format";

/** Coach speech for one move: everything shown in the bubble. */
export interface CoachMessage {
  classification: Classification | null;
  /** Rest of the headline after the move, e.g. "is a miss". */
  verdict: string;
  /** Plain-language advice, most important first. */
  advice: string[];
  /** The backend coach's sentences (motif and line), when it explains the move; they replace `advice`. */
  sentences: Sentence[];
  /** Engine-derived facts from the backend that the advice does not already say. */
  facts: string[];
  /** Mover's winning chances before and after, when the move changed them noticeably. */
  swing: { before: number; after: number } | null;
}

/** Classes worth stopping on: the coach explains them fully and offers Retry. */
export const LEARNABLE: ReadonlySet<Classification> = new Set(["inaccuracy", "mistake", "miss", "blunder"]);

export function playerName(review: ReviewResult, side: Side): string {
  return (side === "white" ? review.white.name : review.black.name) ?? (side === "white" ? "White" : "Black");
}

const BOOK_PREFIX = "Book move: ";

/** What a bad move did to the mover's game, judged from winning chances before and after. */
function consequence(ply: PlyReview): string {
  const { ep_before: before, ep_after: after } = ply;
  if (before >= 0.65) {
    if (after < 0.35) return "This throws away a winning position.";
    if (after < 0.5) return "This gives away the advantage.";
    return "This lets the opponent back into the game.";
  }
  if (before <= 0.35) return "This makes a difficult position worse.";
  return after < 0.35 ? "This hands the opponent a big advantage." : "This hands the opponent the upper hand.";
}

/** "Book move: Italian Game (C50)." → "Italian Game (C50)". */
function openingFromFacts(ply: PlyReview): string | null {
  const s = ply.explanation.find((e) => e.startsWith(BOOK_PREFIX));
  return s ? s.slice(BOOK_PREFIX.length).replace(/\.$/, "") : null;
}

// Facts the advice restates in its own words; the rest are shown as-is.
const RESTATED = ["Expected score for", "The engine's top choice was", BOOK_PREFIX, "Book move.", "This was the only legal move."];

export function coachMessage(review: ReviewResult, ply: PlyReview, next: PlyReview | undefined): CoachMessage {
  const c = ply.classification;
  const name = playerName(review, ply.mover);
  const best = ply.best_move?.san;
  const advice: string[] = [];
  let verdict = c ? `is ${CLASSIFICATIONS[c].phrase}` : "was played";

  switch (c) {
    case "book": {
      const lastBook = !next || next.classification !== "book";
      if (lastBook) verdict = "is the last book move";
      const opening = openingFromFacts(ply);
      advice.push(opening ? `Opening theory: ${opening}.` : "A well-known opening move.");
      if (lastBook && next) advice.push("From here on, the players are on their own.");
      break;
    }
    case "brilliant":
      advice.push("The best move here, and a hard one to see.");
      break;
    case "great_find":
      advice.push("The only move that keeps the position together. Well spotted.");
      break;
    case "best":
      advice.push(`${name} found the engine's top move.`);
      break;
    case "excellent":
      advice.push(best ? `Very close to the best move. The engine slightly preferred ${best}.` : "Very close to the best move.");
      break;
    case "good":
      advice.push(best ? `A reasonable move, but ${best} was stronger.` : "A reasonable move.");
      break;
    case "inaccuracy":
      advice.push(best ? `Not the most accurate. ${best} was better.` : "Not the most accurate move.");
      break;
    case "mistake":
      advice.push(consequence(ply));
      if (best) advice.push(`${best} was much stronger.`);
      break;
    case "miss":
      advice.push(ply.miss_kind ? `${name} ${MISS_KINDS[ply.miss_kind]}.` : `${name} missed a chance.`);
      if (best) advice.push(`${best} was the move to play.`);
      break;
    case "blunder":
      advice.push(`A serious error. ${consequence(ply)}`);
      if (best) advice.push(`${best} was the move to play.`);
      break;
    case "forced":
      advice.push("It was the only legal move.");
      break;
    case "tricky":
      advice.push("Not the engine's choice, but it sets a problem the opponent has to solve.");
      break;
    default:
      break;
  }

  if (ply.flags.checkmate) advice.push("Checkmate.");

  const delta = ply.ep_after - ply.ep_before;
  const swing = Math.abs(delta) >= 0.02 && c !== "book" ? { before: ply.ep_before, after: ply.ep_after } : null;
  let facts = ply.explanation.filter((s) => !RESTATED.some((p) => s.startsWith(p)));
  const sentences = c && LEARNABLE.has(c) ? (ply.coach?.sentences ?? []) : [];
  if (sentences.length > 0) {
    // The coach names the motif and the better move: its sentences replace the
    // generic advice, and the engine facts stay in the details drawer.
    advice.length = 0;
  } else if (c && LEARNABLE.has(c)) {
    // Without a coach explanation, the engine facts are the "why": say them in the bubble.
    advice.push(...facts);
    facts = [];
  }

  return { classification: c, verdict, advice, sentences, facts, swing };
}

/** A board arrow drawn for the coach's explanation. */
export interface CoachArrow {
  from: string;
  to: string;
  kind: ArrowKind;
}

/**
 * Arrows of the coach's main sentence: those of its first fragment that has any,
 * so the board shows what the sentence is about.
 */
export function coachArrows(ply: PlyReview): CoachArrow[] {
  const first = ply.coach?.sentences[0]?.fragments.find((f) => f.arrows.length > 0);
  return first ? first.arrows.map((a) => ({ from: a.from, to: a.to, kind: a.kind })) : [];
}

/** One-paragraph summary for the report view. */
export function reportSummary(review: ReviewResult): string[] {
  const out: string[] = [];
  const { white, black } = review;
  const wName = playerName(review, "white");
  const bName = playerName(review, "black");
  const result = review.game.result;
  if (result === "1-0") out.push(`${wName} won the game.`);
  else if (result === "0-1") out.push(`${bName} won the game.`);
  else if (result === "1/2-1/2") out.push("The game was drawn.");

  const wa = white.accuracy.overall;
  const ba = black.accuracy.overall;
  if (wa != null && ba != null) {
    const [hi, lo] = wa >= ba ? [wName, bName] : [bName, wName];
    out.push(
      Math.abs(wa - ba) < 2
        ? `Both sides played at a similar level (${formatAccuracy(wa)} vs ${formatAccuracy(ba)}).`
        : `${hi} played more accurately than ${lo} (${formatAccuracy(Math.max(wa, ba))} vs ${formatAccuracy(Math.min(wa, ba))}).`,
    );
  }

  const swingPly = review.biggest_swings[0];
  const swing = swingPly ? review.plies[swingPly - 1] : undefined;
  if (swing && swing.ep_lost >= 0.1) {
    const label = swing.classification ? ` (${CLASSIFICATIONS[swing.classification].label.toLowerCase()})` : "";
    out.push(`The turning point was ${moveNumberLabel(swing.move_number, swing.mover)} ${swing.san}${label}.`);
  }
  if (review.game.opening) out.push(`Opening: ${review.game.opening.name}.`);
  return out;
}
