// Usage: node bench.cjs <stockfish pkg dir> <positions.json> <flavor> <threads> <set> <depth> <cold|warm> [limit]
// Prints one JSON line per position: time and nodes at which each depth completed (MultiPV 3,
// multipv 1 line, exact scores only), the depth-limit eval, and a final summary line.
const [, , pkg, posFile, flavor, threads, set, depthArg, mode, limitArg] = process.argv;
const positions = require(posFile)[set].map((p) => (typeof p === "string" ? { fen: p } : p));
const list = limitArg ? positions.slice(0, Number(limitArg)) : positions;
const maxDepth = Number(depthArg);

const t0 = performance.now();
const rss0 = process.memoryUsage().rss;
let waiter = null;
const engine = require(pkg)(flavor, (line) => {
  if (waiter) waiter(line);
});
const send = (cmd) => engine.processCommand(cmd);
function until(pred, onLine) {
  return new Promise((resolve) => {
    waiter = (line) => {
      if (onLine) onLine(line);
      if (pred(line)) {
        waiter = null;
        resolve(line);
      }
    };
  });
}

function parseInfo(line) {
  if (!line.startsWith("info ") || !line.includes(" pv ")) return null;
  const tok = line.split(" ");
  const get = (k) => {
    const i = tok.indexOf(k);
    return i < 0 ? undefined : tok[i + 1];
  };
  if (tok.includes("lowerbound") || tok.includes("upperbound")) return null;
  const si = tok.indexOf("score");
  return {
    depth: Number(get("depth")),
    multipv: Number(get("multipv") ?? 1),
    nodes: Number(get("nodes")),
    nps: Number(get("nps")),
    cp: tok[si + 1] === "cp" ? Number(tok[si + 2]) : null,
    mate: tok[si + 1] === "mate" ? Number(tok[si + 2]) : null,
    best: get("pv"),
  };
}

(async () => {
  const ready = until((l) => l === "uciok");
  send("uci");
  await ready;
  send(`setoption name Threads value ${threads}`);
  send("setoption name Hash value 16");
  send(`setoption name MultiPV value ${process.env.MULTIPV ?? 3}`);
  let r = until((l) => l === "readyok");
  send("isready");
  await r;
  const startupMs = performance.now() - t0;
  const rows = [];
  for (const p of list) {
    if (mode === "cold") {
      send("ucinewgame");
      send("setoption name Clear Hash");
      r = until((l) => l === "readyok");
      send("isready");
      await r;
    }
    const at = {};
    let last = null;
    const s = performance.now();
    const done = until(
      (l) => l.startsWith("bestmove"),
      (l) => {
        const i = parseInfo(l);
        if (i && i.multipv === 1 && !(i.depth in at)) {
          at[i.depth] = [Math.round(performance.now() - s), i.nodes];
          last = i;
        }
      },
    );
    send(`position fen ${p.fen}`);
    send(`go depth ${maxDepth}`);
    await done;
    const row = { ...p, ms: Math.round(performance.now() - s), at, cp: last?.cp, mate: last?.mate, best: last?.best, nps: last?.nps };
    rows.push(row);
    console.log(JSON.stringify(row));
  }
  const rssMb = Math.round((process.memoryUsage().rss - rss0) / 1e6);
  console.log(JSON.stringify({ summary: true, flavor, threads: Number(threads), set, maxDepth, mode, startupMs: Math.round(startupMs), rssMb }));
  engine.terminate?.();
  process.exit(0);
})();
