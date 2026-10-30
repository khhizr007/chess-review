// Sidelines: moves the reader plays on the board instead of the game's (docs/explore-mode.md §5.2).
// Every explored move is kept for the session in a tree of variations, and the board
// shows either a game position or one explored move.
import type { PlyReview } from "../api/generated/PlyReview";
import type { Side } from "../api/generated/Side";
import type { PlayedMove } from "./chess";

/** A move the reader played, with the moves played after it. */
export interface ExploreNode {
  move: PlayedMove;
  /** Moves played from the position after it, in the order played: the first continues its line. */
  next: ExploreNode[];
}

/** Explored lines by the game ply they branch from: the moves played instead of game move `ply + 1`. */
export type Explored = Readonly<Record<number, ExploreNode[]>>;

/** Where the board is, and every line explored so far. */
export interface BoardState {
  /** The game position shown, or the one the shown sideline branches from. */
  ply: number;
  /** Child indices from `explored[ply]` down to the explored move shown; empty on a game position. */
  path: number[];
  explored: Explored;
}

/** The line the board is in: from its branch through the shown move to the end of the move's line. */
export interface Sideline {
  /** Game ply the sideline branches from: its first move replaces game move `fromPly + 1`. */
  fromPly: number;
  moves: PlayedMove[];
  /** Sideline moves shown: k is the position after `moves[k - 1]`. */
  cursor: number;
}

/** Whether the board shows an explored position rather than a game position. */
export function inSideline(state: BoardState): boolean {
  return state.path.length > 0;
}

/** The explored moves from the branch down `path`. */
function nodesOn(explored: Explored, ply: number, path: number[]): ExploreNode[] {
  const out: ExploreNode[] = [];
  let level = explored[ply] ?? [];
  for (const i of path) {
    const node = level[i];
    if (!node) break;
    out.push(node);
    level = node.next;
  }
  return out;
}

/** `level` (the branches at ply, or a node's `next`) with `node` added at the end of `path`'s level. */
function withNode(level: ExploreNode[], path: number[], node: ExploreNode): ExploreNode[] {
  if (path.length === 0) return [...level, node];
  const [i, ...rest] = path;
  return level.map((n, j) => (j === i ? { ...n, next: withNode(n.next, rest, node) } : n));
}

/**
 * Plays a move from the position shown.
 * - On a game position, the game's own next move follows the game.
 * - A move already explored from the position is entered again, with everything played after it.
 * - Any other move is added: a new line from a game position, the line's continuation at its end, and
 *   a variation inside it. Nothing explored is ever dropped.
 */
export function play(state: BoardState, plies: PlyReview[], move: PlayedMove): BoardState {
  const { ply, path, explored } = state;
  if (path.length === 0 && plies[ply]?.uci === move.uci) return { ...state, ply: ply + 1 };
  const level = path.length ? (nodesOn(explored, ply, path).at(-1)?.next ?? []) : (explored[ply] ?? []);
  const known = level.findIndex((n) => n.move.uci === move.uci);
  if (known >= 0) return { ...state, path: [...path, known] };
  return {
    ply,
    path: [...path, level.length],
    explored: { ...explored, [ply]: withNode(explored[ply] ?? [], path, { move, next: [] }) },
  };
}

/**
 * The moves → can go to inside a sideline: the shown move's continuations. None at the end of a line,
 * and several when variations were played there, which is when the reader is asked which one to follow.
 * Null on a game position, where → follows the game.
 */
export function nextMoves(state: BoardState): ExploreNode[] | null {
  if (!inSideline(state)) return null;
  return nodesOn(state.explored, state.ply, state.path).at(-1)?.next ?? [];
}

/** One step back inside a sideline; back from its first move is its branch, a game position. */
export function back(state: BoardState): BoardState | null {
  return inSideline(state) ? { ...state, path: state.path.slice(0, -1) } : null;
}

/** The line the board is in, through the shown move and on along each move's first continuation. */
export function sidelineOf(state: BoardState): Sideline | null {
  if (!inSideline(state)) return null;
  const nodes = nodesOn(state.explored, state.ply, state.path);
  for (let next = nodes.at(-1)?.next[0]; next; next = next.next[0]) nodes.push(next);
  return { fromPly: state.ply, moves: nodes.map((n) => n.move), cursor: state.path.length };
}

/**
 * The game ply whose explored lines the move list shows: the lines the board
 * is in, or those that replace the game move shown. Null when there are none.
 */
export function listedBranch(state: BoardState): number | null {
  if (inSideline(state)) return state.ply;
  const replaced = state.ply - 1;
  return state.explored[replaced]?.length ? replaced : null;
}

/** The move-list label of each sideline move: "12. Nf3", "12... Nc6", and no number for Black after White. */
export function sidelineLabels(branchFen: string, count: number): { number: string | null; mover: Side }[] {
  return Array.from({ length: count }, (_, i) => moveLabel(branchFen, i, i === 0));
}

/**
 * The label of the explored move `depth` moves after the branch position. Black's move is numbered
 * ("12...") only when it opens a line or follows a variation.
 */
export function moveLabel(branchFen: string, depth: number, opens: boolean): { number: string | null; mover: Side } {
  const [, turn, , , , full] = branchFen.split(" ");
  const blackFirst = turn === "b";
  const halfMoves = depth + (blackFirst ? 1 : 0);
  const mover: Side = halfMoves % 2 ? "black" : "white";
  const moveNumber = (Number(full) || 1) + Math.floor(halfMoves / 2);
  const number = mover === "white" ? `${moveNumber}.` : opens ? `${moveNumber}...` : null;
  return { number, mover };
}
