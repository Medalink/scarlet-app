// The Map view's Survey look: a zone's painted map redrawn as a dark survey sheet, and what is drawn
// over it (names, the walked lines, the layers the view switches). One set of rules for every sheet;
// nothing here knows which map it is drawing.
//
// The painting is read once at full size for four per-pixel cues, which are pooled to half size:
//   water colour  the blue-grey wash (cool and not bright)
//   hatching      the sea's horizontal pen lines, which mark water where the wash has faded
//   texture       fine pen strokes: painted mountains, and in a city its buildings
//   colour        how far the ground's colour is from the bare parchment round the sheet's rim
// Everything after that (blurs, masks, tracing, the height field, contours) runs at half size.
//
// styleMap(px, w, h) takes the art's RGBA pixels and returns a canvas S times as large; the time
// each step took is left in styleMap.last.

export const C = { bg: [20, 20, 22], hi: [38, 38, 44], sea: [14, 14, 16], accent: "#ff2d55", soft: "#ff6b8e", fg: "#f0f1f3" };
export const S = 2;

export const P = {
  toneGain: 1,
  minor: 0.07, major: 0.13, coast: 0.26, depth: 0.045,  // line alphas, the sheet's
  interval: 0.085, levels: 22,
  mountain: 0.95,     // height of the most heavily drawn range
  zoneLift: 0.17,     // ground coloured unlike the parchment sits this much higher
  roll: 1.8,          // low rolling relief from the parchment's mottling
  smooth: 2.5,        // blur of the height field, in half-size pixels
  range: 6.5,         // how far pen strokes spread into a massif, in half-size pixels
  landTone: 0.1, zoneTone: 0.3,
  tcap: 16, idark: 30, ilo: 0.04, ihi: 0.16, tlo: 3, thi: 8,  // texture (mean stroke contrast, 0..255) that starts and tops a range
  hlo: 7, hhi: 11, // hatching strength that counts as sea
  strip: 60,          // half-size pixels from the sheet's edge
  wbody: 3,           // water narrower than twice this (half-size pixels) is a river line
  river: 0.5,
  line: 1,
  // The art's own pen work laid back over the sheet in the line colour.
  ink: "long",        // "off", "all" every dark stroke, "long" strokes that run on (roads, walls, shores), "open" long strokes off the ranges
  inkLo: 8, inkHi: 28, inkLong: 34, inkAlpha: 0.22,
};

const clamp = (v, a = 0, b = 1) => (v < a ? a : v > b ? b : v);
const sstep = (e0, e1, v) => { const t = clamp((v - e0) / (e1 - e0)); return t * t * (3 - 2 * t); };

function boxBlur(src, w, h, r, tmp, out) {
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
// Three box blurs make a close Gaussian.
function gauss(src, w, h, sigma) {
  const r = Math.max(1, Math.round((Math.sqrt(4 * sigma * sigma + 1) - 1) / 2));
  const tmp = new Float32Array(w * h), a = new Float32Array(w * h), b = new Float32Array(w * h);
  boxBlur(src, w, h, r, tmp, a); boxBlur(a, w, h, r, tmp, b); boxBlur(b, w, h, r, tmp, a);
  return a;
}
function shrink(src, w, h, k) {
  const W = Math.ceil(w / k), H = Math.ceil(h / k), out = new Float32Array(W * H);
  for (let j = 0; j < H; j++)
    for (let i = 0; i < W; i++) {
      let acc = 0, n = 0;
      for (let y = j * k; y < Math.min(h, j * k + k); y++) for (let x = i * k; x < Math.min(w, i * k + k); x++) { acc += src[y * w + x]; n++; }
      out[j * W + i] = acc / n;
    }
  return { a: out, W, H };
}
function grow(s, w, h, k) {
  const out = new Float32Array(w * h), { a, W, H } = s;
  for (let y = 0; y < h; y++) {
    const fy = clamp((y + 0.5) / k - 0.5, 0, H - 1), j = Math.min(H - 2, fy | 0), v = fy - j;
    for (let x = 0; x < w; x++) {
      const fx = clamp((x + 0.5) / k - 0.5, 0, W - 1), i = Math.min(W - 2, fx | 0), u = fx - i;
      const p = j * W + i;
      out[y * w + x] = (a[p] * (1 - u) + a[p + 1] * u) * (1 - v) + (a[p + W] * (1 - u) + a[p + W + 1] * u) * v;
    }
  }
  return out;
}
// A wide blur, run on a copy k times smaller.
function wide(src, w, h, sigma, k) {
  const s = shrink(src, w, h, k);
  s.a = gauss(s.a, s.W, s.H, sigma / k);
  return grow(s, w, h, k);
}

// The p-th percentile of a over the pixels where mask is set, by histogram.
function percentile(a, p, mask) {
  const hist = new Uint32Array(1024); let n = 0, lo = Infinity, hi = -Infinity;
  for (let i = 0; i < a.length; i++) if (!mask || mask[i]) { if (a[i] < lo) lo = a[i]; if (a[i] > hi) hi = a[i]; }
  if (!(hi > lo)) return hi > -Infinity ? hi : 0;
  const span = hi - lo;
  for (let i = 0; i < a.length; i++) if (!mask || mask[i]) { hist[Math.min(1023, (((a[i] - lo) / span) * 1023) | 0)]++; n++; }
  let acc = 0;
  for (let i = 0; i < 1024; i++) { acc += hist[i]; if (acc >= n * p) return lo + (i / 1023) * span; }
  return hi;
}

// Marching squares over g for many levels in one pass; each level's segments go to paths[group(level index)].
function isoMany(g, cols, rows, step, levels, groupOf, paths) {
  const L = levels.length;
  for (let j = 0; j < rows - 1; j++) {
    for (let i = 0; i < cols - 1; i++) {
      const p = j * cols + i, a = g[p], b = g[p + 1], c = g[p + cols + 1], d = g[p + cols];
      const lo = Math.min(a, b, c, d), hi = Math.max(a, b, c, d);
      for (let q = 0; q < L; q++) {
        const level = levels[q];
        if (level < lo || level >= hi) continue;
        const idx = (a > level ? 8 : 0) | (b > level ? 4 : 0) | (c > level ? 2 : 0) | (d > level ? 1 : 0);
        if (idx === 0 || idx === 15) continue;
        const ctx = paths[groupOf[q]], x = i * step, y = j * step;
        const Tx = x + (step * (level - a)) / (b - a), Ry = y + (step * (level - b)) / (c - b);
        const Bx = x + (step * (level - d)) / (c - d), Ly = y + (step * (level - a)) / (d - a);
        const R = x + step, Bt = y + step;
        if (idx === 1 || idx === 14) { ctx.moveTo(x, Ly); ctx.lineTo(Bx, Bt); }
        else if (idx === 2 || idx === 13) { ctx.moveTo(Bx, Bt); ctx.lineTo(R, Ry); }
        else if (idx === 3 || idx === 12) { ctx.moveTo(x, Ly); ctx.lineTo(R, Ry); }
        else if (idx === 4 || idx === 11) { ctx.moveTo(Tx, y); ctx.lineTo(R, Ry); }
        else if (idx === 6 || idx === 9) { ctx.moveTo(Tx, y); ctx.lineTo(Bx, Bt); }
        else if (idx === 7 || idx === 8) { ctx.moveTo(x, Ly); ctx.lineTo(Tx, y); }
        else if (idx === 5) { ctx.moveTo(x, Ly); ctx.lineTo(Tx, y); ctx.moveTo(Bx, Bt); ctx.lineTo(R, Ry); }
        else { ctx.moveTo(Tx, y); ctx.lineTo(R, Ry); ctx.moveTo(x, Ly); ctx.lineTo(Bx, Bt); }
      }
    }
  }
}

// Keeps the connected pieces of a mask that span at least `min` pixels: drops letters, icons, specks.
function keepLong(mask, w, h, min) {
  const out = new Uint8Array(w * h), seen = new Uint8Array(w * h), stack = new Int32Array(w * h), members = new Int32Array(w * h);
  for (let s = 0; s < w * h; s++) {
    if (!mask[s] || seen[s]) continue;
    let top = 0, x0 = w, x1 = 0, y0 = h, y1 = 0, n = 0;
    stack[top++] = s; seen[s] = 1;
    while (top) {
      const p = stack[--top], x = p % w, y = (p / w) | 0;
      members[n++] = p;
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        const X = x + dx, Y = y + dy;
        if (X < 0 || Y < 0 || X >= w || Y >= h) continue;
        const q = Y * w + X;
        if (mask[q] && !seen[q]) { seen[q] = 1; stack[top++] = q; }
      }
    }
    if (Math.max(x1 - x0, y1 - y0) >= min) for (let i = 0; i < n; i++) out[members[i]] = 1;
  }
  return out;
}

