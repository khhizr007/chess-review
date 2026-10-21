// Sidelines: moves the reader plays on the board instead of the game's (docs/explore-mode.md §5.2).
// One sideline at a time; it branches from a game position and is replaced by the next one.
import type { PlyReview } from "../api/generated/PlyReview";
import type { Side } from "../api/generated/Side";
import type { PlayedMove } from "./chess";

export interface Sideline {
  /** Game ply the sideline branches from: its first move replaces game move `fromPly + 1`. */
  fromPly: number;
  moves: PlayedMove[];
  /** Sideline moves shown: 0 is the branch position, k the position after `moves[k - 1]`. */
  cursor: number;
}

/** Where the board is: a game ply, and the sideline open from it (if any). */
export interface BoardState {
  ply: number;
  sideline: Sideline | null;
}

/** Whether the board shows a sideline position rather than a game position. */
export function inSideline(state: BoardState): boolean {
  return !!state.sideline && state.sideline.cursor > 0;
}

/**
 * Plays a move from the position shown.
 * - On a game position, the game's own next move just follows the game; any other move starts a
 *   new sideline there (replacing an open one).
 * - In a sideline, the move replaces everything after the cursor, unless it is the sideline's own
 *   next move, which is kept with the rest of the line.
 */
export function play(state: BoardState, plies: PlyReview[], move: PlayedMove): BoardState {
  const line = state.sideline;
  if (line && line.cursor > 0) {
    if (line.moves[line.cursor]?.uci === move.uci) return { ...state, sideline: { ...line, cursor: line.cursor + 1 } };
    return { ...state, sideline: { ...line, moves: [...line.moves.slice(0, line.cursor), move], cursor: line.cursor + 1 } };
  }
  if (line && line.moves[0]?.uci === move.uci) return { ...state, sideline: { ...line, cursor: 1 } };
  if (plies[state.ply]?.uci === move.uci) return { ply: state.ply + 1, sideline: null };
  return { ply: state.ply, sideline: { fromPly: state.ply, moves: [move], cursor: 1 } };
}

/**
 * Steps through the open sideline. Back from its first move lands on the branch position, where
 * the sideline stays listed; returns null when there is no further step inside the sideline.
 */
export function stepSideline(line: Sideline, dir: 1 | -1): Sideline | null {
  const cursor = line.cursor + dir;
  if (cursor < 0 || cursor > line.moves.length) return null;
  return { ...line, cursor };
}

/** The move-list label of each sideline move: "12. Nf3", "12... Nc6", and no number for Black after White. */
export function sidelineLabels(branchFen: string, count: number): { number: string | null; mover: Side }[] {
  const [, turn, , , , full] = branchFen.split(" ");
  let mover: Side = turn === "b" ? "black" : "white";
  let moveNumber = Number(full) || 1;
  const out: { number: string | null; mover: Side }[] = [];
  for (let i = 0; i < count; i++) {
    const number = mover === "white" ? `${moveNumber}.` : i === 0 ? `${moveNumber}...` : null;
    out.push({ number, mover });
    if (mover === "black") moveNumber += 1;
    mover = mover === "white" ? "black" : "white";
  }
  return out;
}
