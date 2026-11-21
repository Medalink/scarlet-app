// The detail the shipped dark sheets carry on top of the Survey look (app/ui/views/mapstyle.js):
// mountain ranges, roads, the shore and, on a town sheet, its buildings and walls.
// tools/render-dark.mjs hands drawDetail to styleMap, which calls it once the contours are down.
//
// No pixel of the art reaches the sheet. The art is only measured: where its pen lines run, how
// hard and how long they are, which way they go, what colour the ground is. Those measurements
// become a few facts (a range runs along here, a road goes from there to there, a building covers
// this patch), and each fact is drawn with a mark made here:
//   range     a crest line of our own zigzag along the range's axis, with shade strokes under it
//   road      one even line down the road's middle
//   shore     lines that follow the coast out to sea, closer together near the land; a line where
//             the shallows end; an islet as a small rounded outline
//   building  its outline as a polygon of a few straight sides
//   wall      a chain of straight segments
// Sizes are in half-size pixels, the unit styleMap's fields and line context use, unless they say
// art pixels.

import { morph, keepLong, trace, isoMany, sstep } from "../app/ui/views/mapstyle.js";

export const D = {
  penCap: 20, letters: 8.5, // fine pen work this dense (bright strokes edged dark) is lettering
  crestLo: 6, crestCap: 14, // hard pen: dark line strength over the first, counted up to the second
  ridgeBlur: [2.8, 1.5], // crest strokes this near, along the sheet and down it, are one row
  ridgeLevel: -0.1,   // a row's direction as cos of twice its angle: 1 level, -1 upright
  ridgeLo: 0.06, ridgeMin: 0.3, ridgeMtn: 0.1, ridgeLong: 8,
  ridge: 0.3, shade: 0.17, // line alphas
  roadEdge: 3, roadGap: [3, 4, 5], roadLift: 3, // a road's two edge lines: how dark, how far apart (art pixels), how much lighter between
  roadLo: 0.3, roadHi: 1, roadLong: 22, roadJoin: 7, road: 0.26,
  roadBeside: [0.5, 0.5, 15], // votes beside a road point over this share of its own; a stretch with this share of such points, counted over this many, is not a road
  ground: true,       // ranges and roads; off for a continent or the world, whose art is too small to measure them
  shoreAt: [2, 4.6, 8.2], shoreAlpha: [0.15, 0.09, 0.05],
  shelf: 0.11, shelfOff: 5, // the shallows' edge, and how far off the coast it may start
  isletMin: 3, isletMax: 400, isletTone: "rgb(24,24,27)",
  town: false,        // a town sheet: draw buildings and walls
  bldPen: 5, bldEdge: 11, bldMin: 40, bldMax: 2800, // a building's outline: how dark its pen, how sharp its change of colour, and its size in art pixels
  bldLong: 2.2, bldContrast: 16, bldFilled: 0.5, bldSides: 1.3,
  bldReach: 18, bldGroup: 4, // buildings this near each other are one group; a group smaller than this is not a town
  bld: 0.34, bldFill: 0.055,
  wallLo: 6.5, wallHi: 13, wallLong: 14, wallSides: 0.9, wall: 0.24,
};

// A number in [0, 1) fixed by its place, so a sheet draws the same every time.
function rand(x, y, k = 0) {
  let v = (Math.imul(Math.round(x * 8), 374761393) + Math.imul(Math.round(y * 8), 668265263) + Math.imul(k + 1, 1274126177)) | 0;
  v = Math.imul(v ^ (v >>> 13), 1103515245);
  return ((v ^ (v >>> 16)) >>> 0) / 4294967296;
}

