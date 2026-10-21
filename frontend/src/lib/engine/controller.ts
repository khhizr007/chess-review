// The browser engine (docs/explore-mode.md §5.3): one Stockfish WASM worker that searches the
// position shown in a sideline. A new position supersedes the running search; results are cached
// per position, and the hash is kept between positions.
import type { Line } from "../../api/generated/Line";
import type { Side } from "../../api/generated/Side";
import { legalMoveCount, lineMoves, type PlayedMove } from "../chess";
import { parseInfo, whiteEval, type UciInfo } from "./uci";

/** The build in `public/engine/` (stockfish@19.0.1, THIRD_PARTY.md). */
export const ENGINE_NAME = "Stockfish 19 Lite";
export const ENGINE_URL = "/engine/stockfish-19-lite-single.js";

/** Moves shown per engine line, as the server caps them. */
const LINE_PLIES = 12;
const CACHE_SIZE = 500;

/** An engine line whose moves can be played onto the board. */
export interface LiveLine extends Line {
  moves: PlayedMove[];
}

export interface Analysis {
  fen: string;
  /** Depth of the lines (all lines are from the same completed depth). */
  depth: number;
  lines: LiveLine[];
  /** Nodes per second of the search that produced it. */
  nps: number;
  /** The search reached the depth asked for. */
  done: boolean;
}

export type EngineStatus = "loading" | "ready" | "error";

/** The engine as the controller sees it: UCI commands in, lines out (a Web Worker, or a fake in tests). */
export interface EngineWorker {
  send(command: string): void;
  terminate(): void;
}

export type WorkerFactory = (onLine: (line: string) => void, onError: (message: string) => void) => EngineWorker;

/** Starts the engine build in a Web Worker. A missing or unloadable file reports an error. */
export const browserWorker: WorkerFactory = (onLine, onError) => {
  const worker = new Worker(ENGINE_URL);
  worker.onmessage = (e) => onLine(String(e.data));
  worker.onerror = (e) => {
    e.preventDefault();
    onError(e.message || "The engine could not be loaded.");
  };
  return { send: (c) => worker.postMessage(c), terminate: () => worker.terminate() };
};

export interface EngineOptions {
  multipv: number;
  hashMb: number;
  /** Shortest time between two updates of a running search. */
  throttleMs: number;
  now: () => number;
}

interface Job {
  fen: string;
  depth: number;
  onUpdate: (analysis: Analysis) => void;
}

export class EngineController {
  private readonly options: EngineOptions;
  private worker: EngineWorker | null = null;
  private ready = false;
  /** The engine failed to load; it is not tried again. */
  private failed = false;
  private chess960 = false;
  private newGame = false;
  /** The job whose results are wanted (the latest `analyze`), if any. */
  private wanted: Job | null = null;
  /** The job waiting for the running search to stop. */
  private queued: Job | null = null;
  /** The job of the running search. */
  private running: Job | null = null;
  private stopSent = false;
  private expected = 1;
  private infos = new Map<number, UciInfo>();
  /** The lines of the deepest depth completed so far by the running search. */
  private complete: UciInfo[] = [];
  private lastEmit = -Infinity;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private readonly cache = new Map<string, Analysis>();

  constructor(
    private readonly factory: WorkerFactory,
    options: Partial<EngineOptions> = {},
    private readonly onStatus: (status: EngineStatus, error?: string) => void = () => {},
  ) {
    this.options = { multipv: 3, hashMb: 16, throttleMs: 100, now: () => performance.now(), ...options };
  }

  /** Starts a new game: clears the engine's memory before the next search. */
  setGame(chess960: boolean): void {
    this.chess960 = chess960;
    this.newGame = true;
    this.cache.clear();
  }

  /** Searches `fen` to `depth`, reporting each completed depth; replaces any earlier request. */
  analyze(fen: string, depth: number, onUpdate: (analysis: Analysis) => void): void {
    if (this.failed) return;
    this.start();
    const job = { fen, depth, onUpdate };
    this.wanted = job;
    this.clearTimer();
    const cached = this.cache.get(fen);
    if (cached) onUpdate({ ...cached, done: cached.depth >= depth });
    if (cached && cached.depth >= depth) {
      this.queued = null;
      this.halt();
      return;
    }
    this.queued = job;
    if (this.running) this.halt();
    else this.pump();
  }

  /** Stops searching; the result so far is kept in the cache. */
  stop(): void {
    this.wanted = null;
    this.queued = null;
    this.clearTimer();
    this.halt();
  }

