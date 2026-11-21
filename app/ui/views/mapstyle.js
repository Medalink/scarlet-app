// The Map view's zone art restyled as a dark survey sheet, the way the website draws its maps.
// The art is parchment: painted mountains, roads, rivers, lettering and a hatched sea. Where the
// painted mountains are dense the terrain rises, and contour lines, hill shading and the coast
// are drawn over a dark sheet with the sea darker and bluer. Roads, rivers and lettering stay as
// pale hairlines, so a zone is still recognisable. Takes the art's RGBA pixels (w x h) and
// returns a canvas twice as large; about 150 ms for a 1002 x 668 map.

const C = { bg: [20, 20, 22], hi: [40, 40, 46], sea: [13, 13, 15], water: [17, 25, 30] };
const S = 2; // output scale over the source art

function boxBlur(src, w, h, r) {
  const tmp = new Float32Array(w * h), out = new Float32Array(w * h);
  const n = 2 * r + 1;
  for (let y = 0; y < h; y++) {
    const o = y * w;
    let acc = src[o] * (r + 1);
    for (let i = 1; i <= r; i++) acc += src[o + Math.min(i, w - 1)];
    for (let x = 0; x < w; x++) {
      tmp[o + x] = acc / n;
      acc += src[o + Math.min(x + r + 1, w - 1)] - src[o + Math.max(x - r, 0)];
    }
  }
  for (let x = 0; x < w; x++) {
    let acc = tmp[x] * (r + 1);
    for (let i = 1; i <= r; i++) acc += tmp[Math.min(i, h - 1) * w + x];
    for (let y = 0; y < h; y++) {
      out[y * w + x] = acc / n;
      acc += tmp[Math.min(y + r + 1, h - 1) * w + x] - tmp[Math.max(y - r, 0) * w + x];
    }
  }
  return out;
}
function gauss(src, w, h, sigma) {
  const r = Math.max(1, Math.round((Math.sqrt(4 * sigma * sigma + 1) - 1) / 2));
  return boxBlur(boxBlur(boxBlur(src, w, h, r), w, h, r), w, h, r);
}
const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
const smooth = (e0, e1, v) => { const t = clamp((v - e0) / (e1 - e0)); return t * t * (3 - 2 * t); };

// Per-pixel features of the art.
function features(px, w, h) {
  const n = w * h, lum = new Float32Array(n), water = new Float32Array(n), sat = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const r = px[i * 4], g = px[i * 4 + 1], b = px[i * 4 + 2];
    lum[i] = (0.3 * r + 0.59 * g + 0.11 * b) / 255;
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
    sat[i] = mx ? (mx - mn) / mx : 0;
    water[i] = smooth(-46, -20, b - (r + g) / 2);
  }
  // The sheet's edge: torn paper and corner ornaments fade into the page.
  const edge = new Float32Array(n);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const d = Math.min(x, y, w - 1 - x, h - 1 - y);
      const cx = Math.min(x, w - 1 - x), cy = Math.min(y, h - 1 - y);
      const dc = Math.hypot(Math.max(0, 60 - cx), Math.max(0, 50 - cy));
      edge[y * w + x] = Math.min(smooth(9, 22, d), 1 - smooth(0, 34, dc));
    }
  return { lum, water, sat, edge, w, h };
}

