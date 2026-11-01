// Usage: node summarise.cjs <file.jsonl|file.json>...  (a .json file is a window.__spike dump)
const fs = require("fs");
const q = (xs, p) => {
  const s = xs.filter((x) => x != null).sort((a, b) => a - b);
  if (!s.length) return null;
  return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1) + 0.5))];
};
for (const f of process.argv.slice(2)) {
  let rows, summary;
  if (f.endsWith(".json")) {
    const d = JSON.parse(fs.readFileSync(f, "utf8"));
    rows = d.rows;
    summary = { readyMs: d.readyMs, wasmBytes: d.wasmBytes, wasmType: d.wasmType };
  } else {
    const lines = fs.readFileSync(f, "utf8").trim().split("\n").map((l) => JSON.parse(l));
    rows = lines.filter((r) => !r.summary);
    summary = lines.find((r) => r.summary);
  }
  const out = { file: f.split(/[\\/]/).pop(), n: rows.length, ...summary };
  for (const d of [8, 12, 14, 16, 18, 20]) {
    const ms = rows.map((r) => r.at[d]?.[0]);
    if (ms.every((x) => x == null)) continue;
    out[`d${d}`] = `med ${q(ms, 0.5)} / p90 ${q(ms, 0.9)} / max ${q(ms, 1)} ms`;
  }
  const deepest = Math.max(...Object.keys(rows[0].at).map(Number));
  out[`nodes_d${deepest}_med`] = q(rows.map((r) => r.at[deepest]?.[1]), 0.5);
  out.nps_med = q(rows.map((r) => r.nps), 0.5);
  console.log(JSON.stringify(out));
}
