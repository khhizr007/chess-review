import { describe, expect, it } from "vitest";
import { parseInfo, whiteEval } from "./uci";

describe("parseInfo", () => {
  it("reads depth, line rank, score, nodes, speed and the PV", () => {
    const i = parseInfo("info depth 18 seldepth 24 multipv 2 score cp -35 nodes 1026627 nps 330000 hashfull 412 tbhits 0 time 3011 pv e7e5 g1f3 b8c6");
    expect(i).toEqual({ depth: 18, multipv: 2, score: { cp: -35 }, nodes: 1026627, nps: 330000, pv: ["e7e5", "g1f3", "b8c6"] });
  });

  it("reads mate scores and defaults the line rank to 1", () => {
    expect(parseInfo("info depth 5 score mate -2 nodes 10 nps 5 pv e1e2")?.score).toEqual({ mate: -2 });
    expect(parseInfo("info depth 5 score mate 1 pv d1h5")?.multipv).toBe(1);
  });

  it("drops bound scores, lines without a PV and other output", () => {
    expect(parseInfo("info depth 12 multipv 1 score cp 20 lowerbound nodes 5 pv e2e4")).toBeNull();
    expect(parseInfo("info depth 12 multipv 1 score cp 20 upperbound nodes 5 pv e2e4")).toBeNull();
    expect(parseInfo("info depth 1 currmove e2e4 currmovenumber 1")).toBeNull();
    expect(parseInfo("info string NNUE evaluation using nn-61e7af4bb97d.nnue")).toBeNull();
    expect(parseInfo("bestmove e2e4 ponder e7e5")).toBeNull();
  });
});

describe("whiteEval", () => {
  it("turns the side to move's score into White's view", () => {
    expect(whiteEval({ cp: 35 }, "white")).toEqual({ kind: "cp", cp: 35 });
    expect(whiteEval({ cp: 35 }, "black")).toEqual({ kind: "cp", cp: -35 });
  });

  it("names the mating side", () => {
    expect(whiteEval({ mate: 3 }, "black")).toEqual({ kind: "mate", winner: "black", moves: 3 });
    expect(whiteEval({ mate: -2 }, "black")).toEqual({ kind: "mate", winner: "white", moves: 2 });
  });
});
