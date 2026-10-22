// When the review plays which sound. The rules follow the reference web client's Game Review
// (docs/sound.md). The audio is a local sound pack where one is installed, else this project's own
// synth (samples.ts, synth.ts).
import type { Classification } from "../api/generated/Classification";
import type { MoveFlags } from "../api/generated/MoveFlags";
import type { PlyReview } from "../api/generated/PlyReview";
import type { Side } from "../api/generated/Side";
import { loadSamples, playSample } from "./samples";
import type { SoundName } from "./synth";

export type { SoundName } from "./synth";

// Decode the pack before the first move is stepped through.
void loadSamples();

/** The board sound of a move. Only one plays; the first that applies wins (a mate is a check). */
export function moveSound(flags: MoveFlags): SoundName {
  if (flags.check || flags.checkmate) return "check";
  if (flags.promotion) return "promote";
  if (flags.capture) return "capture";
  if (flags.castle) return "castle";
  return "move";
}

/** Move flags read from SAN, for engine-line moves that carry no flags of their own. */
export function sanFlags(san: string): MoveFlags {
  return {
    check: san.includes("+"),
    checkmate: san.includes("#"),
    stalemate: false,
    capture: san.includes("x"),
    castle: san.startsWith("O-O"),
    promotion: san.includes("="),
    en_passant: false,
  };
}

/**
 * Sounds for moving the board from ply `from` to ply `to`:
 * - one step back plays the move taken back;
 * - any other change plays the move arrived at, and the start position plays nothing;
 * - arriving at a brilliant move other than by stepping back adds the brilliant sound, but only for
 *   the reviewing player's moves (`playerSide`; either side's when no side was chosen).
 */
export function navigationSounds(plies: PlyReview[], from: number, to: number, playerSide: Side | null): SoundName[] {
  if (to === from) return [];
  if (to === from - 1) {
    const undone = plies[from - 1];
    return undone ? [moveSound(undone.flags)] : [];
  }
  const arrived = plies[to - 1];
  if (!arrived) return [];
  const sounds = [moveSound(arrived.flags)];
  if (arrived.classification === "brilliant" && (playerSide === null || playerSide === arrived.mover)) sounds.push("brilliant");
  return sounds;
}

/**
 * Sounds for moving through a sideline from position `from` to `to` (0 = branch position), by the
 * same rules as the game: one step back plays the move taken back, anything else the move arrived at.
 */
export function sidelineSounds(moves: { flags: MoveFlags }[], from: number, to: number): SoundName[] {
  if (to === from) return [];
  const move = to === from - 1 ? moves[from - 1] : moves[to - 1];
  return move ? [moveSound(move.flags)] : [];
}

/** The sound for a solved retry, by how the move found is classified. */
export function retrySound(classification: Classification): SoundName {
  switch (classification) {
    case "brilliant":
    case "great_find":
    case "tricky":
    case "best":
      return "correct";
    case "excellent":
      return "result_good";
    case "good":
    case "book":
    case "forced":
      return "result_ok";
    default:
      return "result_bad";
  }
}

/** Plays sounds together, as the reference client layers the brilliant sound over the move. */
export function playSounds(names: SoundName[]): void {
  for (const name of names) playSample(name);
}