// A true Gaussian blur, by sigma along the sheet and (when it differs) down it. mapstyle's gauss
// is three box blurs and cannot go below a sigma of 1.4.
function blur(src, w, h, sigma, down = sigma) {
  const kernel = (sg) => {
    const r = Math.max(1, Math.ceil(sg * 3)), k = new Float32Array(2 * r + 1);
    let sum = 0;
    for (let i = -r; i <= r; i++) sum += k[i + r] = Math.exp((-i * i) / (2 * sg * sg));
    for (let i = 0; i <= 2 * r; i++) k[i] /= sum;
    return [k, r];
  };
  const tmp = new Float32Array(w * h), out = new Float32Array(w * h), [kx, rx] = kernel(sigma), [ky, ry] = kernel(down);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    let acc = 0;
    for (let i = -rx; i <= rx; i++) acc += kx[i + rx] * src[y * w + Math.min(w - 1, Math.max(0, x + i))];
    tmp[y * w + x] = acc;
  }
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    let acc = 0;
    for (let i = -ry; i <= ry; i++) acc += ky[i + ry] * tmp[Math.min(h - 1, Math.max(0, y + i)) * w + x];
    out[y * w + x] = acc;
  }
  return out;
}
// A field's value between pixels.
function between(a, w, h, x, y) {
  x = Math.min(w - 1.001, Math.max(0, x)); y = Math.min(h - 1.001, Math.max(0, y));
  const i = x | 0, j = y | 0, u = x - i, v = y - j, p = j * w + i;
  return (a[p] * (1 - u) + a[p + 1] * u) * (1 - v) + (a[p + w] * (1 - u) + a[p + w + 1] * u) * v;
}

// Where a field has a line through it: bright lines for sign 1, dark ones for -1. For each pixel,
// how sharply the field falls away on both sides (s), less whatever it also falls away along the
// line, so a dot is not a line; and the line's direction as a doubled angle (c, d).
function lineField(g, w, h, sign) {
  const n = w * h, s = new Float32Array(n), c = new Float32Array(n), d = new Float32Array(n);
  for (let y = 1; y < h - 1; y++)
    for (let x = 1; x < w - 1; x++) {
      const p = y * w + x, v = 2 * g[p];
      const xx = (g[p - 1] + g[p + 1] - v) * sign, yy = (g[p - w] + g[p + w] - v) * sign;
      const xy = (g[p + w + 1] + g[p - w - 1] - g[p + w - 1] - g[p - w + 1]) * 0.25 * sign;
      const mean = (xx + yy) / 2, dx = (xx - yy) / 2, r = Math.hypot(dx, xy);
      const line = r - mean - Math.max(0, -(mean + r));
      if (line <= 0) continue;
      s[p] = line;
      if (r > 1e-6) { c[p] = dx / r; d[p] = xy / r; }
    }
  return { s, c, d };
}

// Each connected piece of a mask, as (members, count).
function pieces(mask, w, h, each) {
  const seen = new Uint8Array(w * h), stack = new Int32Array(w * h), members = new Int32Array(w * h);
  for (let s0 = 0; s0 < w * h; s0++) {
    if (!mask[s0] || seen[s0]) continue;
    let top = 0, n = 0;
    stack[top++] = s0; seen[s0] = 1;
    while (top) {
      const p = stack[--top], x = p % w, y = (p / w) | 0;
      members[n++] = p;
      for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
        const X = x + dx, Y = y + dy;
        if (X < 0 || Y < 0 || X >= w || Y >= h) continue;
        const q = Y * w + X;
        if (mask[q] && !seen[q]) { seen[q] = 1; stack[top++] = q; }
      }
    }
    each(members, n);
  }
}

