// Builds the Chrome Web Store upload: `node pack.mjs` writes dist/openchess-extension-<version>.zip
// with only the files the extension needs (no tests, README or package.json). No dependencies.
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { deflateRawSync } from "node:zlib";

const root = new URL("./", import.meta.url);
const manifest = JSON.parse(readFileSync(new URL("manifest.json", root), "utf8"));
const icons = readdirSync(new URL("icons/", root))
  .filter((name) => name.endsWith(".png"))
  .map((name) => `icons/${name}`);
const files = [
  "manifest.json",
  "background.js",
  "content.js",
  "shared.js",
  "options.html",
  "options.js",
  "redirect.html",
  "redirect.js",
  "ui.css",
  ...icons,
];

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(buf) {
  let c = 0xffffffff;
  for (const byte of buf) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

const locals = [];
const centrals = [];
let offset = 0;
for (const name of files) {
  const data = readFileSync(new URL(name, root));
  const packed = deflateRawSync(data, { level: 9 });
  const nameBuf = Buffer.from(name, "utf8");
  const crc = crc32(data);
  // Fixed timestamp (1980-01-01) so the same sources always give the same zip.
  const head = Buffer.alloc(30);
  head.writeUInt32LE(0x04034b50, 0);
  head.writeUInt16LE(20, 4);
  head.writeUInt16LE(0x0800, 6); // UTF-8 names
  head.writeUInt16LE(8, 8); // deflate
  head.writeUInt16LE(0, 10);
  head.writeUInt16LE(0x21, 12);
  head.writeUInt32LE(crc, 14);
  head.writeUInt32LE(packed.length, 18);
  head.writeUInt32LE(data.length, 22);
  head.writeUInt16LE(nameBuf.length, 26);
  locals.push(head, nameBuf, packed);

  const central = Buffer.alloc(46);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4);
  central.writeUInt16LE(20, 6);
  central.writeUInt16LE(0x0800, 8);
  central.writeUInt16LE(8, 10);
  central.writeUInt16LE(0, 12);
  central.writeUInt16LE(0x21, 14);
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(packed.length, 20);
  central.writeUInt32LE(data.length, 24);
  central.writeUInt16LE(nameBuf.length, 28);
  central.writeUInt32LE(offset, 42);
  centrals.push(central, nameBuf);
  offset += head.length + nameBuf.length + packed.length;
}
const centralSize = centrals.reduce((n, b) => n + b.length, 0);
const end = Buffer.alloc(22);
end.writeUInt32LE(0x06054b50, 0);
end.writeUInt16LE(files.length, 8);
end.writeUInt16LE(files.length, 10);
end.writeUInt32LE(centralSize, 12);
end.writeUInt32LE(offset, 16);

mkdirSync(new URL("dist/", root), { recursive: true });
const out = new URL(`dist/openchess-extension-${manifest.version}.zip`, root);
writeFileSync(out, Buffer.concat([...locals, ...centrals, end]));
console.log(`Wrote ${out.pathname} (${files.length} files)`);
