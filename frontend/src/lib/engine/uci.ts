// Reading the browser engine's UCI output (docs/explore-mode.md §5.3).
import type { Eval } from "../../api/generated/Eval";
import type { Side } from "../../api/generated/Side";

/** One `info … pv …` line with an exact score; bound scores and info without a PV are dropped. */
export interface UciInfo {
  depth: number;
  multipv: number;
  /** Score from the side to move's view, as the engine reports it. */
  score: { cp: number } | { mate: number };
  nodes: number;
  nps: number;
  pv: string[];
}

/** Parses an `info` line, or returns null for anything else (as `uci.rs` does on the server). */
export function parseInfo(line: string): UciInfo | null {
  if (!line.startsWith("info ")) return null;
  const tok = line.split(" ");
  const at = (key: string) => tok.indexOf(key);
  const num = (key: string, fallback: number) => {
    const i = at(key);
    const v = i < 0 ? NaN : Number(tok[i + 1]);
    return Number.isFinite(v) ? v : fallback;
  };
  const pvAt = at("pv");
  const scoreAt = at("score");
  if (pvAt < 0 || scoreAt < 0 || tok.includes("lowerbound") || tok.includes("upperbound")) return null;
  const kind = tok[scoreAt + 1];
  const value = Number(tok[scoreAt + 2]);
  if (!Number.isFinite(value) || (kind !== "cp" && kind !== "mate")) return null;
  const pv = tok.slice(pvAt + 1).filter(Boolean);
  if (pv.length === 0) return null;
  return {
    depth: num("depth", 0),
    multipv: num("multipv", 1),
    score: kind === "cp" ? { cp: value } : { mate: value },
    nodes: num("nodes", 0),
    nps: num("nps", 0),
    pv,
  };
}

/** The engine's side-to-move score as the API's White-view `Eval`. */
export function whiteEval(score: UciInfo["score"], toMove: Side): Eval {
  const other: Side = toMove === "white" ? "black" : "white";
  if ("cp" in score) return { kind: "cp", cp: toMove === "white" ? score.cp : -score.cp };
  return { kind: "mate", winner: score.mate > 0 ? toMove : other, moves: Math.abs(score.mate) };
}