function toCanvas(rgba, w, h) {
  const c = document.createElement("canvas"); c.width = w; c.height = h;
  const cx = c.getContext("2d"), im = cx.createImageData(w, h);
  im.data.set(rgba); cx.putImageData(im, 0, 0);
  return c;
}

// Square erosion (grow = 0) or dilation (grow = 1) of a mask by r pixels, by running counts.
function morph(m, w, h, r, grow) {
  const t = new Uint8Array(w * h), out = new Uint8Array(w * h), run = new Int32Array(Math.max(w, h) + 1);
  const pass = (src, dst, len, lines, at) => {
    for (let l = 0; l < lines; l++) {
      for (let i = 0; i < len; i++) run[i + 1] = run[i] + src[at(l, i)];
      for (let i = 0; i < len; i++) {
        const a = Math.max(0, i - r), b = Math.min(len - 1, i + r), c = run[b + 1] - run[a];
        dst[at(l, i)] = grow ? (c > 0 ? 1 : 0) : (c === b - a + 1 ? 1 : 0);
      }
    }
  };
  pass(m, t, w, h, (y, x) => y * w + x);
  pass(t, out, h, w, (x, y) => y * w + x);
  return out;
}

// Zhang-Suen thinning of a mask to one-pixel lines, then the lines as smoothed polylines.
function trace(mask, w, h, minLen) {
  let live = [];
  for (let i = 0; i < w * h; i++) if (mask[i]) { const x = i % w, y = (i / w) | 0; if (x > 0 && y > 0 && x < w - 1 && y < h - 1) live.push(i); else mask[i] = 0; }
  const N = [-w, -w + 1, 1, w + 1, w, w - 1, -1, -w - 1];
  const v = [0, 0, 0, 0, 0, 0, 0, 0];
  for (let pass = 0, changed = true; changed && pass < 24; pass++) {
    changed = false;
    for (let step = 0; step < 2; step++) {
      const gone = [];
      for (const p of live) {
        if (!mask[p]) continue;
        let n = 0, t = 0;
        for (let k = 0; k < 8; k++) { v[k] = mask[p + N[k]]; n += v[k]; }
        if (n < 2 || n > 6) continue;
        for (let k = 0; k < 8; k++) if (!v[k] && v[(k + 1) & 7]) t++;
        if (t !== 1) continue;
        if (step === 0 ? v[0] * v[2] * v[4] || v[2] * v[4] * v[6] : v[0] * v[2] * v[6] || v[0] * v[4] * v[6]) continue;
        gone.push(p);
      }
      for (const p of gone) mask[p] = 0;
      if (gone.length) changed = true;
    }
    live = live.filter((p) => mask[p]);
  }
  const seen = new Uint8Array(w * h), lines = [];
  const deg = (p) => { let n = 0; for (let k = 0; k < 8; k++) n += mask[p + N[k]]; return n; };
  const ORDER = [0, 2, 4, 6, 1, 3, 5, 7];
  const walk = (start) => {
    const pts = [];
    // A branch starts on the line it leaves, so junctions stay joined.
    for (const k of ORDER) { const q = start + N[k]; if (mask[q] && seen[q]) { pts.push(q); break; } }
    let p = start;
    for (;;) {
      seen[p] = 1; pts.push(p);
      let next = -1;
      for (const k of ORDER) { const q = p + N[k]; if (mask[q] && !seen[q]) { next = q; break; } }
      if (next < 0) {
        for (const k of ORDER) { const q = p + N[k]; if (mask[q] && seen[q] && pts.length > 4 && pts.indexOf(q, pts.length - 5) < 0) { pts.push(q); break; } }
        break;
      }
      p = next;
    }
    if (pts.length >= minLen) lines.push(pts.map((q) => [q % w, (q / w) | 0]));
  };
  for (const p of live) if (!seen[p] && deg(p) === 1) walk(p);
  for (const p of live) if (!seen[p]) walk(p);
  return lines.map((l) => {
    let a = l;
    for (let it = 0; it < 2; it++) a = a.map((_, i) => {
      if (i === 0 || i === a.length - 1) return a[i];
      let x = 0, y = 0, n = 0;
      for (let k = Math.max(0, i - 3); k <= Math.min(a.length - 1, i + 3); k++) { x += a[k][0]; y += a[k][1]; n++; }
      return [x / n, y / n];
    });
    return a.filter((_, i) => i % 2 === 0 || i === a.length - 1);
  });
}

