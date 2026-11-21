// node tools/render-dark.mjs <art dir> <out dir> [ids|all] [--sea <plain art dir>] [--ink drawn|off|long|all|open]
//                            [--format webp|png] [--quality 0.8] [--width 2004] [--debug on]
//
// Draws the Survey look of every map in <art dir> (<uiMapID>.webp or .png files, 1002 x 668 for a zone)
// with the app's own sheet code (app/ui/views/mapstyle.js) in a headless Edge, and saves each sheet as
// <out dir>/<uiMapID>.<format>. This is how the dark sheets the app ships are made.
//
// <art dir> comes from `scarlet-app.exe --openmap-cut <art dir>`, which cuts the maps out of the OpenMap
// addon, so the sheets, the app and the website share one source and one coordinate space.
//
// --ink drawn (the default) adds the sheets' detail with tools/survey-detail.js: ranges, roads, the shore
// and a town's buildings and walls, each worked out from the art and drawn with our own marks. No stroke
// of the art is copied. --ink off leaves the detail out; long, all and open are the app's run-time looks,
// which lay the art's own strokes over the sheet and so are not for shipping.
//
// --sea <dir> is the same maps cut with `--openmap-cut <dir> --plain`. A zone's cut fades its neighbours
// to bare parchment and takes a hatched sea for one of them; water is read from the plain cut instead.
// A city sheet (the maps app/src/openmap_maps.json marks as cities) gets buildings and walls in place of
// ranges and roads. A continent or the world gets the shore only: its art is too small to measure.
//
// --width scales each sheet to that width (default: as drawn, twice the art's). The page keeps no copy of
// the art: it reads each file, draws it and posts the result to this script. --debug on also saves what
// the detail measured, as <uiMapID>-<field>.png. --style and --detail take JSON that overrides
// mapstyle's P and survey-detail's D, for tuning.
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
  console.error("usage: node tools/render-dark.mjs <art dir> <out dir> [ids|all] [--sea <plain art dir>] [--ink drawn|off|long|all|open] [--format webp|png] [--quality 0.8] [--width 2004] [--debug on]");
  process.exit(2);
}
const ink = flags.ink || "drawn";
const debug = flags.debug === "on";
const over = { ...JSON.parse(flags.style || "{}"), ...JSON.parse(flags.detail || "{}") };
const format = flags.format || "webp";
const quality = Number(flags.quality || 0.8);
const width = flags.width ? Number(flags.width) : 0;
const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
// The two modules the page loads, at the paths they import each other by.
const modules = { "/app/ui/views/mapstyle.js": "app/ui/views/mapstyle.js", "/tools/survey-detail.js": "tools/survey-detail.js" };
const edge = process.env.EDGE || "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe";

const list = (dir) => {
  const found = new Map();
  for (const f of fs.readdirSync(dir)) {
    const m = /^(\d+)\.(webp|png)$/.exec(f);
    if (m) found.set(Number(m[1]), path.join(dir, f));
  }
  return found;
};
const files = list(artDir), seas = flags.sea ? list(flags.sea) : new Map();
const ids = (which === "all" ? [...files.keys()] : which.split(",").map(Number)).filter((id) => files.has(id)).sort((a, b) => a - b);
if (!ids.length) {
  console.error("no maps to draw in " + artDir);
  process.exit(1);
}
fs.mkdirSync(outDir, { recursive: true });
const towns = JSON.parse(fs.readFileSync(path.join(root, "app/src/openmap_maps.json"), "utf8")).maps.filter((m) => m.city).map((m) => m.id);
const continents = [947, 1414, 1415, 1463, 1464];

