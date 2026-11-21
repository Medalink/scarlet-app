// node tools/openmap-maps.mjs <tables dir> [out file]
//
// Writes app/src/openmap_maps.json: which maps the app can cut out of OpenMap's world tiles, and where
// each one sits on the world map. The app embeds it, because OpenMap itself keeps no zone rectangles on
// this client (it asks the game's map tables at runtime) and a PC without the Scarlet maps folder has no
// other list of maps.
//
// <tables dir> holds the client's own tables exported as CSV with a header row:
//   UiMap, UiMapAssignment, UiMapXMapArt, UiMapArt, UiMapArtStyleLayer
// Only facts from those tables go in (ids, names, sizes, rectangles in world yards), no art.
//
// A map is listed (cities with `city`, which the tiles have no art for) when it has art, lies on Eastern Kingdoms or Kalimdor, and its art has the shape of its
// rectangle (a 512 x 512 map of a whole continent has not, so it is left out).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [, , tablesDir, outArg] = process.argv;
if (!tablesDir) {
  console.error("usage: node tools/openmap-maps.mjs <tables dir> [out file]");
  process.exit(2);
}
const out = outArg || path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "app", "src", "openmap_maps.json");

function table(name) {
  const text = fs.readFileSync(path.join(tablesDir, name + ".csv"), "utf8").replace(/^﻿/, "");
  const rows = [];
  let row = [], cell = "", quoted = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') { cell += '"'; i++; }
      else if (ch === '"') quoted = false;
      else cell += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ",") { row.push(cell); cell = ""; }
    else if (ch === "\n") { row.push(cell); rows.push(row); row = []; cell = ""; }
    else if (ch !== "\r") cell += ch;
  }
  if (cell || row.length) { row.push(cell); rows.push(row); }
  const head = rows.shift();
  return rows.filter((r) => r.length === head.length).map((r) => Object.fromEntries(head.map((h, i) => [h, r[i]])));
}

const uiMaps = new Map(table("UiMap").map((r) => [+r.ID, r]));
const artOf = new Map();
for (const r of table("UiMapXMapArt")) if (+r.PhaseID === 0 && !artOf.has(+r.UiMapID)) artOf.set(+r.UiMapID, +r.UiMapArtID);
const styleOf = new Map(table("UiMapArt").map((r) => [+r.ID, +r.UiMapArtStyleID]));
const layers = new Map(table("UiMapArtStyleLayer").filter((r) => +r.LayerIndex === 0).map((r) => [+r.UiMapArtStyleID, r]));

const rect = (r) => ({
  c: +r.MapID,
  min: [+r["UiMin[0]"], +r["UiMin[1]"]],
  max: [+r["UiMax[0]"], +r["UiMax[1]"]],
  r: [+r["Region[0]"], +r["Region[1]"], +r["Region[3]"], +r["Region[4]"]],
});
const assignments = new Map();
for (const r of table("UiMapAssignment")) {
  const list = assignments.get(+r.UiMapID) || [];
  list.push(r);
  assignments.set(+r.UiMapID, list);
}

const world = (assignments.get(947) || []).map(rect);
const size = (id) => {
  const l = layers.get(styleOf.get(artOf.get(id)));
  return l ? [+l.LayerWidth, +l.LayerHeight] : null;
};
const [ww, wh] = size(947) || [1002, 668];
const maps = [];
const left = [];
for (const [id, list] of assignments) {
  const whole = list.find((r) => +r["UiMin[0]"] === 0 && +r["UiMin[1]"] === 0 && +r["UiMax[0]"] === 1 && +r["UiMax[1]"] === 1);
  const dims = size(id);
  if (id === 947 || !whole || !dims || !uiMaps.has(id)) continue;
  const a = rect(whole);
  if (!world.some((w) => w.c === a.c)) continue;
  // The sheet's x runs along world y and its y along world x.
  const shape = (a.r[3] - a.r[1]) / (a.r[2] - a.r[0]);
  if (Math.abs(shape / (dims[0] / dims[1]) - 1) > 0.08) { left.push(`${id} ${uiMaps.get(id).Name_lang}`); continue; }
  const m = uiMaps.get(id);
  // A city is a zone-type map without the client's flag 2. OpenMap's tiles leave its art blank.
  const city = +m.Type === 3 && !(+m.Flags & 2);
  maps.push({ id, name: m.Name_lang, w: dims[0], h: dims[1], ...(city ? { city: true } : {}), ...a });
}
maps.unshift({ id: 947, name: uiMaps.get(947).Name_lang, w: ww, h: wh });
maps.sort((a, b) => (a.id === 947 ? -1 : b.id === 947 ? 1 : a.id - b.id));
fs.writeFileSync(out, JSON.stringify({ world: { w: ww, h: wh, rows: world }, maps }) + "\n");
console.log(`${maps.length} maps -> ${out}`);
if (left.length) console.log("left out (art not the shape of its rectangle):", left.join(", "));