// A line with its corners cut once (Chaikin), which rounds it without moving it.
function rounded(l) {
  if (l.length < 3) return l;
  const out = [l[0]];
  for (let i = 0; i < l.length - 1; i++) {
    const [ax, ay] = l[i], [bx, by] = l[i + 1];
    out.push([ax * 0.75 + bx * 0.25, ay * 0.75 + by * 0.25], [ax * 0.25 + bx * 0.75, ay * 0.25 + by * 0.75]);
  }
  out.push(l[l.length - 1]);
  return out;
}
// A line walked at even steps: [x, y, tangent x, tangent y, distance so far], and its length.
function walked(l, step) {
  const out = [];
  let need = 0, run = 0;
  for (let i = 0; i < l.length - 1; i++) {
    const [ax, ay] = l[i], [bx, by] = l[i + 1], len = Math.hypot(bx - ax, by - ay);
    if (len < 1e-6) continue;
    const tx = (bx - ax) / len, ty = (by - ay) / len;
    for (; need <= len; need += step) out.push([ax + tx * need, ay + ty * need, tx, ty, run + need]);
    need -= len; run += len;
  }
  return { pts: out, len: run };
}
// The fewest of a line's points that stay within tol of it (Douglas-Peucker).
function straightened(l, tol) {
  if (l.length < 3) return l;
  const keep = new Uint8Array(l.length), todo = [[0, l.length - 1]];
  keep[0] = keep[l.length - 1] = 1;
  while (todo.length) {
    const [a, b] = todo.pop(), [ax, ay] = l[a], [bx, by] = l[b], len = Math.hypot(bx - ax, by - ay) || 1;
    let far = 0, at = -1;
    for (let i = a + 1; i < b; i++) {
      const d = Math.abs((l[i][0] - ax) * (by - ay) - (l[i][1] - ay) * (bx - ax)) / len;
      if (d > far) { far = d; at = i; }
    }
    if (far > tol) { keep[at] = 1; todo.push([a, at], [at, b]); }
  }
  return l.filter((_, i) => keep[i]);
}
// The same for a closed ring: cut at its first point and the point furthest from it, and
// straighten the two halves.
function fewSides(ring, tol) {
  let far = 0, at = 0;
  for (let i = 1; i < ring.length; i++) { const d = Math.hypot(ring[i][0] - ring[0][0], ring[i][1] - ring[0][1]); if (d > far) { far = d; at = i; } }
  if (!at) return ring;
  const a = straightened(ring.slice(0, at + 1), tol), b = straightened(ring.slice(at).concat([ring[0]]), tol);
  return a.concat(b.slice(1, -1));
}
const addLine = (path, l) => l.forEach(([x, y], i) => (i ? path.lineTo(x, y) : path.moveTo(x, y)));

// Lettering and icons: fine bright strokes edged dark, packed close. A mask at half size.
function findLetters(f, o) {
  const [W, H] = f.full, n = W * H, { w, h } = f;
  const g = blur(f.L, W, H, 0.7), lit = lineField(g, W, H, 1).s, dark = lineField(g, W, H, -1).s, both = new Float32Array(n);
  for (let y = 1; y < H - 1; y++) for (let x = 1; x < W - 1; x++) {
    const p = y * W + x;
    let d = 0;
    for (let dy = -W; dy <= W; dy += W) for (let dx = -1; dx <= 1; dx++) if (dark[p + dy + dx] > d) d = dark[p + dy + dx];
    both[p] = Math.sqrt(Math.min(o.penCap, lit[p]) * Math.min(o.penCap, d));
  }
  const dense = blur(both, W, H, 3), mask = new Uint8Array(w * h);
  for (let y = 0; y < h * 2; y++) for (let x = 0; x < w * 2; x++) if (dense[y * W + x] > o.letters) mask[(y >> 1) * w + (x >> 1)] = 1;
  return morph(mask, w, h, 2, 1);
}

// The art's hard pen at half size: its dark line strength at full size over crestLo, capped (a pen
// line counts as one line however dark it is), averaged over each half-size pixel.
function hardPen(f, o, pen) {
  const { w, h } = f, W = f.full[0], out = new Float32Array(w * h);
  const hard = (p) => Math.min(o.crestCap, Math.max(0, pen[p] - o.crestLo));
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const p = 2 * y * W + 2 * x;
    out[y * w + x] = (hard(p) + hard(p + 1) + hard(p + W) + hard(p + W + 1)) / 4;
  }
  return out;
}

