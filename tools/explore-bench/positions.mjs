// Usage: node positions.mjs <deps node_modules> <games dir> <out.json>
// Samples FENs from calibration games: plies 12/24/36/48/60 of the first 8 games ("sample"),
// and plies 20..35 of the first game ("sideline", consecutive positions for the warm-hash test).
import fs from "fs";
import path from "path";
import { pathToFileURL } from "url";

const [, , mods, gamesDir, out] = process.argv;
const esm = (m) => import(pathToFileURL(`${mods}/chessops/dist/esm/${m}.js`).href);
const { parsePgn, startingPosition } = await esm("pgn");
const { parseSan } = await esm("san");
const { makeFen } = await esm("fen");

function fens(pgnText) {
  const game = parsePgn(pgnText)[0];
  const pos = startingPosition(game.headers).unwrap();
  const list = [makeFen(pos.toSetup())];
  for (const node of game.moves.mainline()) {
    const move = parseSan(pos, node.san);
    if (!move) break;
    pos.play(move);
    list.push(makeFen(pos.toSetup()));
  }
  return list;
}

const files = fs.readdirSync(gamesDir).filter((f) => f.endsWith(".pgn")).sort();
const sample = [];
let sideline = [];
for (const f of files.slice(0, 8)) {
  const list = fens(fs.readFileSync(path.join(gamesDir, f), "utf8"));
  if (sideline.length === 0) sideline = list.slice(20, 36);
  for (const ply of [12, 24, 36, 48, 60]) {
    if (ply < list.length - 1) sample.push({ game: f, ply, fen: list[ply] });
  }
}
fs.writeFileSync(out, JSON.stringify({ sample, sideline }, null, 1));
console.log(`sample ${sample.length}, sideline ${sideline.length}`);