// Marching squares over a field g (cols x rows, spacing `step` output px).
function iso(ctx, g, cols, rows, step, level) {
  ctx.beginPath();
  for (let j = 0; j < rows - 1; j++) {
    for (let i = 0; i < cols - 1; i++) {
      const a = g[j * cols + i], b = g[j * cols + i + 1], c = g[(j + 1) * cols + i + 1], d = g[(j + 1) * cols + i];
      const idx = (a > level ? 8 : 0) | (b > level ? 4 : 0) | (c > level ? 2 : 0) | (d > level ? 1 : 0);
      if (idx === 0 || idx === 15) continue;
      const x = i * step, y = j * step;
      const T = [x + (step * (level - a)) / (b - a), y], R = [x + step, y + (step * (level - b)) / (c - b)];
      const B = [x + (step * (level - d)) / (c - d), y + step], L = [x, y + (step * (level - a)) / (d - a)];
      const seg = (p, q) => { ctx.moveTo(p[0], p[1]); ctx.lineTo(q[0], q[1]); };
      if (idx === 1 || idx === 14) seg(L, B);
      else if (idx === 2 || idx === 13) seg(B, R);
      else if (idx === 3 || idx === 12) seg(L, R);
      else if (idx === 4 || idx === 11) seg(T, R);
      else if (idx === 6 || idx === 9) seg(T, B);
      else if (idx === 7 || idx === 8) seg(L, T);
      else if (idx === 5) { seg(L, T); seg(B, R); }
      else { seg(T, R); seg(L, B); }
    }
  }
  ctx.stroke();
}

// Sub-sample a field to every k-th pixel.
function sub(f, w, h, k) {
  const cols = Math.ceil(w / k) + 1, rows = Math.ceil(h / k) + 1, g = new Float32Array(cols * rows);
  for (let j = 0; j < rows; j++) for (let i = 0; i < cols; i++) g[j * cols + i] = f[Math.min(j * k, h - 1) * w + Math.min(i * k, w - 1)];
  return { g, cols, rows };
}

function percentile(a, p, mask) {
  const hist = new Uint32Array(1024); let n = 0;
  for (let i = 0; i < a.length; i++) if (!mask || mask[i]) { hist[Math.min(1023, (clamp(a[i]) * 1023) | 0)]++; n++; }
  let acc = 0;
  for (let i = 0; i < 1024; i++) { acc += hist[i]; if (acc >= n * p) return i / 1023; }
  return 1;
}

function shadeOf(H, w, h, gain) {
  const out = new Float32Array(w * h);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const k = y * w + x;
      const dx = H[y * w + Math.min(x + 2, w - 1)] - H[y * w + Math.max(x - 2, 0)];
      const dy = H[Math.min(y + 2, h - 1) * w + x] - H[Math.max(y - 2, 0) * w + x];
      out[k] = clamp(-(dx + dy) * gain, -1, 1);
    }
  return out;
}

function toCanvas(rgb, w, h) {
  const c = document.createElement("canvas"); c.width = w; c.height = h;
  const cx = c.getContext("2d"), im = cx.createImageData(w, h);
  im.data.set(rgb); cx.putImageData(im, 0, 0);
  return c;
}

function base(px, w, h) {
  const f = features(px, w, h);
  // wc: water with its rivers, cleaned of the sea's hatching, for the coast line and the water tone.
  const wc = gauss(f.water, w, h, 2.2);
  for (let i = 0; i < wc.length; i++) wc[i] = smooth(0.38, 0.62, wc[i]);
  return { f, wc };
}

function makeCanvas(w, h) {
  const out = document.createElement("canvas");
  out.width = w * S; out.height = h * S;
  const ctx = out.getContext("2d");
  ctx.imageSmoothingQuality = "high";
  return { out, ctx };
}

const INSET = 10;
function clipSheet(ctx, w, h) {
  ctx.beginPath(); ctx.rect(INSET, INSET, w - 2 * INSET, h - 2 * INSET); ctx.clip();
}
function coast(ctx, wc, w, h, alpha) {
  const { g, cols, rows } = sub(wc, w, h, 1);
  ctx.save(); ctx.scale(S, S); clipSheet(ctx, w, h);
  ctx.lineCap = "round"; ctx.lineJoin = "round";
  ctx.strokeStyle = `rgba(255,255,255,${alpha})`; ctx.lineWidth = 0.9;
  iso(ctx, g, cols, rows, 1, 0.5);
  ctx.restore();
}