// What the art says. Every field returned is half size (w2 x h2).
export function read(px, w, h, o = P) {
  const n = w * h, w2 = w >> 1, h2 = h >> 1, m = w2 * h2;
  const L = new Float32Array(n), wc = new Float32Array(n);
  const R2 = new Float32Array(m), G2 = new Float32Array(m), B2 = new Float32Array(m);
  for (let y = 0; y < h; y++) {
    const row = (y >> 1) * w2, in2 = y < h2 * 2;
    for (let x = 0; x < w; x++) {
      const i = y * w + x, r = px[i * 4], g = px[i * 4 + 1], b = px[i * 4 + 2], l = (r + g + b) / 3;
      L[i] = l;
      // The water wash is the only cool colour on the sheet: blue near red, green at or above red,
      // and never bright (pale sand and snow-white paper are).
      const br = b / (r + 1), gr = g / (r + 1);
      wc[i] = br > 0.5 && gr > 0.8 && l < 175 && l > 55 ? sstep(0.5, 0.68, br) * sstep(0.8, 0.9, gr) * sstep(175, 150, l) : 0;
      if (in2 && x < w2 * 2) { const k = row + (x >> 1); R2[k] += r * 0.25; G2[k] += g * 0.25; B2[k] += b * 0.25; }
    }
  }
  // Box means of lightness along the sheet (A) and down it (V), 9 pixels long. A level pen line
  // survives the first and is wiped out by the second; any other stroke does the opposite or neither.
  const A = new Float32Array(n), V = new Float32Array(n);
  for (let y = 0; y < h; y++) { const o2 = y * w; let acc = 0; for (let x = 0; x < 9; x++) acc += L[o2 + x]; for (let x = 4; x < w - 5; x++) { A[o2 + x] = acc / 9; acc += L[o2 + x + 5] - L[o2 + x - 4]; } }
  for (let x = 0; x < w; x++) { let acc = 0; for (let y = 0; y < 9; y++) acc += L[y * w + x]; for (let y = 4; y < h - 5; y++) { V[y * w + x] = acc / 9; acc += L[(y + 5) * w + x] - L[(y - 4) * w + x]; } }
  // Pooled to half size: water colour by max (keeps the wash between pen lines), hatching and
  // texture by mean.
  const wraw = new Float32Array(m), hat = new Float32Array(m), tex = new Float32Array(m), L2 = new Float32Array(m), ink = new Float32Array(m);
  for (let j = 0; j < h2; j++)
    for (let i = 0; i < w2; i++) {
      let wm = 0, hs = 0, ts = 0, dark = 0;
      for (let dy = 0; dy < 2; dy++)
        for (let dx = 0; dx < 2; dx++) {
          const x = i * 2 + dx, y = j * 2 + dy, p = y * w + x, l = L[p];
          if (wc[p] > wm) wm = wc[p];
          if (l < 46) dark++;
          if (x < 6 || y < 6 || x >= w - 6 || y >= h - 6) continue;
          hs += Math.abs(A[p - w] + A[p + w] - 2 * A[p]) + Math.abs(A[p - 2 * w] + A[p + 2 * w] - 2 * A[p]) * 0.5
            - 1.6 * (Math.abs(V[p - 1] + V[p + 1] - 2 * V[p]) + Math.abs(V[p - 2] + V[p + 2] - 2 * V[p]) * 0.5);

        }
      const k = j * w2 + i;
      wraw[k] = wm; ink[k] = dark * 0.25; hat[k] = Math.max(-4, hs * 0.25); L2[k] = (R2[k] + G2[k] + B2[k]) / 3;
    }

  // The sheet fades out before the torn paper edge and the corner ornaments.
  const edge = new Float32Array(m);
  for (let y = 0; y < h2; y++)
    for (let x = 0; x < w2; x++) {
      const cx = Math.min(x, w2 - 1 - x) * 2, cy = Math.min(y, h2 - 1 - y) * 2, d = Math.min(cx, cy);
      if (d >= 46 && (cx >= 80 || cy >= 66)) { edge[y * w2 + x] = 1; continue; }
      const dc = Math.hypot(Math.max(0, 80 - cx), Math.max(0, 66 - cy));
      edge[y * w2 + x] = Math.min(sstep(12, 46, d), 1 - sstep(18, 70, dc));
    }

  // Water: the wash, or level hatching over a wide enough patch.
  const hb = gauss(hat, w2, h2, 4.5), wcol = new Float32Array(m), wany = new Float32Array(m);
  {
    // One more max down the sheet closes the wash across each pen line.
    for (let y = 0; y < h2; y++) for (let x = 0; x < w2; x++) {
      const k = y * w2 + x;
      wcol[k] = Math.max(wraw[k], y > 0 ? wraw[k - w2] : 0, y < h2 - 1 ? wraw[k + w2] : 0);
    }
    // Hatching only ever extends the wash it is drawn on, so level strokes inland (lettering, the
    // foot of a range) never become a lake.
    const nearWash = wide(wcol, w2, h2, 10, 4);
    for (let k = 0; k < m; k++) wany[k] = Math.max(wcol[k], sstep(o.hlo, o.hhi, hb[k]) * sstep(0.06, 0.25, nearWash[k]));
  }
  const w1 = gauss(wcol, w2, h2, 1.1), wa = gauss(wany, w2, h2, 1.3), all = new Uint8Array(m);
  for (let i = 0; i < m; i++) all[i] = Math.max(w1[i], wa[i]) > 0.5 ? 1 : 0;
  // Water wide enough to survive an erosion is a body (sea, lake) and gets a coast line; the
  // narrow rest is a river and gets the sheet's blue line down its middle.
  const core = morph(morph(all, w2, h2, o.wbody, 0), w2, h2, o.wbody + 2, 1), rmask = new Uint8Array(m), solid = new Float32Array(m);
  for (let i = 0; i < m; i++) { solid[i] = all[i] && core[i] ? 1 : 0; rmask[i] = all[i] && !core[i] && edge[i] > 0.3 ? 1 : 0; }
  // A sea that pales toward the sheet's edge is still sea: a wide blur fills it in. Solid land
  // inside it (an island) has no water near it and stays.
  const w3 = gauss(solid, w2, h2, 1.2), w6 = wide(wany, w2, h2, 3, 2), fill = wide(wany, w2, h2, 8, 4), body = new Float32Array(m), cst = new Float32Array(m);
  for (let i = 0; i < m; i++) { cst[i] = Math.max(w3[i], sstep(0.5, 0.75, fill[i]) * sstep(0.1, 0.32, w6[i]) * 0.9); body[i] = sstep(0.4, 0.6, cst[i]); }
  // Past the last hatching the sea fades to bare paper before the sheet ends. Ground cut off from
  // the rest by water, and never further than a strip's width from the sheet's edge, is that sea.
  {
    const lab = new Int32Array(m), stack = new Int32Array(m), far = [0];
    let next = 0;
    for (let s0 = 0; s0 < m; s0++) {
      if (lab[s0] || body[s0] > 0.5 || edge[s0] <= 0.5) continue;
      let top = 0, reach = 0; lab[s0] = ++next; stack[top++] = s0;
      while (top) {
        const p = stack[--top], x = p % w2, y = (p / w2) | 0, d = Math.min(x, w2 - 1 - x, y, h2 - 1 - y);
        if (d > reach) reach = d;
        if (x > 0 && !lab[p - 1] && body[p - 1] <= 0.5 && edge[p - 1] > 0.5) { lab[p - 1] = next; stack[top++] = p - 1; }
        if (x < w2 - 1 && !lab[p + 1] && body[p + 1] <= 0.5 && edge[p + 1] > 0.5) { lab[p + 1] = next; stack[top++] = p + 1; }
        if (y > 0 && !lab[p - w2] && body[p - w2] <= 0.5 && edge[p - w2] > 0.5) { lab[p - w2] = next; stack[top++] = p - w2; }
        if (y < h2 - 1 && !lab[p + w2] && body[p + w2] <= 0.5 && edge[p + w2] > 0.5) { lab[p + w2] = next; stack[top++] = p + w2; }
      }
      far.push(reach);
    }
    for (let i = 0; i < m; i++) if (lab[i] && far[lab[i]] < o.strip) { cst[i] = Math.max(cst[i], 0.95); body[i] = 1; }
  }
  // The faded frame says nothing about land or sea, so it takes whatever the sheet beside it is.
  // Otherwise a sea that runs off the sheet would get a shore, and depth lines, along the frame.
  const inside = new Float32Array(m), bin = new Float32Array(m);
  for (let i = 0; i < m; i++) { inside[i] = edge[i] > 0.5 ? 1 : 0; bin[i] = inside[i] * (body[i] > 0.5 ? 1 : 0); }
  const fn = wide(bin, w2, h2, 6, 4), fd = wide(inside, w2, h2, 6, 4);
  for (let i = 0; i < m; i++) if (!inside[i] && fn[i] > 0.5 * fd[i] + 1e-4) { body[i] = 1; cst[i] = Math.max(cst[i], 0.95); }
  const within = (sigma) => {
    const src = new Float32Array(m);
    for (let i = 0; i < m; i++) src[i] = inside[i] * body[i];
    const a = wide(src, w2, h2, sigma, 4), b = wide(inside, w2, h2, sigma, 4);
    for (let i = 0; i < m; i++) a[i] = b[i] > 0.02 ? Math.min(1, a[i] / b[i]) : body[i];
    return a;
  };
  const shore = within(7), deep = within(17);
  for (let i = 0; i < m; i++) if (cst[i] > 0.5) rmask[i] = 0;
  const rivers = trace(keepLong(rmask, w2, h2, 15), w2, h2, 7);

  // Mountains (and a city's buildings) are where the pen worked hardest. Water hatching and the
  // coast's own outline are pen work too and are taken out.
  // Pen strokes are a few pixels wide: lightness against its own neighbourhood, at half size so
  // paper grain has already averaged out. One stroke counts as a stroke however black it is.
  const l1 = gauss(L2, w2, h2, 0.8), l4 = gauss(L2, w2, h2, 3.2);
  for (let i = 0; i < m; i++) { tex[i] = Math.min(o.tcap, Math.abs(l1[i] - l4[i])); ink[i] = l4[i] - l1[i] > o.idark ? 1 : 0; }
  // Lettering and icons are inked near black, which no painted slope is; strokes beside that ink
  // are not counted.
  const inky = gauss(ink, w2, h2, 2.2), dry = new Float32Array(m);
  for (let i = 0; i < m; i++) dry[i] = tex[i] * (1 - sstep(0.02, 0.2, shore[i])) * (1 - sstep(0.1, 0.4, wa[i])) * (1 - sstep(o.ilo, o.ihi, inky[i]));
  const energy = wide(dry, w2, h2, o.range, 2), mtn = new Float32Array(m);
  for (let i = 0; i < m; i++) mtn[i] = sstep(o.tlo, o.thi, energy[i]);

  // The zone's own ground is whatever is coloured unlike the bare parchment round the rim.
  // Chroma only (g/r, b/r) plus a little lightness, measured against the rim's median.
  const gr = new Float32Array(m), br = new Float32Array(m), rim = new Uint8Array(m), land = new Uint8Array(m);
  for (let y = 0; y < h2; y++) for (let x = 0; x < w2; x++) {
    const k = y * w2 + x, d = Math.min(x, w2 - 1 - x, y, h2 - 1 - y);
    gr[k] = G2[k] / (R2[k] + 1); br[k] = B2[k] / (R2[k] + 1);
    land[k] = body[k] < 0.3 && shore[k] < 0.25 ? 1 : 0;
    rim[k] = land[k] && d >= 22 && d < 50 && mtn[k] < 0.3 ? 1 : 0;
  }
  const inner = new Uint8Array(m);
  for (let i = 0; i < m; i++) inner[i] = land[i] && edge[i] > 0.95 ? 1 : 0;
  let rimN = 0; for (let i = 0; i < m; i++) rimN += rim[i];
  const base = rimN > 400 ? rim : land;
  const g0 = percentile(gr, 0.5, base), b0 = percentile(br, 0.5, base), l0 = percentile(L2, 0.5, base);
  const dist = new Float32Array(m);
  for (let i = 0; i < m; i++) dist[i] = Math.hypot(gr[i] - g0, br[i] - b0, ((L2[i] - l0) / 255) * 0.6);
  const dsm = wide(dist, w2, h2, 3, 2), top = Math.max(0.12, percentile(dsm, 0.93, inner));
  const zraw = new Float32Array(m);
  for (let i = 0; i < m; i++) zraw[i] = sstep(0.38, 0.75, dsm[i] / top);
  const zb = wide(zraw, w2, h2, 5.5, 4), zone = new Float32Array(m);
  for (let i = 0; i < m; i++) zone[i] = sstep(0.25, 0.6, zb[i]) * (1 - body[i]) * edge[i];


  // Rolling relief from the parchment's mottling, so open ground has a few wandering contours.
  const m20 = wide(L2, w2, h2, 13, 4), m70 = wide(L2, w2, h2, 40, 8);

  const H = new Float32Array(m);
  for (let i = 0; i < m; i++) {
    const ramp = 0.5 - shore[i];
    if (body[i] > 0.5) { H[i] = -0.02 - Math.max(0, deep[i] - 0.5) * 0.75 - Math.max(0, -ramp) * 0.12; continue; }
    const inland = sstep(0, 0.3, ramp);
    H[i] = 0.03 + 0.08 * inland + o.zoneLift * zone[i] * inland + o.mountain * mtn[i] * (0.35 + 0.65 * inland)
      + clamp(((m20[i] - m70[i]) / 255) * 5, -1, 1) * 0.12 * o.roll * inland * (1 - mtn[i]);
    H[i] *= edge[i] * 0.6 + 0.4;
  }
  return { w: w2, h: h2, L, l4, H, body, cst, edge, mtn, zone, rivers, wcol, hb, tex, energy, dsm, rmask, inky, dry };
}

