// node tools/map-labels.mjs <tables dir> <maps dir> [--ids 1413,1411]
//
// Writes <maps dir>/<uiMapID>.labels.json for every map in <maps dir>/index.json: the place names
// the game client itself ships for that map, with where each one sits on it. The Map view letters
// its survey sheet from this table, so a map is labelled before the player has walked it.
//
// <tables dir> holds the client's own tables exported as CSV with a header row:
//   UiMap, UiMapAssignment, UiMapXMapArt   which map is which, and the world rectangle it covers
//   WorldMapOverlay                        each explorable area's rectangle on the map art
//   AreaTable                              area names and which zone an area belongs to
//   AreaPOI                                towns and landmarks, at world positions
//   TaxiNodes                              flight points, at world positions
// A missing table only drops its kind of label.
//
// A label is { name, kind, x, y } with x and y in map coordinates times 10000, the unit of the
// recorded path points. kind: `area` an explorable area (with w and h, its size in the same unit),
// `town` a settlement, `spot` a landmark, `flight` a flight point, `zone` a zone on a continent map.
import fs from "node:fs";
import path from "node:path";

const [, , tablesDir, mapsDir, ...rest] = process.argv;
if (!tablesDir || !mapsDir) {
  console.error("usage: node tools/map-labels.mjs <tables dir> <maps dir> [--ids 1413,1411]");
  process.exit(2);
}
const only = rest[0] === "--ids" ? new Set(rest[1].split(",").map(Number)) : null;