  dispose(): void {
    this.stop();
    this.worker?.terminate();
    this.worker = null;
    this.ready = false;
    this.running = null;
  }

  private start(): void {
    if (this.worker) return;
    this.onStatus("loading");
    this.worker = this.factory(
      (line) => this.onLine(line),
      (message) => this.onError(message),
    );
    this.worker.send("uci");
  }

  private send(command: string): void {
    this.worker?.send(command);
  }

  private halt(): void {
    if (this.running && !this.stopSent) {
      this.stopSent = true;
      this.send("stop");
    }
  }

  private onError(message: string): void {
    this.failed = true;
    this.worker?.terminate();
    this.worker = null;
    this.ready = false;
    this.running = null;
    this.queued = null;
    this.onStatus("error", message);
  }

  private onLine(line: string): void {
    if (line === "uciok") {
      this.send(`setoption name Hash value ${this.options.hashMb}`);
      this.send(`setoption name MultiPV value ${this.options.multipv}`);
      this.send("isready");
    } else if (line === "readyok" && !this.ready) {
      this.ready = true;
      this.onStatus("ready");
      this.pump();
    } else if (line.startsWith("bestmove")) {
      this.finish();
    } else if (this.running) {
      const info = parseInfo(line);
      if (info) this.onInfo(info);
    }
  }

  private pump(): void {
    const job = this.queued;
    if (!this.ready || this.running || !job) return;
    this.queued = null;
    this.running = job;
    this.stopSent = false;
    this.infos.clear();
    this.complete = [];
    this.expected = Math.max(1, Math.min(this.options.multipv, legalMoveCount(job.fen)));
    if (this.newGame) {
      this.newGame = false;
      this.send("ucinewgame");
      this.send(`setoption name UCI_Chess960 value ${this.chess960}`);
    }
    this.send(`position fen ${job.fen}`);
    this.send(`go depth ${job.depth}`);
  }

  private onInfo(info: UciInfo): void {
    if (info.multipv > this.expected) return;
    this.infos.set(info.multipv, info);
    if (info.multipv !== this.expected) return;
    // Every line is printed again after each completed depth; the last one completes the set.
    const set = [...Array(this.expected).keys()].map((i) => this.infos.get(i + 1));
    if (set.some((l) => !l || l.depth !== info.depth)) return;
    this.complete = set as UciInfo[];
    if (this.running === this.wanted && !this.stopSent) this.schedule();
  }

  private schedule(): void {
    const wait = this.lastEmit + this.options.throttleMs - this.options.now();
    if (wait <= 0) this.flush(false);
    else this.timer ??= setTimeout(() => this.flush(false), wait);
  }

  private clearTimer(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }

  /** Reports the deepest complete set of lines of the running search, and caches it. */
  private flush(done: boolean): void {
    this.timer = null;
    const job = this.running;
    if (!job || this.complete.length === 0) return;
    const cached = this.cache.get(job.fen);
    // A shallower result than one already known is not news (a revisited position searches again).
    if (cached && cached.depth >= this.complete[0]!.depth && !done) return;
    const analysis = this.snapshot(job, done);
    if (!cached || analysis.depth >= cached.depth) this.remember(analysis);
    if (job === this.wanted) {
      this.lastEmit = this.options.now();
      job.onUpdate(cached && cached.depth > analysis.depth ? { ...cached, done } : analysis);
    }
  }

  private finish(): void {
    const job = this.running;
    this.clearTimer();
    // A search that ran to its depth is done; a stopped one only leaves its result in the cache.
    if (job) this.flush(!this.stopSent && job === this.wanted);
    this.running = null;
    this.stopSent = false;
    this.pump();
  }

  private snapshot(job: Job, done: boolean): Analysis {
    const toMove: Side = job.fen.split(" ")[1] === "b" ? "black" : "white";
    const lines = this.complete.map((info, i) => ({
      rank: i + 1,
      eval: whiteEval(info.score, toMove),
      depth: info.depth,
      moves: lineMoves(job.fen, info.pv, LINE_PLIES, this.chess960),
    }));
    return { fen: job.fen, depth: this.complete[0]!.depth, lines, nps: this.complete[0]!.nps, done };
  }

  private remember(analysis: Analysis): void {
    this.cache.delete(analysis.fen);
    this.cache.set(analysis.fen, analysis);
    if (this.cache.size > CACHE_SIZE) this.cache.delete(this.cache.keys().next().value!);
  }
}