// The blend mask: where the painting has pen work darker than the ground round it (roads, walls,
// building outlines, bridges, shore detail), as an alpha field at the art's size. Water hatching
// is left out; so is whatever is short and compact, which is lettering, icons and single hill strokes.
export function lift(f, w, h, o = P) {
  const n = w * h, L = f.L, b = f.l4, a = new Float32Array(n), m = new Uint8Array(n);
  const w2 = f.w, h2 = f.h;
  for (let y = 0; y < h; y++) {
    const row = Math.min(h2 - 1, y >> 1) * w2;
    for (let x = 0; x < w; x++) {
      const i = y * w + x, k = row + Math.min(w2 - 1, x >> 1);
      let v = sstep(o.inkLo, o.inkHi, b[k] - L[i]) * (1 - sstep(0.15, 0.5, f.cst[k])) * f.edge[k];
      if (o.ink === "open") v *= 1 - 0.92 * f.mtn[k];
      a[i] = v; m[i] = v > 0.3 ? 1 : 0;
    }
  }
  if (o.ink !== "all") {
    const keep = keepLong(m, w, h, o.inkLong);
    // A kept stroke keeps its soft edge: one pixel round it stays.
    const near = morph(keep, w, h, 1, 1);
    for (let i = 0; i < n; i++) if (!near[i]) a[i] = 0;
  }
  const d = new Uint8ClampedArray(n * 4);
  for (let i = 0; i < n; i++) { d[i * 4] = d[i * 4 + 1] = d[i * 4 + 2] = 255; d[i * 4 + 3] = a[i] * 255 * o.inkAlpha; }
  return toCanvas(d, w, h);
}

