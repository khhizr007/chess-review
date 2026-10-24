import { afterEach, describe, expect, it, vi } from "vitest";
import { EngineController, type Analysis, type EngineStatus, type WorkerFactory } from "./controller";

const START = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const AFTER_E4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1";
// Two legal moves only (Kh1-g1/g2 boxed in): the engine prints two lines whatever MultiPV says.
const TWO_MOVES = "7k/8/8/8/8/8/6pp/7K w - - 0 1";

/** A scripted engine: records commands, and the test feeds its output. */
function fakeEngine() {
  const sent: string[] = [];
  let emit: (line: string) => void = () => {};
  let fail: (message: string) => void = () => {};
  const factory: WorkerFactory = (onLine, onError) => {
    emit = onLine;
    fail = onError;
    return { send: (c) => sent.push(c), terminate: () => sent.push("<terminated>") };
  };
  return {
    sent,
    factory,
    out: (...lines: string[]) => lines.forEach((l) => emit(l)),
    fail: (m: string) => fail(m),
    /** Completes the handshake. */
    boot() {
      emit("uciok");
      emit("readyok");
    },
    /** Prints a full set of three lines for one depth. */
    depth(d: number, cps: [number, number, number], pvs: [string, string, string]) {
      for (let i = 0; i < 3; i++) emit(`info depth ${d} seldepth ${d + 4} multipv ${i + 1} score cp ${cps[i]} nodes ${d * 1000} nps 330000 pv ${pvs[i]}`);
    },
  };
}

function setup(throttleMs = 0) {
  const engine = fakeEngine();
  const statuses: EngineStatus[] = [];
  const controller = new EngineController(engine.factory, { throttleMs }, (s) => statuses.push(s));
  const updates: Analysis[] = [];
  return { engine, statuses, controller, updates, onUpdate: (a: Analysis) => updates.push(a) };
}

afterEach(() => vi.useRealTimers());

