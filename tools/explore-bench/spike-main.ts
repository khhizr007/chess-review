// Phase 0 spike (throwaway): times Stockfish 19 Lite (single-threaded WASM) in a Web Worker.
// Results land on window.__spike; ?run=sample|sideline-cold|sideline-warm picks the set.
import data from "./positions.json";

type At = Record<number, [number, number]>;
const log = (s: string) => {
  document.getElementById("log")!.textContent += s + "\n";
};
const out: Record<string, unknown> = { done: false, rows: [] as unknown[] };
(window as unknown as { __spike: unknown }).__spike = out;

const QS = new URLSearchParams(location.search);
const ENGINE = `/engine/stockfish-19-${QS.get("engine") ?? "lite-single"}.js`;
const THREADS = Number(QS.get("threads") ?? 1);

let onLine: ((l: string) => void) | null = null;
function until(worker: Worker, pred: (l: string) => boolean, each?: (l: string) => void) {
  return new Promise<string>((resolve) => {
    onLine = (l) => {
      each?.(l);
      if (pred(l)) {
        onLine = null;
        resolve(l);
      }
    };
  });
}

async function main() {
  const params = new URLSearchParams(location.search);
  const run = params.get("run") ?? "sample";
  const depth = Number(params.get("depth") ?? 18);

  const wasm = await fetch(ENGINE.replace(".js", ".wasm"), { cache: "no-store" });
  out.wasmType = wasm.headers.get("content-type");
  out.wasmBytes = (await wasm.arrayBuffer()).byteLength;

  const t0 = performance.now();
  const worker = new Worker(ENGINE);
  worker.onmessage = (e) => onLine?.(String(e.data));
  const send = (c: string) => worker.postMessage(c);
  let w = until(worker, (l) => l === "uciok");
  send("uci");
  await w;
  out.uciokMs = Math.round(performance.now() - t0);
  send(`setoption name Threads value ${THREADS}`);
  send("setoption name Hash value 16");
  out.threads = THREADS;
  out.isolated = crossOriginIsolated;
  send("setoption name MultiPV value 3");
  w = until(worker, (l) => l === "readyok");
  send("isready");
  await w;
  out.readyMs = Math.round(performance.now() - t0);
  log(`engine ready in ${out.readyMs} ms (wasm ${out.wasmBytes} B, ${out.wasmType})`);

  const fens: string[] =
    run === "sample" ? data.sample.map((p: { fen: string }) => p.fen) : (data.sideline as string[]);
  const cold = run !== "sideline-warm";
  for (const fen of fens) {
    if (cold) {
      send("ucinewgame");
      send("setoption name Clear Hash");
      w = until(worker, (l) => l === "readyok");
      send("isready");
      await w;
    }
    const at: At = {};
    const s = performance.now();
    let nps = 0;
    w = until(
      worker,
      (l) => l.startsWith("bestmove"),
      (l) => {
        if (!l.startsWith("info ") || !l.includes(" pv ") || /bound/.test(l)) return;
        const tok = l.split(" ");
        const get = (k: string) => tok[tok.indexOf(k) + 1];
        const d = Number(get("depth"));
        const mpv = tok.includes("multipv") ? Number(get("multipv")) : 1;
        if (mpv === 1 && !(d in at)) at[d] = [Math.round(performance.now() - s), Number(get("nodes"))];
        nps = Number(get("nps"));
      },
    );
    send(`position fen ${fen}`);
    send(`go depth ${depth}`);
    await w;
    const row = { fen, ms: Math.round(performance.now() - s), at, nps };
    (out.rows as unknown[]).push(row);
    log(`${row.ms} ms  d8 ${at[8]?.[0]}  d12 ${at[12]?.[0]}  d16 ${at[16]?.[0]}  nps ${nps}`);
  }
  out.done = true;
  log("done");
  worker.terminate();
}

main().catch((e) => {
  out.error = String(e);
  log(String(e));
});