export function styleMap(px, w, h, o = P) {
  const t0 = performance.now();
  const f = read(px, w, h, o), W = f.w, Hh = f.h, n = W * Hh, K = (w * S) / W;
  const t1 = performance.now();
  const Hs = gauss(f.H, W, Hh, o.smooth);
  // Hold the shore at zero where the water body ends, so contours never cross the coast.
  for (let i = 0; i < n; i++) Hs[i] = f.body[i] > 0.5 ? Math.min(Hs[i], -0.01) : Math.max(Hs[i], 0.012);

  const T = [], tick = () => T.push(Math.round(performance.now()));
  tick();
  const zt = wide(f.zone, W, Hh, 3, 2);
  const rgba = new Uint8ClampedArray(n * 4);
  for (let y = 0; y < Hh; y++)
    for (let x = 0; x < W; x++) {
      const k = y * W + x, v = Hs[k];
      const dx = Hs[y * W + Math.min(x + 2, W - 1)] - Hs[y * W + Math.max(x - 2, 0)];
      const dy = Hs[Math.min(y + 2, Hh - 1) * W + x] - Hs[Math.max(y - 2, 0) * W + x];
      const shade = v > 0 ? clamp(-(dx + dy) * 13, -1, 1) * 7 : 0;
      const t = v > 0 ? Math.min(1, v / 1.1 + o.landTone + o.zoneTone * zt[k]) : 0, land = 1 - f.body[k], e = f.edge[k];
      for (let c = 0; c < 3; c++) {
        const col = C.sea[c] + (C.bg[c] + ((C.hi[c] - C.bg[c]) * t + shade) * o.toneGain - C.sea[c]) * land;
        rgba[k * 4 + c] = C.bg[c] + (col - C.bg[c]) * e;
      }
      rgba[k * 4 + 3] = 255;
    }

  const out = document.createElement("canvas");
  out.width = w * S; out.height = h * S;
  const ctx = out.getContext("2d");
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(toCanvas(rgba, W, Hh), 0, 0, w * S, h * S);
  tick();

  // Lines go straight onto the sheet; a veil of the ground colour then fades them out with its edge.
  const lc = ctx;
  lc.save();
  lc.scale(K, K);
  lc.translate(0.5, 0.5);
  const lw = (o.line * S) / K;
  lc.lineCap = "butt"; lc.lineJoin = "bevel";
  const levels = [], groupOf = [], paths = [new Path2D(), new Path2D(), new Path2D()];
  for (let i = 1; i <= o.levels; i++) { levels.push(i * o.interval); groupOf.push(i % 5 === 0 ? 1 : 0); }
  for (const l of [-0.07, -0.12, -0.17, -0.23, -0.3]) { levels.push(l); groupOf.push(2); }
  // The height field is smooth, so every second sample carries it.
  const gw = (W + 1) >> 1, gh = (Hh + 1) >> 1, G = new Float32Array(gw * gh);
  for (let j = 0; j < gh; j++) for (let i = 0; i < gw; i++) G[j * gw + i] = Hs[j * 2 * W + i * 2];
  isoMany(G, gw, gh, 2, levels, groupOf, paths);
  tick();
  lc.lineWidth = lw;
  [o.minor, o.major, o.depth].forEach((a, i) => { lc.strokeStyle = `rgba(255,255,255,${a})`; lc.stroke(paths[i]); });

  tick();
  // Rivers are traced from the art and stroked like every other line on the sheet.
  lc.lineCap = "round"; lc.lineJoin = "round";
  const stroke = (lines) => { lc.beginPath(); for (const l of lines) l.forEach(([x, y], i) => (i ? lc.lineTo(x, y) : lc.moveTo(x, y))); lc.stroke(); };
  lc.strokeStyle = `rgba(47,158,196,${o.river})`; lc.lineWidth = lw * 1.6;
  stroke(f.rivers);

  if (o.ink !== "off") {
    lc.save(); lc.setTransform(1, 0, 0, 1, 0, 0);
    lc.imageSmoothingEnabled = true; lc.imageSmoothingQuality = "high";
    lc.drawImage(lift(f, w, h, o), 0, 0, w * S, h * S);
    lc.restore();
  }
  tick();
  const coast = [new Path2D()];
  isoMany(gauss(f.cst, W, Hh, 0.7), W, Hh, 1, [0.5], [0], coast);
  lc.lineCap = "butt";
  lc.strokeStyle = `rgba(255,255,255,${o.coast})`;
  lc.lineWidth = lw * 1.25;
  lc.stroke(coast[0]);

  tick();
  const veil = new Uint8ClampedArray(n * 4);
  for (let i = 0; i < n; i++) { veil[i * 4] = C.bg[0]; veil[i * 4 + 1] = C.bg[1]; veil[i * 4 + 2] = C.bg[2]; veil[i * 4 + 3] = (1 - f.edge[i]) * 255; }
  lc.restore();
  ctx.drawImage(toCanvas(veil, W, Hh), 0, 0, w * S, h * S);
  tick();
  styleMap.last = { steps: T.slice(1).map((t, i) => t - T[i]), read: Math.round(t1 - t0), draw: Math.round(performance.now() - t1) };
  return out;
}