// Hills where the art paints mountains: a shore ramp plus the local contrast of the art make
// the height field. Its contours, a soft hill shade and the coast follow the site's survey sheet.
function survey(px, w, h, o) {
  const { f, wc } = base(px, w, h), n = w * h;
  const lb = gauss(f.lum, w, h, 2.2), hp = new Float32Array(n);
  for (let i = 0; i < n; i++) hp[i] = Math.abs(f.lum[i] - lb[i]) * (1 - wc[i]);
  const energy = gauss(hp, w, h, 6);
  const landMask = new Uint8Array(n);
  for (let i = 0; i < n; i++) landMask[i] = wc[i] < 0.3 ? 1 : 0;
  const top = percentile(energy, 0.97, landMask) || 0.05;
  const shoreW = gauss(f.water, w, h, 9);
  const zone = gauss(f.sat, w, h, 14);
  const zlo = percentile(zone, 0.2, landMask), zhi = percentile(zone, 0.85, landMask);
  const H = new Float32Array(n), Z = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const z = smooth(zlo, zhi, zone[i]); Z[i] = z;
    const hills = clamp(energy[i] / top) * (0.5 + 0.5 * z) * f.edge[i];
    const ramp = (0.5 - shoreW[i]) * 0.7;
    H[i] = ramp < 0 ? ramp : Math.min(ramp, 0.3) + hills * 1.15 * smooth(0, 0.12, ramp);
  }
  const Hs = gauss(H, w, h, 2.5);
  const shade = shadeOf(Hs, w, h, 11);
  // Thin dark marks: narrower than about four pixels.
  const fine = gauss(f.lum, w, h, 1), wide = gauss(f.lum, w, h, 4), line = new Float32Array(n);
  for (let i = 0; i < n; i++) line[i] = clamp((wide[i] - fine[i] - o.lineFloor) * o.lineGain) * (1 - wc[i]) * f.edge[i];
  const rgb = new Uint8ClampedArray(n * 4);
  for (let i = 0; i < n; i++) {
    const t = clamp(Hs[i] / 1.1), zt = 0.55 + 0.45 * Z[i], land = (1 - wc[i]) * f.edge[i];
    for (let c = 0; c < 3; c++) {
      const v = C.bg[c] + (C.hi[c] - C.bg[c]) * t * zt * o.lift + shade[i] * 7 * zt * o.lift;
      const wet = C.sea[c] + (C.water[c] - C.sea[c]) * wc[i];
      rgb[i * 4 + c] = wet + (v - wet) * land;
    }
    rgb[i * 4 + 3] = 255;
  }
  const { out, ctx } = makeCanvas(w, h);
  ctx.drawImage(toCanvas(rgb, w, h), 0, 0, w * S, h * S);
  const k = 2, { g, cols, rows } = sub(Hs, w, h, k);
  ctx.save(); ctx.scale(S, S); clipSheet(ctx, w, h); ctx.lineCap = "round"; ctx.lineWidth = 0.9;
  ctx.strokeStyle = "rgba(255,255,255,0.045)";
  for (const l of [-0.07, -0.17, -0.3]) iso(ctx, g, cols, rows, k, l);
  for (let n2 = 1; n2 <= 22; n2++) { ctx.strokeStyle = n2 % 5 === 0 ? "rgba(255,255,255,0.13)" : "rgba(255,255,255,0.07)"; iso(ctx, g, cols, rows, k, n2 * 0.085); }
  ctx.restore();
  const lw = new Uint8ClampedArray(n * 4);
  for (let i = 0; i < n; i++) { lw[i * 4] = lw[i * 4 + 1] = lw[i * 4 + 2] = 240; lw[i * 4 + 3] = line[i] * o.lineAlpha; }
  ctx.drawImage(toCanvas(lw, w, h), 0, 0, w * S, h * S);
  coast(ctx, wc, w, h, 0.26);
  return out;
}

// The survey look: hills lifted, the art's thin marks kept as hairlines.
export function styleMap(px, w, h) {
  return survey(px, w, h, { lift: 1.5, lineFloor: 0.02, lineGain: 4, lineAlpha: 125 });
}