describe("EngineController", () => {
  it("starts the worker on first use and searches once the engine is ready", () => {
    const { engine, statuses, controller, onUpdate } = setup();
    expect(engine.sent).toEqual([]);
    controller.analyze(START, 18, onUpdate);
    expect(engine.sent).toEqual(["uci"]);
    engine.boot();
    expect(statuses).toEqual(["loading", "ready"]);
    expect(engine.sent).toEqual([
      "uci",
      "setoption name Hash value 16",
      "setoption name MultiPV value 3",
      "isready",
      `position fen ${START}`,
      "go depth 18",
    ]);
  });

  it("reports each completed depth as White-view lines with SAN, then the final result", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(AFTER_E4, 18, onUpdate);
    engine.boot();
    engine.out("info depth 10 multipv 1 score cp 20 lowerbound nodes 5 nps 5 pv e7e5");
    engine.depth(10, [-30, -35, -60], ["e7e5 g1f3", "c7c5", "e7e6 d2d4"]);
    // A partial set (line 1 of the next depth) is not reported until the set is complete.
    engine.out("info depth 11 multipv 1 score mate 3 nodes 9 nps 330000 pv d8h4");
    expect(updates).toHaveLength(1);
    expect(updates[0]).toMatchObject({ fen: AFTER_E4, depth: 10, nps: 330000, done: false });
    expect(updates[0]!.lines.map((l) => l.eval)).toEqual([
      { kind: "cp", cp: 30 },
      { kind: "cp", cp: 35 },
      { kind: "cp", cp: 60 },
    ]);
    expect(updates[0]!.lines[0]!.moves.map((m) => m.san)).toEqual(["e5", "Nf3"]);
    engine.out("bestmove e7e5");
    expect(updates[1]).toMatchObject({ depth: 10, done: true });
  });

  it("expects only as many lines as there are legal moves", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(TWO_MOVES, 18, onUpdate);
    engine.boot();
    engine.out("info depth 5 multipv 1 score cp -900 nodes 1 nps 1 pv h1g2", "info depth 5 multipv 2 score cp -950 nodes 1 nps 1 pv h1g1");
    expect(updates[0]?.lines).toHaveLength(2);
  });

  it("stops a running search for a new position and starts it once the engine has stopped", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(START, 18, onUpdate);
    engine.boot();
    engine.depth(8, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    controller.analyze(AFTER_E4, 18, onUpdate);
    controller.analyze(AFTER_E4, 18, onUpdate);
    expect(engine.sent.filter((c) => c === "stop")).toHaveLength(1);
    // Output of the stopped search is not reported.
    engine.depth(9, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    engine.out("bestmove e2e4");
    expect(updates.map((u) => u.fen)).toEqual([START]);
    expect(engine.sent.slice(-2)).toEqual([`position fen ${AFTER_E4}`, "go depth 18"]);
  });

  it("answers a position searched before from the cache, without a new search", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(START, 12, onUpdate);
    engine.boot();
    engine.depth(12, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    engine.out("bestmove e2e4");
    const searches = engine.sent.filter((c) => c.startsWith("go")).length;
    controller.analyze(START, 12, onUpdate);
    expect(updates.at(-1)).toMatchObject({ fen: START, depth: 12, done: true });
    expect(engine.sent.filter((c) => c.startsWith("go"))).toHaveLength(searches);
    // Asking for more depth shows the cached lines at once and searches on.
    controller.analyze(START, 16, onUpdate);
    expect(updates.at(-1)).toMatchObject({ depth: 12, done: false });
    expect(engine.sent.at(-1)).toBe("go depth 16");
  });

  it("does not report a revisited position's shallower depths again", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(START, 18, onUpdate);
    engine.boot();
    engine.depth(14, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    controller.stop();
    engine.out("bestmove e2e4");
    updates.length = 0;
    controller.analyze(START, 18, onUpdate);
    expect(updates).toHaveLength(1);
    engine.depth(10, [10, 5, 0], ["e2e4", "d2d4", "g1f3"]);
    expect(updates).toHaveLength(1);
    engine.depth(15, [31, 26, 21], ["e2e4", "d2d4", "g1f3"]);
    expect(updates.at(-1)?.depth).toBe(15);
  });

  it("stops without reporting the stopped search", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(START, 18, onUpdate);
    // Stopped before the engine is ready: nothing is searched.
    controller.stop();
    engine.boot();
    expect(engine.sent.filter((c) => c.startsWith("go") || c === "stop")).toEqual([]);
    controller.analyze(START, 18, onUpdate);
    engine.depth(8, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    controller.stop();
    engine.depth(9, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    engine.out("bestmove e2e4");
    expect(engine.sent.filter((c) => c === "stop")).toHaveLength(1);
    expect(updates.map((u) => u.depth)).toEqual([8]);
  });

  it("throttles updates of a running search", () => {
    vi.useFakeTimers();
    const { engine, controller, updates, onUpdate } = setup(100);
    controller.analyze(START, 18, onUpdate);
    engine.boot();
    engine.depth(5, [30, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    engine.depth(6, [31, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    engine.depth(7, [32, 25, 20], ["e2e4", "d2d4", "g1f3"]);
    expect(updates.map((u) => u.depth)).toEqual([5]);
    vi.advanceTimersByTime(100);
    expect(updates.map((u) => u.depth)).toEqual([5, 7]);
  });

  it("starts the next game with a cleared engine and the right castling mode", () => {
    const { engine, controller, onUpdate } = setup();
    controller.setGame(true);
    controller.analyze(START, 18, onUpdate);
    engine.boot();
    expect(engine.sent.slice(-4)).toEqual(["ucinewgame", "setoption name UCI_Chess960 value true", `position fen ${START}`, "go depth 18"]);
  });

  it("reports a worker that fails to load, and does not try again", () => {
    const { engine, statuses, controller, onUpdate } = setup();
    controller.analyze(START, 18, onUpdate);
    engine.fail("404");
    expect(statuses).toEqual(["loading", "error"]);
    controller.analyze(START, 18, onUpdate);
    expect(engine.sent.filter((c) => c === "uci")).toHaveLength(1);
  });

  it("searches for fewer lines when asked, and caches each line count apart", () => {
    const { engine, controller, updates, onUpdate } = setup();
    controller.analyze(START, 10, onUpdate, 1);
    engine.boot();
    expect(engine.sent.slice(-3)).toEqual(["setoption name MultiPV value 1", `position fen ${START}`, "go depth 10"]);
    engine.out("info depth 10 multipv 1 score cp 30 nodes 9 nps 9 pv e2e4", "bestmove e2e4");
    expect(updates.at(-1)).toMatchObject({ multipv: 1, depth: 10, done: true });
    expect(updates.at(-1)!.lines).toHaveLength(1);
    // Three lines of the same position are a different search, with the setting put back.
    controller.analyze(START, 10, onUpdate);
    expect(engine.sent.slice(-3)).toEqual(["setoption name MultiPV value 3", `position fen ${START}`, "go depth 10"]);
  });
});