// What is drawn over the sheet: names, the walked lines and the layers the Map view switches.
// Every size here is in the context's own pixels; positions come in the recorded unit (map
// coordinates times 10000) and go through X and Y.

const FG = "240,241,243", RIVER = "47,158,196", DARK = "#141416";
const nameKey = (name) => name.toLowerCase().replace(/^the /, "");

// One list of names for a sheet from its two sources: `table` is the map's label table (the
// client's own area, town and flight point names, written by tools/map-labels.mjs) and `places`
// the subzones the player's own lines name, [{ name, n, x, y }]. A place both know keeps the
// table's position and kind and the player's count; `found` are the area names of EX lines.
export function mergeLabels(table, places, found) {
  const all = new Map();
  for (const l of table || []) {
    const had = all.get(nameKey(l.name));
    // "Crossroads" the flight point and "The Crossroads" the town are one place.
    if (had) { if (l.kind === "town" || had.kind === "flight") Object.assign(had, l, { flight: had.flight || l.flight }); else had.flight ||= l.flight; }
    else all.set(nameKey(l.name), { ...l, n: 0 });
  }
  for (const p of places || []) {
    const had = all.get(nameKey(p.name));
    if (had) had.n = p.n;
    else all.set(nameKey(p.name), { name: p.name, kind: "own", x: p.x, y: p.y, n: p.n });
  }
  for (const f of found || []) { const had = all.get(nameKey(f.name)); if (had) had.found = true; }
  return [...all.values()];
}

