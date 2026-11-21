// Map: P points per uiMapID as glowing polylines over the map's art (when a `maps` folder
// holds it), shown as the game draws it or redrawn as a dark survey sheet, with zoom and pan.
// The game look colours the paths by session and draws a teleport as a dashed jump. The survey
// look draws the paths in one colour with place names and the layers the buttons switch.

import { invoke, el, num, esc, PALETTE, state } from "../lib.js";
import { styleMap, mergeLabels, drawLabels, drawLayers, drawMark, LAYERS } from "./mapstyle.js";

// Art by "<look>:<map id>", oldest first. One map is about 11 MB of GPU bitmap, so few are kept.
const styled = new Map();
const KEEP = 6;

// Which art the map shows: `game` the zone art as the game draws it, `survey` the dark sheet.
const LOOK_KEY = "scarlet.mapLook";
let look = "game";
try {
  if (localStorage.getItem(LOOK_KEY) === "survey") look = "survey";
} catch {}

// Which layers the survey look draws, by LAYERS key.
const LAYERS_KEY = "scarlet.mapLayers";
const layersOn = { names: true, travel: true, deaths: true, met: false, gather: true, people: false, quests: false, found: true, camps: true };
try {
  const saved = JSON.parse(localStorage.getItem(LAYERS_KEY) || "{}");
  for (const [k] of LAYERS) if (typeof saved[k] === "boolean") layersOn[k] = saved[k];
} catch {}
// The mark drawn on a layer's button; the others have none.
const SWATCH = { travel: "teleport", deaths: "death", gather: "gather", people: "npc", quests: "quest", found: "found", camps: "camp" };

// The art with its colours muted, the paths staying the brightest thing on the map.
function darken(c, px, img) {
  const d = px.data;
  for (let i = 0; i < d.length; i += 4) {
    const l = 0.3 * d[i] + 0.59 * d[i + 1] + 0.11 * d[i + 2];
    for (let k = 0; k < 3; k++) d[i + k] = (l + (d[i + k] - l) * 0.8) * 0.9;
  }
  c.putImageData(px, 0, 0);
  return img;
}