// Roads. The art draws a road as two dark lines side by side with lighter ground between. Each
// place where two such lines run alongside at a road's width votes for the point midway; a road is
// a long run of votes.
function findRoads(f, o, letters) {
  const [W, H] = f.full, { w, h } = f, n = w * h;
  const g = blur(f.L, W, H, 0.8), dk = lineField(g, W, H, -1), votes = new Float32Array(n);
  for (let y = 6; y < H - 6; y++) for (let x = 6; x < W - 6; x++) {
    const p = y * W + x;
    if (dk.s[p] <= o.roadEdge) continue;
    const c = dk.c[p], d = dk.d[p], phi = 0.5 * Math.atan2(d, c), nx = -Math.sin(phi), ny = Math.cos(phi);
    for (const gap of o.roadGap) for (const side of [1, -1]) {
      const qx = x + side * nx * gap, qy = y + side * ny * gap;
      if (between(dk.s, W, H, qx, qy) <= o.roadEdge) continue;
      if (between(dk.c, W, H, qx, qy) * c + between(dk.d, W, H, qx, qy) * d <= 0.8) continue;
      const mx = (x + qx) / 2, my = (y + qy) / 2;
      if (between(g, W, H, mx, my) <= Math.max(g[p], between(g, W, H, qx, qy)) + o.roadLift) continue;
      votes[Math.min(h - 1, Math.round(my) >> 1) * w + Math.min(w - 1, Math.round(mx) >> 1)]++;
    }
  }
  const run = blur(votes, w, h, 1.2), mask = new Uint8Array(n), kept = new Uint8Array(n);
  for (let i = 0; i < n; i++) mask[i] = run[i] > o.roadLo && !letters[i] && f.body[i] < 0.3 && f.shore[i] < 0.25 && f.edge[i] > 0.6 ? 1 : 0;
  pieces(mask, w, h, (mem, cnt) => {
    let top = 0, x0 = w, x1 = 0, y0 = h, y1 = 0;
    for (let i = 0; i < cnt; i++) {
      const p = mem[i], x = p % w, y = (p / w) | 0;
      if (run[p] > top) top = run[p];
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
    }
    if (top > o.roadHi && Math.max(x1 - x0, y1 - y0) >= o.roadLong) for (let i = 0; i < cnt; i++) kept[mem[i]] = 1;
  });
  // A road has open ground on both sides. Where the votes spread sideways instead (the grain of
  // bark, a town drawn in elevation, a banner: line after line at a road's width), it is no road:
  // cut out the stretches where most points have votes beside them.
  const lines = [], half = o.roadBeside[2] >> 1;
  for (const l of trace(kept, w, h, 8)) {
    const pts = walked(l, 1).pts, beside = pts.map(([x, y, tx, ty]) => {
      let side = 0;
      for (const k of [4.5, 6]) side = Math.max(side, between(run, w, h, x - ty * k, y + tx * k), between(run, w, h, x + ty * k, y - tx * k));
      return side > o.roadBeside[0] * between(run, w, h, x, y) ? 1 : 0;
    });
    let piece = [];
    const end = () => { if (piece.length >= o.roadLong * 0.6) lines.push(piece); piece = []; };
    for (let i = 0; i < pts.length; i++) {
      let sum = 0, cnt = 0;
      for (let j = Math.max(0, i - half); j <= Math.min(pts.length - 1, i + half); j++) { sum += beside[j]; cnt++; }
      if (sum > o.roadBeside[1] * cnt) end(); else piece.push([pts[i][0], pts[i][1]]);
    }
    end();
  }
  const cell = (x, y) => Math.min(h - 1, Math.max(0, Math.round(y))) * w + Math.min(w - 1, Math.max(0, Math.round(x)));
  const open = new Uint8Array(n);
  for (const l of lines) for (const [x, y] of l) open[cell(x, y)] = 1;
  const near = morph(open, w, h, 3, 1);
  // A road that stops just short of another joins it.
  for (const l of lines) for (const end of [0, l.length - 1]) {
    const [ex, ey] = l[end];
    let best = o.roadJoin, to = null;
    for (const m of lines) for (let i = 0; i < m.length; i++) {
      if (m === l && Math.abs(i - end) < 12) continue;
      const d = Math.hypot(m[i][0] - ex, m[i][1] - ey);
      if (d < best) { best = d; to = m[i]; }
    }
    if (to && best > 0.5) { if (end) l.push(to); else l.unshift(to); }
  }
  return { lines, near, run, pen: dk.s };
}

// Ranges: the art draws a range as rows of crest strokes in hard, dark pen (dunes and grass are
// soft strokes, and fall under crestLo), seen from the south, so the rows run across the sheet.
// The strokes' density, blurred further along the sheet than down it until the strokes of one row
// run together, has a ridge along each row; that ridge, where it runs more across than down, is
// the range's axis. pen is the art's dark line strength at full size.
function findRidges(f, o, roadNear, letters, pen) {
  const { w, h } = f, n = w * h, dens = hardPen(f, o, pen);
  for (let i = 0; i < n; i++) dens[i] = roadNear[i] || letters[i] ? 0 : dens[i] * (1 - sstep(0.02, 0.2, f.shore[i])) * (1 - f.body[i]);
  const db = blur(dens, w, h, o.ridgeBlur[0], o.ridgeBlur[1]), rf = lineField(db, w, h, 1), mask = new Uint8Array(n);
  for (let i = 0; i < n; i++) mask[i] = rf.s[i] > o.ridgeLo && rf.c[i] > o.ridgeLevel && db[i] > o.ridgeMin && f.mtn[i] > o.ridgeMtn && f.edge[i] > 0.5 ? 1 : 0;
  return { lines: trace(keepLong(mask, w, h, o.ridgeLong), w, h, o.ridgeLong), dens, db, rf, mask };
}

