// node tools/render-dark.mjs <art dir> <out dir> [ids|all] [--ink off|long|all|open] [--format webp|png] [--quality 0.8] [--width 2004]
//
// Draws the Survey look of every map in <art dir> (<uiMapID>.webp or .png files, 1002 x 668 for a zone)
// with the app's own sheet code (app/ui/views/mapstyle.js) in a headless Edge, and saves each sheet as
// <out dir>/<uiMapID>.<format>. This is how the dark sheets the app ships in `dark\` are made; the Map
// view draws a sheet the same way at run time when the folder has none.
//
// <art dir> comes from `scarlet-app.exe --openmap-cut <art dir>`, which cuts the maps out of the OpenMap
// addon, so the sheets, the app and the website share one source and one coordinate space. --ink off
// (the default here) leaves out the long strokes the sheet otherwise copies from the art; the sheet is
// then drawn from the art's colour and texture alone.
//
// --width scales each sheet to that width (default: as drawn, twice the art's). The page keeps no copy of
// the art: it reads each file, draws it and posts the result to this script.
import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const args = process.argv.slice(2);
const flags = {};
const rest = [];
for (let i = 0; i < args.length; i++) {
  if (args[i].startsWith("--")) flags[args[i].slice(2)] = args[++i];
  else rest.push(args[i]);
}
const [artDir, outDir, which = "all"] = rest;
if (!artDir || !outDir) {
  console.error("usage: node tools/render-dark.mjs <art dir> <out dir> [ids|all] [--ink off|long|all|open] [--format webp|png] [--quality 0.8] [--width 2004]");
  process.exit(2);
}
const ink = flags.ink || "off";
const format = flags.format || "webp";
const quality = Number(flags.quality || 0.8);
const width = flags.width ? Number(flags.width) : 0;
const styleJs = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "app", "ui", "views", "mapstyle.js");
const edge = process.env.EDGE || "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";

const files = new Map();
for (const f of fs.readdirSync(artDir)) {
  const m = /^(\d+)\.(webp|png)$/.exec(f);
  if (m) files.set(Number(m[1]), f);
}
const ids = (which === "all" ? [...files.keys()] : which.split(",").map(Number)).filter((id) => files.has(id)).sort((a, b) => a - b);
if (!ids.length) {
  console.error("no maps to draw in " + artDir);
  process.exit(1);
}
fs.mkdirSync(outDir, { recursive: true });

const page = `<!doctype html><meta charset=utf-8><body><script type=module>
import { P, styleMap } from "/style.js";
const ids = ${JSON.stringify(ids)}, ink = ${JSON.stringify(ink)}, mime = ${JSON.stringify("image/" + format)}, q = ${quality}, width = ${width};
const log = (o) => fetch("/log", { method: "POST", body: JSON.stringify(o) });
window.onerror = (m, u, l) => log({ error: m + " line " + l });
for (const id of ids) {
  const bmp = await createImageBitmap(await (await fetch("/art/" + id)).blob());
  const c = document.createElement("canvas");
  c.width = bmp.width; c.height = bmp.height;
  const cx = c.getContext("2d", { willReadFrequently: true });
  cx.drawImage(bmp, 0, 0);
  const t0 = performance.now();
  let out = styleMap(cx.getImageData(0, 0, c.width, c.height).data, c.width, c.height, { ...P, ink });
  const ms = Math.round(performance.now() - t0);
  if (width && width !== out.width) {
    const s = document.createElement("canvas");
    s.width = width; s.height = Math.round(out.height * width / out.width);
    const sc = s.getContext("2d");
    sc.imageSmoothingQuality = "high";
    sc.drawImage(out, 0, 0, s.width, s.height);
    out = s;
  }
  const blob = await new Promise((r) => out.toBlob(r, mime, q));
  await fetch("/save/" + id, { method: "POST", body: blob });
  await log({ id, ms, w: out.width, h: out.height, bytes: blob.size });
}
await fetch("/done", { method: "POST" });
</script>`;

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "render-dark-"));
let total = 0;
const srv = http.createServer((req, res) => {
  const u = decodeURIComponent(req.url);
  if (u === "/") return res.writeHead(200, { "content-type": "text/html" }).end(page);
  if (u === "/style.js") return res.writeHead(200, { "content-type": "text/javascript" }).end(fs.readFileSync(styleJs));
  if (u.startsWith("/art/")) return res.writeHead(200).end(fs.readFileSync(path.join(artDir, files.get(Number(u.slice(5))))));
  if (req.method === "POST") {
    const parts = [];
    req.on("data", (d) => parts.push(d));
    req.on("end", () => {
      const body = Buffer.concat(parts);
      if (u.startsWith("/save/")) fs.writeFileSync(path.join(outDir, `${Number(u.slice(6))}.${format}`), body);
      else if (u === "/log") {
        const o = JSON.parse(body.toString());
        if (o.bytes) total += o.bytes;
        console.log(o.error ? "ERROR " + o.error : `${o.id}: ${o.ms} ms, ${o.w}x${o.h}, ${o.bytes} bytes`);
      } else if (u === "/done") {
        console.log(`${ids.length} sheets, ${total} bytes`);
        finish(0);
      }
      res.writeHead(200).end("ok");
    });
    return;
  }
  res.writeHead(404).end();
});
let child;
function finish(code) {
  child?.kill();
  srv.close();
  setTimeout(() => {
    fs.rmSync(tmp, { recursive: true, force: true });
    process.exit(code);
  }, 500);
}
srv.listen(0, "127.0.0.1", () => {
  const url = `http://127.0.0.1:${srv.address().port}/`;
  child = spawn(edge, ["--headless=new", "--disable-gpu", `--user-data-dir=${tmp}`, url], { stdio: "ignore" });
  setTimeout(() => { console.error("timed out"); finish(1); }, 20 * 60 * 1000);
});