function table(name) {
  const file = path.join(tablesDir, name + ".csv");
  if (!fs.existsSync(file)) { console.error(`no ${name}.csv: its labels are left out`); return []; }
  const text = fs.readFileSync(file, "utf8").replace(/^﻿/, "");
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

const index = JSON.parse(fs.readFileSync(path.join(mapsDir, "index.json"), "utf8")).maps;
const uiMaps = new Map(table("UiMap").map((r) => [+r.ID, r]));
const areas = new Map(table("AreaTable").map((r) => [+r.ID, { name: r.AreaName_lang, parent: +r.ParentAreaID }]));
// The zone an area belongs to: the top of its parent chain.
const zoneOf = (id) => { for (let n = 0; n < 8 && areas.get(id)?.parent; n++) id = areas.get(id).parent; return id; };
const areasByName = new Map();
for (const [id, a] of areas) if (!areasByName.has(a.name)) areasByName.set(a.name, id);

// One world rectangle per map: the first assignment that covers the whole sheet.
const world = new Map();
for (const r of table("UiMapAssignment")) {
  const id = +r.UiMapID;
  if (world.has(id) || +r["UiMin[0]"] !== 0 || +r["UiMin[1]"] !== 0 || +r["UiMax[0]"] !== 1 || +r["UiMax[1]"] !== 1) continue;
  world.set(id, { cont: +r.MapID, area: +r.AreaID, x0: +r["Region[0]"], y0: +r["Region[1]"], x1: +r["Region[3]"], y1: +r["Region[4]"] });
}
// World x runs north and y runs west; the sheet's x runs east and y south.
const place = (w, wx, wy) => [Math.round(((w.y1 - wy) / (w.y1 - w.y0)) * 10000), Math.round(((w.x1 - wx) / (w.x1 - w.x0)) * 10000)];
const onSheet = ([x, y]) => x > 150 && y > 150 && x < 9850 && y < 9850;

const artOf = new Map();
for (const r of table("UiMapXMapArt")) if (+r.PhaseID === 0 && !artOf.has(+r.UiMapID)) artOf.set(+r.UiMapID, +r.UiMapArtID);
const overlays = new Map();
for (const r of table("WorldMapOverlay")) {
  const list = overlays.get(+r.UiMapArtID) || [];
  list.push(r);
  overlays.set(+r.UiMapArtID, list);
}
const pois = table("AreaPOI"), taxis = table("TaxiNodes");

let written = 0, empty = [];
for (const m of index) {
  if (only && !only.has(m.id)) continue;
  const labels = new Map(); // by name; a later, better-placed source replaces an earlier one
  const w = world.get(m.id);

  // Explorable areas: the rectangle the client highlights when the pointer is over the area.
  for (const o of overlays.get(artOf.get(m.id)) || []) {
    let top = +o.HitRectTop, bottom = +o.HitRectBottom, left = +o.HitRectLeft, right = +o.HitRectRight;
    if (!(bottom > top && right > left)) { left = +o.OffsetX; top = +o.OffsetY; right = left + +o.TextureWidth; bottom = top + +o.TextureHeight; }
    for (const key of ["AreaID[0]", "AreaID[1]", "AreaID[2]", "AreaID[3]"]) {
      const name = areas.get(+o[key])?.name;
      if (!name || name === m.name || labels.has(name)) continue;
      labels.set(name, {
        name, kind: "area",
        x: Math.round(((left + right) / 2 / m.width) * 10000), y: Math.round(((top + bottom) / 2 / m.height) * 10000),
        w: Math.round(((right - left) / m.width) * 10000), h: Math.round(((bottom - top) / m.height) * 10000),
      });
    }
  }

  if (w && m.type === "zone") {
    // Towns and landmarks. A zone's rectangle overlaps its neighbours', so a point of interest
    // counts only when its area (or, lacking one, an area of the same name) is in this zone.
    for (const p of pois) {
      if (+p.ContinentID !== w.cont || !p.Name_lang) continue;
      const at = place(w, +p["Pos[0]"], +p["Pos[1]"]);
      if (!onSheet(at)) continue;
      const area = +p.AreaID > 0 ? +p.AreaID : areasByName.get(p.Name_lang);
      if (!area || zoneOf(area) !== w.area) continue;
      if (p.Name_lang === m.name) continue;
      const kind = +p.Importance >= 3 ? "town" : "spot", had = labels.get(p.Name_lang);
      labels.set(p.Name_lang, { ...(had || {}), name: p.Name_lang, kind, x: at[0], y: at[1] });
    }
    // Flight points are named "Place, Zone".
    for (const t of taxis) {
      if (+t.ContinentID !== w.cont) continue;
      const cut = t.Name_lang.lastIndexOf(", ");
      if (cut < 0 || t.Name_lang.slice(cut + 2) !== m.name) continue;
      const at = place(w, +t["Pos[0]"], +t["Pos[1]"]);
      if (!onSheet(at)) continue;
      // "Crossroads" the flight point and "The Crossroads" the town are one place.
      const name = t.Name_lang.slice(0, cut), had = labels.get(name) || labels.get("The " + name);
      // A town that has a flight point stays a town and is flagged; a lone flight point is its own label.
      if (had) had.flight = [at[0], at[1]];
      else labels.set(name, { name, kind: "flight", x: at[0], y: at[1], flight: [at[0], at[1]] });
    }
  } else if (w) {
    // A continent or world sheet is lettered with the zones on it.
    for (const z of index) {
      const zw = world.get(z.id);
      if (!zw || z.type !== "zone" || zw.cont !== w.cont || z.id === m.id) continue;
      const at = place(w, (zw.x0 + zw.x1) / 2, (zw.y0 + zw.y1) / 2);
      if (onSheet(at)) labels.set(z.name, { name: z.name, kind: "zone", x: at[0], y: at[1] });
    }
  }

  const list = [...labels.values()].sort((a, b) => a.name.localeCompare(b.name));
  const build = fs.readdirSync(path.join(tablesDir, "..")).includes("metadata.json") ? path.basename(path.resolve(tablesDir, "..")) : "";
  fs.writeFileSync(path.join(mapsDir, `${m.id}.labels.json`), JSON.stringify({ map: m.id, name: m.name, build, labels: list }, null, 1) + "\n");
  written++;
  if (!list.length) empty.push(`${m.id} ${m.name}`);
}
console.log(`${written} label tables written to ${mapsDir}`);
if (empty.length) console.log(`no names in the client's tables for: ${empty.join(", ")}`);