// The range mark, added to two paths (the crest and its shading): a zigzag of our own along the
// axis, uneven teeth that shrink toward the ends, with a few strokes falling away under each
// tooth on the side away from the light (the north-west).
function addCrest(l, crest, shade, f) {
  const { pts, len } = walked(rounded(l), 0.5);
  if (len < 4) return;
  const at = (d) => pts[Math.min(pts.length - 1, Math.max(0, Math.round(d / 0.5)))];
  let d = 0, k = 0, first = true;
  while (d < len) {
    const [x, y, tx, ty] = at(d), r1 = rand(x, y, 1), r2 = rand(x, y, 2);
    // The normal that points up the sheet; the light comes from the upper left.
    let nx = ty, ny = -tx;
    if (ny > 0 || (ny === 0 && nx > 0)) { nx = -nx; ny = -ny; }
    const taper = Math.sin((Math.PI * Math.min(len, d + 0.01)) / len) ** 0.5;
    const m = f.mtn[Math.min(f.h - 1, Math.max(0, Math.round(y))) * f.w + Math.min(f.w - 1, Math.max(0, Math.round(x)))];
    const up = k % 2 === 0, amp = (up ? 1.5 + 1.5 * r1 : -(0.5 + 0.7 * r1)) * taper * (0.6 + 0.4 * m);
    const px = x + nx * amp, py = y + ny * amp;
    if (first) { crest.moveTo(px, py); first = false; } else crest.lineTo(px, py);
    if (up && taper > 0.35) {
      // Shade strokes run down and to the right from the tooth, the way its far face falls.
      const strokes = 2 + (r2 > 0.5 ? 1 : 0);
      for (let j = 0; j < strokes; j++) {
        const sx = px + 0.9 + j * 1.1, sy = py + 0.7 + j * 0.55, sl = (2.6 - j * 0.6) * (0.7 + 0.5 * rand(x, y, 3 + j)) * taper;
        shade.moveTo(sx, sy); shade.lineTo(sx - sl * 0.45, sy + sl);
      }
    }
    d += up ? 1.6 + 1.3 * r2 : 1.9 + 1.6 * r2;
    k++;
  }
}

// Distance out to sea from the nearest land, by two chamfer passes, then smoothed.
function seaward(f) {
  const { w, h, cst } = f, n = w * h, d = new Float32Array(n), far = 40;
  for (let i = 0; i < n; i++) d[i] = cst[i] > 0.5 ? far : 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const p = y * w + x; let v = d[p];
    if (x > 0) v = Math.min(v, d[p - 1] + 1);
    if (y > 0) { v = Math.min(v, d[p - w] + 1); if (x > 0) v = Math.min(v, d[p - w - 1] + 1.414); if (x < w - 1) v = Math.min(v, d[p - w + 1] + 1.414); }
    d[p] = v;
  }
  for (let y = h - 1; y >= 0; y--) for (let x = w - 1; x >= 0; x--) {
    const p = y * w + x; let v = d[p];
    if (x < w - 1) v = Math.min(v, d[p + 1] + 1);
    if (y < h - 1) { v = Math.min(v, d[p + w] + 1); if (x < w - 1) v = Math.min(v, d[p + w + 1] + 1.414); if (x > 0) v = Math.min(v, d[p + w - 1] + 1.414); }
    d[p] = v;
  }
  return blur(d, w, h, 1.1);
}