// Letters the sheet. Towns first, then where the player has been (busiest first), then the rest;
// a name that would run over one already drawn, or off the sheet, tries its other sides and is
// left out when none is free. A town or a point is a dot with its name beside it; an area has no
// dot and is spaced capitals on its middle. Where the player has been is lettered brighter.
export function drawLabels(ctx, list, X, Y, bounds) {
  ctx.save();
  ctx.lineJoin = "round";
  const taken = [], hit = (b) => taken.some((t) => b[0] < t[2] && b[2] > t[0] && b[1] < t[3] && b[3] > t[1]);
  const off = (b) => bounds && (b[0] < bounds[0] || b[1] < bounds[1] || b[2] > bounds[2] || b[3] > bounds[3]);
  const rank = (l) => (l.kind === "town" ? 4e9 : 0) + (l.n ? 1e9 + l.n : 0) + (l.found ? 5e8 : 0) + (l.kind === "area" || l.kind === "zone" ? 0 : 1e6);
  const spaced = "letterSpacing" in ctx;
  for (const l of [...(list || [])].sort((a, b) => rank(b) - rank(a))) {
    const x = X(l.x), y = Y(l.y), seen = l.n > 0 || l.found;
    if (l.kind === "area" || l.kind === "zone") {
      const text = l.name.toUpperCase();
      ctx.font = '600 10px "Sofia Sans", "Bahnschrift", sans-serif';
      if (spaced) ctx.letterSpacing = "1.6px";
      const tw = ctx.measureText(text).width;
      for (const dy of [3.5, -9, 16]) {
        const box = [x - tw / 2 - 4, y + dy - 10, x + tw / 2 + 4, y + dy + 4];
        if (hit(box) || off(box)) continue;
        taken.push(box);
        ctx.textAlign = "center";
        ctx.strokeStyle = "rgba(20,20,22,0.7)"; ctx.lineWidth = 3;
        ctx.strokeText(text, x, y + dy);
        ctx.fillStyle = `rgba(${FG},${seen ? 0.62 : 0.34})`;
        ctx.fillText(text, x, y + dy);
        break;
      }
      if (spaced) ctx.letterSpacing = "0px";
      continue;
    }
    const town = l.kind === "town";
    ctx.font = `${town ? 600 : 500} ${town ? 12 : 11.5}px "Sofia Sans", "Bahnschrift", sans-serif`;
    const tw = ctx.measureText(l.name).width, dot = [x - 5, y - 5, x + 5, y + 5];
    if (hit(dot)) continue;
    for (const [tx, ty, align] of [[x + 8, y + 4, "left"], [x - 8, y + 4, "right"], [x, y - 8, "center"], [x, y + 16, "center"]]) {
      const x0 = align === "left" ? tx : align === "right" ? tx - tw : tx - tw / 2;
      const box = [x0 - 3, ty - 11, x0 + tw + 3, ty + 4];
      if (hit(box) || off(box)) continue;
      taken.push(box, dot);
      const alpha = town ? 0.9 : seen ? 0.74 : 0.46;
      ctx.textAlign = align;
      ctx.strokeStyle = "rgba(20,20,22,0.82)"; ctx.lineWidth = 3.5;
      ctx.strokeText(l.name, tx, ty);
      ctx.strokeRect(x - 2, y - 2, 4, 4);
      ctx.fillStyle = `rgba(${FG},${alpha})`;
      ctx.fillRect(x - 2, y - 2, 4, 4);
      ctx.fillText(l.name, tx, ty);
      break;
    }
  }
  ctx.restore();
}

// One mark vocabulary for the canvas and for the layer switches' swatches. The marks are
// kept (death the cross, a quest the diamond); the rest are outlines in the same weight.
export function drawMark(ctx, kind, x, y) {
  ctx.save();
  ctx.translate(x, y);
  ctx.lineCap = ctx.lineJoin = "round";
  const ring = (draw, colour = `rgb(${FG})`, w = 1.5) => { ctx.strokeStyle = DARK; ctx.lineWidth = w + 2.6; draw(); ctx.stroke(); ctx.strokeStyle = colour; ctx.lineWidth = w; draw(); ctx.stroke(); };
  const diamond = (r) => { ctx.beginPath(); ctx.moveTo(0, -r); ctx.lineTo(r, 0); ctx.lineTo(0, r); ctx.lineTo(-r, 0); ctx.closePath(); };
  if (kind === "death") ring(() => { ctx.beginPath(); ctx.moveTo(-4.5, -4.5); ctx.lineTo(4.5, 4.5); ctx.moveTo(4.5, -4.5); ctx.lineTo(-4.5, 4.5); }, `rgb(${FG})`, 1.6);
  else if (kind === "quest-done") { diamond(4.6); ctx.strokeStyle = DARK; ctx.lineWidth = 3; ctx.stroke(); ctx.fillStyle = `rgba(${FG},0.92)`; ctx.fill(); }
  else if (kind === "quest") ring(() => diamond(4), `rgba(${FG},0.85)`, 1.3);
  else if (kind === "npc" || kind === "flightmaster") {
    ring(() => { ctx.beginPath(); ctx.rect(-3.2, -3.2, 6.4, 6.4); }, `rgba(${FG},0.8)`, 1.3);
    if (kind === "flightmaster") { ctx.fillStyle = `rgba(${FG},0.9)`; ctx.fillRect(-1.2, -1.2, 2.4, 2.4); }
  } else if (kind === "gather" || kind === "fishing") ring(() => { ctx.beginPath(); ctx.arc(0, 0, 3.1, 0, 6.3); }, kind === "fishing" ? `rgb(${RIVER})` : `rgba(${FG},0.8)`, 1.4);
  else if (kind === "camp") ring(() => { ctx.beginPath(); ctx.moveTo(0, -4.4); ctx.lineTo(4.6, 3.6); ctx.lineTo(-4.6, 3.6); ctx.closePath(); }, `rgba(${FG},0.9)`, 1.4);
  else if (kind === "found") { ring(() => { ctx.beginPath(); ctx.arc(0, 0, 4.2, 0, 6.3); }, `rgba(${FG},0.7)`, 1.1); ctx.fillStyle = `rgba(${FG},0.9)`; ctx.fillRect(-1, -1, 2, 2); }
  else if (kind === "teleport") ring(() => { ctx.beginPath(); ctx.arc(0, 0, 3.6, 0, 6.3); }, "rgba(255,107,142,0.95)", 1.4);
  ctx.restore();
}

