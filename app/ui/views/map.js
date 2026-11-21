// Map: P points per uiMapID as glowing polylines over the map's art (when
// a `maps` folder holds it) on a dark canvas, with session colours, zoom
// and pan. A teleport on the map is a dashed jump between its two points.

import { invoke, el, num, esc, PALETTE, state } from "../lib.js";

export async function render(body) {
  body.innerHTML = "";
  const maps = state.store?.maps ?? [];

  const wrap = el("div", "map-wrap");
  const side = el("div", "map-side");
  const canvas = el("canvas");
  canvas.id = "map-canvas";
  wrap.append(side, canvas);
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
  // The map's art, darkened: {img, asp} with asp = height / width, or null.
  // The view is in map-percent space with y scaled by asp, so the art keeps
  // its shape and a point (x, y) sits at (x, y * asp).
  let art = null;

  // Art from the `maps` folder. A map without a file keeps the plain canvas.
  async function loadArt(map) {
    try {
      const raw = await invoke("map_image", { map });
      if (!raw.byteLength) return null;
      const bmp = await createImageBitmap(new Blob([raw]));
      const img = document.createElement("canvas");
      img.width = bmp.width;
      img.height = bmp.height;
      const c = img.getContext("2d");
      c.drawImage(bmp, 0, 0);
      const asp = bmp.height / bmp.width;
      bmp.close();
      // Mute the colours a little so the paths stay the brightest thing on the map.
      const px = c.getImageData(0, 0, img.width, img.height);
      const d = px.data;
      for (let i = 0; i < d.length; i += 4) {
        const l = 0.3 * d[i] + 0.59 * d[i + 1] + 0.11 * d[i + 2];
        for (let k = 0; k < 3; k++) d[i + k] = (l + (d[i + k] - l) * 0.8) * 0.9;
      }
      c.putImageData(px, 0, 0);
      // A bitmap stays on the GPU, so redraws while moving do not upload the pixels again.
      return { img: await createImageBitmap(img), asp };
    } catch {
      return null;
    }
  }

  function resize() {
    canvas.width = canvas.clientWidth * devicePixelRatio;
    canvas.height = canvas.clientHeight * devicePixelRatio;
    draw();
  }

  let token = 0;
  async function load() {
    const mine = ++token;
    const [got, found] = await Promise.all([
      invoke("map_data", { map: current, ch: state.ch }),
      loadArt(current),
    ]);
    if (mine !== token) return; // a newer map was picked meanwhile
    art = found;
    const asp = art ? art.asp : 1;
    polys = got.map((p) => ({ ...p, pts: p.pts.map(([x, y]) => [x, y * asp]) }));
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