// Islets: small patches the sheet draws as water that are plainly land in the art (warm, strong
// colour, no wash, no hatching) and clear of the drawn coast, each as its convex outline with the
// corners cut twice.
function findIslets(f, o) {
  const { w, h } = f, n = w * h, land = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    const r = f.R2[i], b = f.B2[i];
    land[i] = f.body[i] > 0.5 && f.edge[i] > 0.9 && f.faded[i] < o.fadedLo && f.wcol[i] < 0.05 && f.hb[i] < o.hlo && r > 1.5 * b && f.L2[i] > 90 ? 1 : 0;
  }
  const soft = blur(land, w, h, 0.7), mask = new Uint8Array(n), out = [];
  for (let i = 0; i < n; i++) mask[i] = soft[i] > 0.5 ? 1 : 0;
  pieces(mask, w, h, (mem, cnt) => {
    if (cnt < o.isletMin || cnt > o.isletMax) return;
    const corners = [];
    for (let i = 0; i < cnt; i++) {
      if (f.cst[mem[i]] < 0.8) return;
      const x = mem[i] % w, y = (mem[i] / w) | 0;
      corners.push([x - 0.5, y - 0.5], [x + 0.5, y - 0.5], [x - 0.5, y + 0.5], [x + 0.5, y + 0.5]);
    }
    const hl = hull(corners);
    if (cnt / area(hl) < 0.55) return;
    let ring = fewSides(hl, 0.45);
    for (let pass = 0; pass < 2; pass++) ring = ring.flatMap(([ax, ay], i) => { const [bx, by] = ring[(i + 1) % ring.length]; return [[ax * 0.75 + bx * 0.25, ay * 0.75 + by * 0.25], [ax * 0.25 + bx * 0.75, ay * 0.25 + by * 0.75]]; });
    out.push(ring);
  });
  return out;
}

// The convex outline of a set of points (Andrew's chain), in order round it.
function hull(points) {
  const p = points.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const cross = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lower = [], upper = [];
  for (const q of p) { while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], q) <= 0) lower.pop(); lower.push(q); }
  for (let i = p.length - 1; i >= 0; i--) { const q = p[i]; while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], q) <= 0) upper.pop(); upper.push(q); }
  lower.pop(); upper.pop();
  return lower.concat(upper);
}
const area = (poly) => { let a = 0; for (let i = 0; i < poly.length; i++) { const [x0, y0] = poly[i], [x1, y1] = poly[(i + 1) % poly.length]; a += x0 * y1 - x1 * y0; } return Math.abs(a) / 2; };