export async function render(body) {
  body.innerHTML = "";
  const maps = state.store?.maps ?? [];

  const wrap = el("div", "map-wrap");
  const side = el("div", "map-side");
  const canvas = el("canvas");
  canvas.id = "map-canvas";
  const stage = el("div", "map-stage");
  const seg = el("div", "seg map-look");
  seg.setAttribute("role", "group");
  seg.setAttribute("aria-label", "Map art");
  for (const [key, label] of [["game", "Game map"], ["survey", "Survey"]]) {
    const b = el("button", "", label);
    b.type = "button";
    b.dataset.look = key;
    b.setAttribute("aria-pressed", String(look === key));
    b.onclick = () => {
      if (look === key) return;
      look = key;
      try { localStorage.setItem(LOOK_KEY, key); } catch {}
      seg.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x.dataset.look === key)));
      toggles.hidden = look !== "survey";
      reloadArt();
    };
    seg.appendChild(b);
  }
  const toggles = el("div", "map-layers");
  toggles.setAttribute("role", "group");
  toggles.setAttribute("aria-label", "Map layers");
  toggles.hidden = look !== "survey";
  for (const [key, label] of LAYERS) {
    const b = el("button", "", label);
    b.type = "button";
    b.setAttribute("aria-pressed", String(layersOn[key]));
    if (SWATCH[key]) {
      const sw = el("canvas");
      sw.width = sw.height = 16 * devicePixelRatio;
      sw.style.width = sw.style.height = "16px";
      const c = sw.getContext("2d");
      c.scale(devicePixelRatio, devicePixelRatio);
      drawMark(c, SWATCH[key], 8, 8);
      b.prepend(sw);
    }
    b.onclick = () => {
      layersOn[key] = !layersOn[key];
      b.setAttribute("aria-pressed", String(layersOn[key]));
      try { localStorage.setItem(LAYERS_KEY, JSON.stringify(layersOn)); } catch {}
      draw();
    };
    toggles.appendChild(b);
  }
  stage.append(canvas, seg, toggles);
  wrap.append(side, stage);
  body.appendChild(wrap);

  if (!maps.length) {
    side.appendChild(el("div", "empty", "No path points yet."));
    return;
  }

  let current = maps[0].id;
  let view = null; // {x, y, w, h} in map-percent space

  for (const m of maps) {
    const item = el("div", "map-item" + (m.id === current ? " active" : ""));
    item.innerHTML = `<div>${esc(m.name)}</div><div class="pts">${num(m.points)} points</div>`;
    item.onclick = () => {
      current = m.id;
      side.querySelectorAll(".map-item").forEach((i) => i.classList.remove("active"));
      item.classList.add("active");
      view = null;
      load();
    };
    side.appendChild(item);
  }

  const ctx = canvas.getContext("2d");
  let polys = [];
  // The survey look's data for the map: the walked lines in the recorded unit (map coordinates
  // times 10000), the other layers, and the place names merged from the client's table and the
  // player's own lines.
  let walks = [], layers = {}, names = [];
  // The map's art, darkened: {img, asp} with asp = height / width, or null.
  // The view is in map-percent space with y scaled by asp, so the art keeps
  // its shape and a point (x, y) sits at (x, y * asp).
  let art = null;

  // Art from the `maps` folder, as the game draws it or redrawn as a survey sheet
  // (mapstyle.js). A map without a file keeps the plain canvas; if the restyle throws, the
  // art is darkened instead. Loaded art is kept (a few, newest last) so going back is instant.
  async function loadArt(map, want) {
    const key = want + ":" + map;
    const hit = styled.get(key);
    if (hit) {
      styled.delete(key);
      styled.set(key, hit);
      return hit;
    }
    try {
      const raw = await invoke("map_image", { map });
      if (!raw.byteLength) return null;
      const bmp = await createImageBitmap(new Blob([raw]));
      const img = document.createElement("canvas");
      img.width = bmp.width;
      img.height = bmp.height;
      const c = img.getContext("2d", { willReadFrequently: true });
      c.drawImage(bmp, 0, 0);
      const asp = bmp.height / bmp.width;
      bmp.close();
      if (want === "game") return keep(key, { img: await createImageBitmap(img), asp });
      const px = c.getImageData(0, 0, img.width, img.height);
      let out;
      try {
        const t0 = performance.now();
        out = styleMap(px.data, img.width, img.height);
        console.debug(`map ${map} styled in ${Math.round(performance.now() - t0)} ms`);
      } catch {
        out = darken(c, px, img);
      }
      // A bitmap stays on the GPU, so redraws while moving do not upload the pixels again.
      return keep(key, { img: await createImageBitmap(out), asp });
    } catch {
      return null;
    }
  }

  function keep(key, found) {
    styled.set(key, found);
    while (styled.size > KEEP) {
      const [old, gone] = styled.entries().next().value;
      gone.img.close();
      styled.delete(old);
    }
    return found;
  }

  // The other look on the same map: same paths and view, new art.
  async function reloadArt() {
    const mine = ++token;
    const found = await loadArt(current, look);
    if (mine !== token) return;
    art = found;
    draw();
  }

  function resize() {
    canvas.width = canvas.clientWidth * devicePixelRatio;
    canvas.height = canvas.clientHeight * devicePixelRatio;
    draw();
  }

  let token = 0;
  async function load() {
    const mine = ++token;
    const [got, found, extra, table] = await Promise.all([
      invoke("map_data", { map: current, ch: state.ch }),
      loadArt(current, look),
      invoke("map_layers", { map: current, ch: state.ch }).catch(() => null),
      invoke("map_labels", { map: current }).catch(() => []),
    ]);
    if (mine !== token) return; // a newer map was picked meanwhile
    art = found;
    const asp = art ? art.asp : 1;
    polys = got.map((p) => ({ ...p, pts: p.pts.map(([x, y]) => [x, y * asp]) }));
    walks = polys.filter((p) => !p.dash && p.pts.length > 1).map((p) => p.pts.map(([x, y]) => [x * 100, (y / asp) * 100]));
    layers = extra || {};
    names = mergeLabels(table, layers.places, layers.found);
    // Session index -> colour, stable across maps.
    const ids = [...new Set(polys.map((p) => p.session))];
    for (const p of polys) p.color = PALETTE[Math.max(0, ids.indexOf(p.session)) % PALETTE.length];
    fit();
    draw();
  }

  function fit() {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const p of polys)
      for (const [x, y] of p.pts) {
        x0 = Math.min(x0, x); y0 = Math.min(y0, y);
        x1 = Math.max(x1, x); y1 = Math.max(y1, y);
      }
    if (!isFinite(x0)) { view = { x: 0, y: 0, w: 100, h: 100 * (art ? art.asp : 1) }; return; }
    const pad = Math.max((x1 - x0), (y1 - y0)) * 0.06 + 0.5;
    x0 -= pad; y0 -= pad; x1 += pad; y1 += pad;
    // Square aspect: y axis of WoW maps is south -> down.
    const w = Math.max(x1 - x0, (y1 - y0) * (canvas.clientWidth / canvas.clientHeight || 1));
    const h = Math.max(y1 - y0, (x1 - x0) * (canvas.clientHeight / canvas.clientWidth || 1));
    view = { x: (x0 + x1 - w) / 2, y: (y0 + y1 - h) / 2, w, h };
  }

  // Redraws wait for the next frame, so a burst of wheel or mouse events draws once.
  // While zooming or dragging the paths draw without the blur glow (the costly part);
  // the full glow comes back once input stops for 150 ms.
  let frame = 0, moving = false, settle = 0;
  function schedule(interacting) {
    if (interacting) {
      moving = true;
      clearTimeout(settle);
      settle = setTimeout(() => { moving = false; schedule(false); }, 150);
    }
    if (!frame) frame = requestAnimationFrame(() => { frame = 0; draw(); });
  }

  function draw() {
    const W = canvas.width, H = canvas.height;
    ctx.clearRect(0, 0, W, H);
    if (!view) return;
    const sx = W / view.w, sy = H / view.h;
    const X = (x) => (x - view.x) * sx;
    const Y = (y) => (y - view.y) * sy;
    if (art) {
      ctx.imageSmoothingQuality = moving ? "low" : "high";
      ctx.drawImage(art.img, X(0), Y(0), X(100) - X(0), Y(100 * art.asp) - Y(0));
    }
    if (look === "survey" && art) {
      // Marks and lettering are in CSS pixels, so they keep their size at every zoom; the
      // recorded unit is map coordinates times 10000.
      const k = devicePixelRatio, cw = W / k, ch = H / k;
      const rx = (v) => X(v / 100) / k, ry = (v) => Y(((v / 100) * art.asp)) / k;
      ctx.save();
      ctx.scale(k, k);
      drawLayers(ctx, { ...layers, walks }, rx, ry, moving ? { ...layersOn, met: false } : layersOn, names);
      if (layersOn.names) drawLabels(ctx, names, rx, ry, [0, 0, cw, ch]);
      ctx.restore();
      return;
    }
    ctx.lineWidth = 2 * devicePixelRatio;
    ctx.lineCap = "round";
    for (const p of polys) {
      if (p.pts.length < 2) continue;
      // A teleport is a dashed jump between its two points.
      ctx.setLineDash(p.dash ? [8 * devicePixelRatio, 6 * devicePixelRatio] : []);
      ctx.strokeStyle = p.color;
      ctx.beginPath();
      ctx.moveTo(X(p.pts[0][0]), Y(p.pts[0][1]));
      for (const [x, y] of p.pts) ctx.lineTo(X(x), Y(y));
      if (!moving) {
        ctx.shadowColor = p.color;
        ctx.shadowBlur = 7 * devicePixelRatio;
        ctx.stroke();
        ctx.shadowBlur = 0;
      }
      // Crisp core over the glow.
      ctx.globalAlpha = moving ? 1 : 0.7;
      ctx.stroke();
      ctx.globalAlpha = 1;
    }
    ctx.setLineDash([]);
  }

  canvas.onwheel = (e) => {
    e.preventDefault();
    if (!view) return; // a map is still loading
    const r = canvas.getBoundingClientRect();
    const fx = (e.clientX - r.left) / r.width, fy = (e.clientY - r.top) / r.height;
    const k = e.deltaY > 0 ? 1.15 : 1 / 1.15;
    const mx = view.x + fx * view.w, my = view.y + fy * view.h;
    view = { x: mx - fx * view.w * k, y: my - fy * view.h * k, w: view.w * k, h: view.h * k };
    schedule(true);
  };

  let drag = null;
  canvas.onmousedown = (e) => (drag = [e.clientX, e.clientY]);
  window.onmousemove = (e) => {
    if (!drag || !view) return;
    const dx = ((e.clientX - drag[0]) / canvas.clientWidth) * view.w;
    const dy = ((e.clientY - drag[1]) / canvas.clientHeight) * view.h;
    view.x -= dx; view.y -= dy;
    drag = [e.clientX, e.clientY];
    schedule(true);
  };
  window.onmouseup = () => (drag = null);
  canvas.ondblclick = () => { fit(); draw(); };

  new ResizeObserver(resize).observe(canvas);
  resize();
  load();
}