const page = `<!doctype html><meta charset=utf-8><body><script type=module>
import { P, styleMap } from "/app/ui/views/mapstyle.js";
import { D, drawDetail } from "/tools/survey-detail.js";
const ids = ${JSON.stringify(ids)}, seas = ${JSON.stringify(ids.filter((id) => seas.has(id)))}, ink = ${JSON.stringify(ink)}, mime = ${JSON.stringify("image/" + format)}, q = ${quality}, width = ${width};
const towns = ${JSON.stringify(towns)}, continents = ${JSON.stringify(continents)};
const style = { ...P, ...D, ...${JSON.stringify(over)}, ink: ink === "drawn" ? "off" : ink, detail: ink === "drawn" ? drawDetail : null };
const log = (o) => fetch("/log", { method: "POST", body: JSON.stringify(o) });
window.onerror = (m, u, l) => log({ error: m + " line " + l });
window.onunhandledrejection = (e) => log({ error: String((e.reason && e.reason.stack) || e.reason) });
const pixels = async (url) => {
  const bmp = await createImageBitmap(await (await fetch(url)).blob());
  const c = document.createElement("canvas");
  c.width = bmp.width; c.height = bmp.height;
  const cx = c.getContext("2d", { willReadFrequently: true });
  cx.drawImage(bmp, 0, 0);
  return cx.getImageData(0, 0, c.width, c.height);
};
// What the detail measured, each field as a grey picture at half size.
const fields = async (id) => {
  const { w, h, fields } = drawDetail.last;
  if (drawDetail.tally) await log({ note: id + " buildings: " + JSON.stringify(drawDetail.tally) });
  for (const [name, a] of Object.entries(fields)) {
    if (!a) continue;
    let top = 1e-6;
    for (let i = 0; i < a.length; i++) if (a[i] > top) top = a[i];
    const g = document.createElement("canvas");
    g.width = w; g.height = h;
    const im = g.getContext("2d").createImageData(w, h);
    for (let i = 0; i < a.length; i++) { im.data[i * 4] = im.data[i * 4 + 1] = im.data[i * 4 + 2] = Math.min(255, (a[i] / top) * 255); im.data[i * 4 + 3] = 255; }
    g.getContext("2d").putImageData(im, 0, 0);
    await fetch("/field/" + id + "-" + name, { method: "POST", body: await new Promise((r) => g.toBlob(r, "image/png")) });
    await log({ note: id + " " + name + ": white is " + top.toFixed(3) });
  }
};
for (const id of ids) {
  const art = await pixels("/art/" + id), sea = seas.includes(id) ? await pixels("/sea/" + id) : null;
  if (sea && (sea.width !== art.width || sea.height !== art.height)) { await log({ error: id + ": the plain cut is another size" }); continue; }
  const t0 = performance.now();
  let out = styleMap(art.data, art.width, art.height, { ...style, town: towns.includes(id), ground: !continents.includes(id), sea: sea ? sea.data : null });
  const ms = Math.round(performance.now() - t0);
  if (${debug} && style.detail) await fields(id);
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
let total = 0, failed = false;
const srv = http.createServer((req, res) => {
  const u = decodeURIComponent(req.url);
  if (u === "/") return res.writeHead(200, { "content-type": "text/html" }).end(page);
  if (modules[u]) return res.writeHead(200, { "content-type": "text/javascript" }).end(fs.readFileSync(path.join(root, modules[u])));
  if (u.startsWith("/art/")) return res.writeHead(200).end(fs.readFileSync(files.get(Number(u.slice(5)))));
  if (u.startsWith("/sea/")) return res.writeHead(200).end(fs.readFileSync(seas.get(Number(u.slice(5)))));
  if (req.method === "POST") {
    const parts = [];
    req.on("data", (d) => parts.push(d));
    req.on("end", () => {
      const body = Buffer.concat(parts);
      const field = /^\/field\/(\d+-[A-Za-z]+)$/.exec(u);
      if (field) fs.writeFileSync(path.join(outDir, field[1] + ".png"), body);
      else if (u.startsWith("/save/")) fs.writeFileSync(path.join(outDir, `${Number(u.slice(6))}.${format}`), body);
      else if (u === "/log") {
        const o = JSON.parse(body.toString());
        if (o.bytes) total += o.bytes;
        if (o.error) failed = true;
        console.log(o.error ? "ERROR " + o.error : o.note || `${o.id}: ${o.ms} ms, ${o.w}x${o.h}, ${o.bytes} bytes`);
      } else if (u === "/done") {
        console.log(`${ids.length} sheets, ${total} bytes`);
        finish(failed ? 1 : 0);
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