// Buildings. A town's art gives each building its own colour and a dark pen outline. A patch of
// ground closed off by pen or by a change of colour, small and compact, coloured unlike both the
// parchment and the ground round it, is a building; its footprint is the patch's convex outline
// cut down to a few straight sides.
// Measured at the art's size; the polygons come back in half-size pixels.
function findBuildings(f, o) {
  const [W, H] = f.full, N = W * H, { w, h } = f;
  const pen = lineField(blur(f.L, W, H, 0.8), W, H, -1).s, lab = new Int32Array(N), stack = new Int32Array(N), shut = new Uint8Array(N);
  for (let i = 0; i < N; i++) if (pen[i] > o.bldPen) shut[i] = 1;
  const one = new Float32Array(N);
  for (let c = 0; c < 3; c++) {
    for (let i = 0; i < N; i++) one[i] = f.px[i * 4 + c];
    const sm = blur(one, W, H, 0.8);
    for (let y = 1; y < H - 1; y++) for (let x = 1; x < W - 1; x++) {
      const p = y * W + x, gx = (sm[p + 1] - sm[p - 1]) / 2, gy = (sm[p + W] - sm[p - W]) / 2;
      pen[p] = (c ? pen[p] : 0) + gx * gx + gy * gy;
    }
  }
  for (let i = 0; i < N; i++) if (pen[i] > o.bldEdge * o.bldEdge) shut[i] = 1;
  const half = (p) => Math.min(h - 1, ((p / W) | 0) >> 1) * w + Math.min(w - 1, (p % W) >> 1);
  const open = (p) => !shut[p] && f.edge[half(p)] > 0.9 && f.body[half(p)] < 0.5;
  const found = [], tally = { patches: 0, sized: 0, shaped: 0, contrast: 0, filled: 0, kept: 0 };
  let id = 0;
  for (let s0 = 0; s0 < N; s0++) {
    if (lab[s0] || !open(s0)) continue;
    id++;
    let top = 0, cnt = 0, x0 = W, x1 = 0, y0 = H, y1 = 0, r = 0, g = 0, b = 0, sx = 0, sy = 0, sxx = 0, syy = 0, sxy = 0;
    const mem = [];
    stack[top++] = s0; lab[s0] = id;
    while (top) {
      const p = stack[--top], x = p % W, y = (p / W) | 0, k = half(p);
      cnt++;
      if (cnt <= o.bldMax) mem.push(p);
      r += f.R2[k]; g += f.G2[k]; b += f.B2[k];
      sx += x; sy += y; sxx += x * x; syy += y * y; sxy += x * y;
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
      for (const q of [p - 1, p + 1, p - W, p + W]) if (q >= 0 && q < N && !lab[q] && open(q)) { lab[q] = id; stack[top++] = q; }
    }
    tally.patches++;
    if (cnt < o.bldMin || cnt > o.bldMax) continue;
    tally.sized++;
    // Not a lane: about as long as it is wide, by the spread of its pixels.
    const cx = sx / cnt, cy = sy / cnt, vxx = sxx / cnt - cx * cx, vyy = syy / cnt - cy * cy, vxy = sxy / cnt - cx * cy;
    const mid = (vxx + vyy) / 2, dif = Math.hypot((vxx - vyy) / 2, vxy);
    if (Math.sqrt((mid + dif) / Math.max(0.05, mid - dif)) > o.bldLong) continue;
    tally.shaped++;
    r /= cnt; g /= cnt; b /= cnt;
    // Unlike the ground round it: what lies within a few pixels of its box and is neither it nor pen.
    let rr = 0, rg = 0, rb = 0, rn = 0;
    for (let y = Math.max(0, y0 - 5); y <= Math.min(H - 1, y1 + 5); y++) for (let x = Math.max(0, x0 - 5); x <= Math.min(W - 1, x1 + 5); x++) {
      const q = y * W + x;
      if (lab[q] === id || shut[q]) continue;
      const k = half(q);
      rr += f.R2[k]; rg += f.G2[k]; rb += f.B2[k]; rn++;
    }
    if (rn && Math.hypot(r - rr / rn, g - rg / rn, b - rb / rn) < o.bldContrast) continue;
    tally.contrast++;
    const corners = [];
    for (const q of mem) { const x = q % W, y = (q / W) | 0; corners.push([x - 0.5, y - 0.5], [x + 0.5, y - 0.5], [x - 0.5, y + 0.5], [x + 0.5, y + 0.5]); }
    const hl = hull(corners);
    if (cnt / area(hl) < o.bldFilled) continue;
    tally.filled++;
    const poly = fewSides(hl, o.bldSides);
    if (poly.length < 3) continue;
    found.push({ poly, mem, cx, cy, at: half(Math.round(cy) * W + Math.round(cx)) });
  }
  // A town is many buildings close together. The sheet's ornaments (a title ribbon, a crest) are
  // painted in the same colours and close off patches too, but only a few, off on their own:
  // buildings within reach of each other form a group, and small groups are dropped.
  const spots = new Uint8Array(w * h), group = new Int32Array(w * h), sizes = [0];
  for (const c of found) spots[c.at] = 1;
  const reach = morph(spots, w, h, o.bldReach, 1);
  pieces(reach, w, h, (mem, cnt) => { for (let i = 0; i < cnt; i++) group[mem[i]] = sizes.length; sizes.push(0); });
  for (const c of found) sizes[group[c.at]]++;
  const least = Math.max(o.bldGroup, 0.12 * Math.max(...sizes));
  const polys = [], built = new Uint8Array(w * h), town = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) if (group[i] && sizes[group[i]] >= least) town[i] = 1;
  for (const c of found) {
    if (sizes[group[c.at]] < least) continue;
    tally.kept++;
    // Out to the middle of the pen line that closes the patch, then to half size.
    polys.push(c.poly.map(([x, y]) => { const d = Math.hypot(x - c.cx, y - c.cy) || 1, k = (d + 1.2) / d; return [(c.cx + (x - c.cx) * k) / 2 - 0.25, (c.cy + (y - c.cy) * k) / 2 - 0.25]; }));
    for (const q of c.mem) built[half(q)] = 1;
  }
  return { polys, built, town, tally };
}

