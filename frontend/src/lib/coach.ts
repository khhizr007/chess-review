import type { ArrowKind } from "../api/generated/ArrowKind";
import type { Classification } from "../api/generated/Classification";
import type { CoachNote } from "../api/generated/CoachNote";
import type { MoveRef } from "../api/generated/MoveRef";
import type { PlyReview } from "../api/generated/PlyReview";
import type { ReviewResult } from "../api/generated/ReviewResult";
import type { Sentence } from "../api/generated/Sentence";
import type { Side } from "../api/generated/Side";
import { uciSquares } from "./fen";
import { CLASSIFICATIONS, MISS_KINDS, moveNumberLabel } from "./format";

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
      // The coach says so itself when it has a sentence for the move.
      if (!ply.coach?.sentences.length) advice.push("It was the only legal move.");
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
  // The backend coach explains bad moves (what they allow and miss) and good
  // ones (what they do, threaten and stop).
  const sentences = ply.coach?.sentences ?? [];
  const learnable = c !== null && LEARNABLE.has(c);
  if (sentences.length > 0 && learnable) {
    // The coach names the motif and the better move: its sentences replace the
    // generic advice, and the engine facts stay in the details drawer.
    advice.length = 0;
  } else if (sentences.length === 0 && learnable) {
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

/** What a move's board arrows come from: a game move, or a judged sideline move with its UCI. */
export interface ArrowSource {
  uci: string;
  classification: Classification | null;
  played_is_best: boolean;
  show_best_move: boolean;
  best_move: MoveRef | null;
  coach: CoachNote | null;
}

/** Classes that show no arrow: the move played was the one to play. */
const NO_ARROWS: ReadonlySet<Classification> = new Set(["book", "best", "brilliant", "great_find", "forced"]);

/**
 * The arrows a move shows by default, as on Chess.com (docs/polish-gaps.md #7): none on book and
 * best moves; otherwise the best move's arrow (with `withBest`) and one arrow of the coach's main
 * sentence, never one along the move just played.
 */
export function moveArrows(move: ArrowSource, withBest = true): CoachArrow[] {
  if (move.played_is_best || (move.classification && NO_ARROWS.has(move.classification))) return [];
  const out: CoachArrow[] = [];
  const best = withBest && move.show_best_move && move.best_move ? uciSquares(move.best_move.uci) : null;
  if (best) out.push({ ...best, kind: "best" });
  const played = uciSquares(move.uci);
  const coach = move.coach?.sentences[0]?.fragments
    .flatMap((f) => f.arrows)
    .find((a) => !(played && a.from === played.from && a.to === played.to) && !out.some((b) => b.from === a.from && b.to === a.to));
  if (coach) out.push({ from: coach.from, to: coach.to, kind: coach.kind });
  return out;
}

/** The coach avatar's expression. */
export type CoachMood = "impressed" | "happy" | "neutral" | "concerned" | "sad";

/** Avatar expression for a move, from its classification (the API has no emotion yet). */
export function coachMood(c: Classification | null | undefined): CoachMood {
  switch (c) {
    case "brilliant":
    case "great_find":
      return "impressed";
    case "best":
      return "happy";
    case "mistake":
    case "miss":
      return "concerned";
    case "blunder":
      return "sad";
    default:
      return "neutral";
  }
}

/**
 * The coach's one line atop the report, as in the reference: the result, told to the player when
 * their side is known, then an invitation. Accuracy is left to the boxes just below it.
 */
export function reportHeadline(review: ReviewResult): string {
  const result = review.game.result;
  const winner: Side | null = result === "1-0" ? "white" : result === "0-1" ? "black" : null;
  const me = review.settings.player_side ?? null;
  const last = review.plies[review.plies.length - 1];
  const how = last?.flags.checkmate ? " by checkmate" : "";
  let told: string;
  if (winner) {
    if (me) told = me === winner ? `You won${how}.` : `You lost${how}.`;
    else told = `${playerName(review, winner)} won${how}.`;
  } else if (result === "1/2-1/2") told = "The game was drawn.";
  else told = "Here's how the game went.";
  return `${told} Let's review!`;
}

/** Report notes below the tallies: the turning point and the opening. */
export function reportNotes(review: ReviewResult): string[] {
  const out: string[] = [];
  const swingPly = review.biggest_swings[0];
  const swing = swingPly ? review.plies[swingPly - 1] : undefined;
  if (swing && swing.ep_lost >= 0.1) {
    const label = swing.classification ? ` (${CLASSIFICATIONS[swing.classification].label.toLowerCase()})` : "";
    out.push(`The turning point was ${moveNumberLabel(swing.move_number, swing.mover)} ${swing.san}${label}.`);
  }
  if (review.game.opening) out.push(`Opening: ${review.game.opening.name}.`);
  return out;
}