export const LAYERS = [
  ["travel", "Flights and teleports"], ["deaths", "Deaths"], ["met", "Creatures met"], ["gather", "Gathering and fishing"],
  ["people", "NPCs"], ["quests", "Quests"], ["found", "Discovered"], ["camps", "Camps"], ["names", "Names"],
];

// The walked lines, always, in scarlet: thin lines that add up where routes are shared. Under
// them the faint heat of where creatures were met; over them whichever layers `on` switches.
//   data: { walks, flights, teleports, deaths, met [[cellX, cellY, n]], gather, fishing, npcs [[x, y, kind]],
//           quests [[x, y, 1 accepted | 2 turned in]], found [{ name, x, y }], camps }
//   labels: the merged names (mergeLabels), used to frame a discovered area
export function drawLayers(ctx, data, X, Y, on = {}, labels = [], o = {}) {
  const core = o.core ?? 0.34, halo = o.halo ?? 0.03;
  ctx.save();
  ctx.lineCap = ctx.lineJoin = "round";
  if (on.met && data.met?.length) {
    // A cell is 0.015 of the map. Each is a soft patch of light, stronger the more was met there.
    const r = Math.abs(X(150) - X(0)) * 1.05;
    ctx.globalCompositeOperation = "lighter";
    for (const [cx, cy, n] of data.met) {
      const x = X(cx * 150 + 75), y = Y(cy * 150 + 75), a = Math.min(0.1, 0.035 + 0.02 * Math.log2(1 + n));
      const g = ctx.createRadialGradient(x, y, 0, x, y, r);
      g.addColorStop(0, `rgba(${FG},${a})`); g.addColorStop(1, `rgba(${FG},0)`);
      ctx.fillStyle = g; ctx.fillRect(x - r, y - r, 2 * r, 2 * r);
    }
    ctx.globalCompositeOperation = "source-over";
  }
  if (on.found) {
    // A discovered area that the label table knows is framed by four corner ticks.
    const byName = new Map(labels.map((l) => [nameKey(l.name), l]));
    ctx.strokeStyle = `rgba(${FG},0.38)`; ctx.lineWidth = 1.1;
    for (const f of data.found || []) {
      const l = byName.get(nameKey(f.name));
      if (!l || !l.w) continue;
      const x0 = X(l.x - l.w / 2), x1 = X(l.x + l.w / 2), y0 = Y(l.y - l.h / 2), y1 = Y(l.y + l.h / 2), t = Math.min(9, (x1 - x0) / 4);
      ctx.beginPath();
      for (const [cx, cy, sx, sy] of [[x0, y0, 1, 1], [x1, y0, -1, 1], [x0, y1, 1, -1], [x1, y1, -1, -1]]) { ctx.moveTo(cx + sx * t, cy); ctx.lineTo(cx, cy); ctx.lineTo(cx, cy + sy * t); }
      ctx.stroke();
    }
  }
  for (const [width, alpha] of [[7, halo], [1.5, core]]) {
    ctx.lineWidth = width; ctx.strokeStyle = `rgba(255,45,85,${alpha})`;
    for (const p of data.walks) {
      ctx.beginPath();
      p.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))));
      ctx.stroke();
    }
  }
  if (on.travel) {
    // A flight is a dashed line; a teleport a dotted one that ends in a ring where it set you down.
    const line = (list, dash) => {
      ctx.setLineDash(dash); ctx.lineWidth = 1.4; ctx.strokeStyle = "rgba(255,107,142,0.7)";
      const drawn = new Set();
      for (const [a, b] of list || []) {
        const key = [a, b].map((p) => p.map((v) => Math.round(v / 150)).join(",")).sort().join("|");
        if (drawn.has(key)) continue;
        drawn.add(key);
        ctx.beginPath(); ctx.moveTo(X(a[0]), Y(a[1])); ctx.lineTo(X(b[0]), Y(b[1])); ctx.stroke();
      }
      ctx.setLineDash([]);
    };
    line(data.flights, [7, 6]);
    line(data.teleports, [1, 5]);
    for (const [, b] of data.teleports || []) drawMark(ctx, "teleport", X(b[0]), Y(b[1]));
  }
  const marks = (list, kind) => { for (const p of list || []) drawMark(ctx, typeof kind === "function" ? kind(p) : kind, X(p[0]), Y(p[1])); };
  if (on.gather) { marks(data.gather, "gather"); marks(data.fishing, "fishing"); }
  if (on.people) marks(data.npcs, (p) => (p[2] === "f" ? "flightmaster" : "npc"));
  if (on.quests) marks(data.quests, (p) => (p[2] === 2 ? "quest-done" : "quest"));
  if (on.found) for (const f of data.found || []) drawMark(ctx, "found", X(f.x), Y(f.y));
  if (on.camps) marks(data.camps, "camp");
  if (on.deaths) marks(data.deaths, "death");
  ctx.restore();
}