// Walls: the long heavy pen lines round and between the buildings that are not a building's own
// outline, each drawn as a chain of straight segments. A wall has a stretch of heavy pen; fainter
// stretches joined to it belong to it.
function findWalls(f, o, built, town, letters) {
  const { w, h } = f, n = w * h;
  const pen = lineField(blur(f.L2, w, h, 0.8), w, h, -1).s, mask = new Uint8Array(n), kept = new Uint8Array(n), off = morph(built, w, h, 1, 1);
  for (let i = 0; i < n; i++) mask[i] = pen[i] > o.wallLo && !off[i] && !letters[i] && town[i] && f.body[i] < 0.3 && f.edge[i] > 0.9 ? 1 : 0;
  pieces(mask, w, h, (mem, cnt) => {
    let top = 0, x0 = w, x1 = 0, y0 = h, y1 = 0;
    for (let i = 0; i < cnt; i++) {
      const p = mem[i], x = p % w, y = (p / w) | 0;
      if (pen[p] > top) top = pen[p];
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
    }
    if (top > o.wallHi && Math.max(x1 - x0, y1 - y0) >= o.wallLong) for (let i = 0; i < cnt; i++) kept[mem[i]] = 1;
  });
  return { lines: trace(kept, w, h, o.wallLong).map((l) => straightened(l, o.wallSides)), pen, mask };
}

export function drawDetail(lc, f, lw, o) {
  const { w, h } = f, white = (a) => `rgba(255,255,255,${a})`;
  lc.lineCap = "round"; lc.lineJoin = "round";

  // The shore: lines out to sea that follow the coast.
  const out = seaward(f), shore = o.shoreAt.map(() => new Path2D());
  isoMany(out, w, h, 1, o.shoreAt, o.shoreAt.map((_, i) => i), shore);
  lc.lineWidth = lw;
  shore.forEach((p, i) => { lc.strokeStyle = white(o.shoreAlpha[i]); lc.stroke(p); });
  if (!o.town) {
    // Open water: the edge of the shallows (where the art's wash gives way to hatched sea), then
    // the islets. A town's water is harbours and canals, which have neither.
    const wash = blur(f.wcol, w, h, 1.6), shelf = [new Path2D()], islets = new Path2D();
    for (let i = 0; i < w * h; i++) wash[i] = Math.max(wash[i], 1 - sstep(o.shelfOff, o.shelfOff + 2.5, out[i]));
    isoMany(wash, w, h, 1, [0.5], [0], shelf);
    lc.strokeStyle = white(o.shelf); lc.stroke(shelf[0]);
    for (const p of findIslets(f, o)) { addLine(islets, p); islets.closePath(); }
    lc.fillStyle = o.isletTone; lc.fill(islets);
    lc.lineWidth = lw * 1.25; lc.strokeStyle = white(o.coast); lc.stroke(islets);
  }

  const letters = findLetters(f, o), fields = { letters, mtn: f.mtn, zone: f.zone, body: f.body };
  if (o.town) {
    // A town has no ranges or roads: its pen work is buildings and walls.
    const town = findBuildings(f, o), walls = new Path2D(), bld = new Path2D();
    const wall = findWalls(f, o, town.built, town.town, letters);
    for (const l of wall.lines) addLine(walls, l);
    lc.lineJoin = "miter"; lc.lineWidth = lw * 1.1; lc.strokeStyle = white(o.wall); lc.stroke(walls);
    for (const p of town.polys) { addLine(bld, p); bld.closePath(); }
    lc.fillStyle = white(o.bldFill); lc.fill(bld);
    lc.lineWidth = lw; lc.strokeStyle = white(o.bld); lc.stroke(bld);
    Object.assign(fields, { built: town.built, pen: wall.pen, town: town.town, wallMask: wall.mask });
    drawDetail.tally = town.tally;
  } else if (o.ground) {
    const roads = findRoads(f, o, letters);
    const ridges = findRidges(f, o, roads.near, letters, roads.pen);
    const crest = new Path2D(), shade = new Path2D(), road = new Path2D();
    for (const l of ridges.lines) addCrest(l, crest, shade, f);
    lc.lineWidth = lw * 0.9; lc.strokeStyle = white(o.shade); lc.stroke(shade);
    lc.lineWidth = lw * 1.15; lc.strokeStyle = white(o.ridge); lc.stroke(crest);
    for (const l of roads.lines) addLine(road, rounded(l));
    lc.lineWidth = lw * 1.5; lc.strokeStyle = white(o.road); lc.stroke(road);
    Object.assign(fields, { road: roads.run, crest: ridges.dens, density: ridges.db, ridge: ridges.rf.s, ridgeMask: ridges.mask });
  }
  drawDetail.last = { w, h, fields };
}
