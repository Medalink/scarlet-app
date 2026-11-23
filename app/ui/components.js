// Components of DESIGN.md: the same drawn parts as the website and the addon, built from fills, lines and SVG paths.
// lib.js re-exports everything here.

import { el, elText, num, dur, displayName } from "./lib.js";
import { GLYPHS, svg } from "./glyphs.js";
import { drawCrest } from "./crests.js";
import { BY_KEY } from "./feats-catalog.js";

const HALF_MILE_YD = 880;
const YARDS_PER_MILE = 1760;

export const distanceHtml = (yd) => (yd < HALF_MILE_YD ? `${num(yd)}<small>yd</small>` : `${(yd / YARDS_PER_MILE).toFixed(1)}<small>mi</small>`);
export const distanceText = (yd) => (yd < HALF_MILE_YD ? `${num(yd)} yd` : `${(yd / YARDS_PER_MILE).toFixed(1)} mi`);

let tipEl = null;

/** The hover readout shared by every chart: "title|detail" at the pointer, or hidden with no text. */
export function tip(ev, text) {
  if (!tipEl) {
    tipEl = el("div", "sc-tip");
    document.body.append(tipEl);
  }
  if (!text) {
    tipEl.classList.remove("on");
    return;
  }
  const [a, b] = text.split("|");
  tipEl.replaceChildren(document.createTextNode(a), ...(b ? [elText("small", "", b)] : []));
  tipEl.classList.add("on");
  const w = tipEl.offsetWidth;
  tipEl.style.left = `${Math.max(8, Math.min(innerWidth - w - 8, ev.clientX + 14))}px`;
  tipEl.style.top = `${ev.clientY + 16}px`;
}

let tipsWired = false;

/** Elements with a `data-tip` attribute show it on hover. */
export function wireTips() {
  if (tipsWired) return;
  tipsWired = true;
  const at = (ev) => ev.target.closest?.("[data-tip]");
  document.addEventListener("mouseover", (ev) => {
    const t = at(ev);
    if (t) tip(ev, t.dataset.tip);
  });
  document.addEventListener("mousemove", (ev) => {
    const t = at(ev);
    if (t && !t.closest("svg[data-own]")) tip(ev, t.dataset.tip);
  });
  document.addEventListener("mouseout", (ev) => {
    if (at(ev)) tip();
  });
}

/** A stat tile: label, the number (HTML so a unit can sit in a `<small>`), one note line. */
export function tile(label, value, note, { size = "", best = "", extra = null } = {}) {
  const t = el("div", "sc-tile");
  t.append(elText("span", "sc-label", label));
  t.append(el("span", `sc-num${size ? ` sc-num-${size}` : ""}`, value));
  if (note) t.append(elText("span", "sc-sub", note));
  if (extra) t.append(extra);
  if (best) t.append(elText("span", "best", best));
  return t;
}

/** Tiles on one hairline grid; a tile is never a separate card. */
export function tiles(list, cols = 4) {
  const g = el("div", "sc-grid");
  g.style.gridTemplateColumns = `repeat(${cols}, minmax(0, 1fr))`;
  g.append(...list);
  return g;
}

/** The progress ring: one arc filled clockwise from the top for `value` (0 to 1), `inner` in the middle. */
export function progressRing(value, { size = 120, color = "#ff2d55", inner = null } = {}) {
  const host = el("span", "sc-ring");
  host.style.width = host.style.height = `${size}px`;
  const root = svg("svg", { viewBox: "0 0 100 100", width: size, height: size });
  const share = Math.round(Math.max(0, Math.min(1, value)) * 10000) / 100;
  const sw = size < 60 ? 10 : 5;
  const r = 50 - sw / 2;
  root.append(svg("circle", { cx: 50, cy: 50, r, fill: "none", stroke: "rgba(255,255,255,0.10)", "stroke-width": sw }));
  if (share > 0) {
    root.append(svg("circle", {
      class: "arc", cx: 50, cy: 50, r, fill: "none", stroke: color, "stroke-width": sw, "stroke-linecap": "round",
      pathLength: 100, "stroke-dasharray": `${share} 100`, transform: "rotate(-90 50 50)",
    }));
  }
  host.append(root);
  if (inner) {
    const c = el("span", "sc-ring-in");
    c.append(inner);
    host.append(c);
  }
  return host;
}

/** A sparkline (a 2 px line with an end dot) or, with `form: "cols"`, columns with the newest in scarlet. */
export function spark(values, { w = 96, h = 26, form = "line", color = "#ff2d55" } = {}) {
  const max = Math.max(...values, 1);
  const root = svg("svg", { viewBox: `0 0 ${w} ${h}`, width: w, height: h, class: "sc-spark" });
  if (form === "cols") {
    const bw = (w - (values.length - 1) * 2) / values.length;
    values.forEach((x, i) =>
      root.append(
        svg("rect", {
          x: i * (bw + 2), y: h - Math.max(1, (x / max) * h), width: bw, height: Math.max(1, (x / max) * h),
          fill: i === values.length - 1 ? color : x ? "rgba(255,255,255,0.3)" : "rgba(255,255,255,0.1)",
        }),
      ),
    );
  } else if (values.length > 1) {
    const pts = values.map((x, i) => [(i / (values.length - 1)) * (w - 6) + 3, h - 3 - (x / max) * (h - 6)]);
    root.append(svg("polyline", { points: pts.map((p) => p.join(",")).join(" "), fill: "none", stroke: color, "stroke-width": 2, "stroke-linejoin": "round", "stroke-linecap": "round" }));
    const last = pts[pts.length - 1];
    root.append(svg("circle", { cx: last[0], cy: last[1], r: 3, fill: color, stroke: "#141416", "stroke-width": 2 }));
  }
  return root;
}

/** Stacked bar with its key. rows: [[label, value, cssVar, text?]], drawn in the order given with 2 px gaps. */
export function stack(rows, { key = true, h = 12 } = {}) {
  const host = el("div");
  const sum = rows.reduce((a, r) => a + r[1], 0) || 1;
  const bar = el("div", "sc-stack");
  bar.style.height = `${h}px`;
  const legend = el("div", "sc-key");
  for (const [label, value, c, text] of rows) {
    const i = el("i");
    i.style.flex = String(value);
    i.style.setProperty("--c", `var(${c})`);
    i.dataset.tip = `${label}|${text || num(value)} · ${Math.round((value / sum) * 100)}%`;
    bar.append(i);
    const k = el("span");
    k.style.setProperty("--c", `var(${c})`);
    k.append(`${label} `, elText("b", "", text || num(value)));
    legend.append(k);
  }
  host.append(bar);
  if (key) host.append(legend);
  return host;
}

/** The design's bar list: label left, value right, a 4 px bar under both. rows: [[label, value, text?, cssVar?]]. */
export function bars(rows) {
  const host = el("div", "sc-bars");
  const max = Math.max(...rows.map((r) => r[1]), 1);
  for (const [label, value, text, c] of rows) {
    const row = el("div", "row");
    const i = el("i");
    i.style.setProperty("--w", `${(value / max) * 100}%`);
    if (c) i.style.setProperty("--c", `var(${c})`);
    row.append(elText("span", "", label), elText("b", "", text ?? num(value)), i);
    host.append(row);
  }
  return host;
}

/**
 * The compare strip: a hairline axis, the 10th to 90th percentile as a thin band, the middle half as a thick one, the
 * median tick and your scarlet diamond. `spread` is `{ p10, p25, p50, p75, p90, you }`; the axis runs `lo` to `hi`.
 */
export function compare(spread, { lo = 0, hi, name = "You", text } = {}) {
  const top = hi ?? (Math.max(spread.p90, spread.you) * 1.1 || 1);
  const x = (v) => `${Math.max(0, Math.min(100, ((v - lo) / (top - lo)) * 100))}%`;
  const w = (a, b) => `${Math.max(0, ((b - a) / (top - lo)) * 100)}%`;
  const mk = (cls, left, width) => {
    const e = el("span", cls);
    e.style.left = left;
    if (width) e.style.width = width;
    return e;
  };
  const host = el("span", "sc-cmp");
  host.append(mk("axis", 0), mk("wide", x(spread.p10), w(spread.p10, spread.p90)), mk("mid", x(spread.p25), w(spread.p25, spread.p75)), mk("med", x(spread.p50)), mk("you", x(spread.you)));
  host.dataset.tip = `${name}: ${text ?? spread.you}|Middle half ${spread.p25} to ${spread.p75} · median ${spread.p50}`;
  return host;
}

const clockOf = (sec) => {
  const d = new Date(sec * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

/**
 * The session strip: zones in order, XP per bucket, combat, moments and the clock. Drawn from the recap's `buckets`,
 * `zones`, `levels` and `moments`; `compact` drops the clock row and the labels, as on session cards.
 */
export function strip(r, { compact = false } = {}) {
  const W = 960;
  const secs = r.buckets.secs || 300;
  const xp = r.buckets.xp;
  const spanS = Math.max(r.end - r.start, xp.length * secs, 60);
  const x = (s) => (s / spanS) * W;
  const zh = compact ? 15 : 20;
  const ph = compact ? 26 : 46;
  const py = zh + 8;
  const cy = py + ph + 4;
  const ey = cy + 11;
  const H = ey + (compact ? 18 : 22) + (compact ? 0 : 16);
  const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, width: "100%", class: "sc-svg", "data-own": "" });
  root.style.aspectRatio = `${W} / ${H}`;
  r.zones.forEach((z, i) => {
    const x0 = x(z.at);
    const wd = x(z.at + z.secs) - x0;
    root.append(svg("rect", { x: x0 + (i ? 1 : 0), y: 0, width: Math.max(1, wd - (i ? 1 : 0)), height: zh, fill: i % 2 ? "rgba(255,255,255,0.07)" : "rgba(255,255,255,0.12)" }));
    if (wd > z.name.length * 6.4 + 12) {
      const t = svg("text", { x: x0 + 7, y: zh / 2 + 4, class: "strong" });
      t.textContent = z.name;
      root.append(t);
    }
  });
  const max = Math.max(...xp, 1);
  const bw = x(secs);
  xp.forEach((v, i) => {
    const h = Math.max(1, (v / max) * ph);
    root.append(svg("rect", { x: x(i * secs) + 1, y: py + ph - h, width: Math.max(1, bw - 2), height: h, fill: v === max ? "#ff2d55" : "rgba(255,45,85,0.62)" }));
  });
  root.append(svg("rect", { x: 0, y: py + ph, width: W, height: 1, fill: "rgba(255,255,255,0.13)" }));
  r.buckets.combat.forEach((c, i) => {
    if (c > 0) root.append(svg("rect", { x: x(i * secs), y: cy, width: Math.max(1.5, bw * Math.min(1, c / secs)), height: 3, fill: "#c0102e" }));
  });
  for (const lv of r.levels) {
    const ex = x(lv.at);
    root.append(svg("rect", { x: ex - 0.5, y: 0, width: 1, height: ey + 4, fill: "rgba(240,241,243,0.45)" }));
    root.append(svg("rect", { x: ex - 5, y: ey + 3, width: 10, height: 10, fill: "#f0f1f3", transform: `rotate(45 ${ex} ${ey + 8})` }));
    if (!compact) {
      const t = svg("text", { x: ex + 11, y: ey + 12, class: "strong" });
      t.textContent = String(lv.level);
      root.append(t);
    }
  }
  for (const m of r.moments) {
    const ex = x(m.at);
    if (m.kind === "death") {
      const g = svg("g", { transform: `translate(${ex - 7} ${ey + 1}) scale(0.6)`, fill: "none", stroke: "#f0f1f3", "stroke-width": 2.2, "stroke-linecap": "round", "stroke-linejoin": "round" });
      g.innerHTML = GLYPHS.deaths;
      root.append(g);
    } else if (m.kind === "feat") {
      const cat = BY_KEY.get(m.feat);
      const host = document.createElement("span");
      Object.assign(host.dataset, { crest: cat?.glyph ?? "kills", scope: cat?.crest ?? "character", tier: m.tier || 2, size: 18, progress: 0, label: "" });
      drawCrest(host);
      const mark = host.firstChild;
      mark.setAttribute("x", String(ex - 9));
      mark.setAttribute("y", String(ey - 1));
      root.append(mark);
    } else {
      root.append(svg("rect", { x: ex - 1.5, y: ey + 6.5, width: 3, height: 3, fill: "#8a8a93" }));
    }
  }
  if (!compact) {
    const lab = (s, text, anchor) => {
      const t = svg("text", { x: x(s), y: H - 2, "text-anchor": anchor || "middle" });
      t.textContent = text;
      root.append(t);
    };
    lab(0, clockOf(r.start), "start");
    lab(spanS, clockOf(r.start + spanS), "end");
    const next = new Date(r.start * 1000);
    next.setMinutes(0, 0, 0);
    next.setHours(next.getHours() + 1);
    for (let t = next.getTime() / 1000; t < r.start + spanS - 480; t += 3600) {
      if (t - r.start > 840) lab(t - r.start, `${String(new Date(t * 1000).getHours()).padStart(2, "0")}:00`);
    }
  }
  const cross = svg("rect", { x: -9, y: 0, width: 1, height: ey + 20, fill: "#f0f1f3", opacity: 0.5 });
  root.append(cross);
  root.addEventListener("mousemove", (ev) => {
    const b = root.getBoundingClientRect();
    const mx = ((ev.clientX - b.left) / b.width) * W;
    const s = Math.max(0, Math.min(spanS, (mx / W) * spanS));
    cross.setAttribute("x", String(x(s)));
    const zone = [...r.zones].reverse().find((z) => z.at <= s)?.name ?? r.zone;
    const near = [...r.moments, ...r.levels.map((l) => ({ at: l.at, text: `Level ${l.level} reached` }))].find((m) => Math.abs(m.at - s) < spanS / 90 && m.text);
    const i = Math.min(xp.length - 1, Math.floor(s / secs));
    tip(ev, `${clockOf(r.start + s)} · ${zone}|${near ? near.text : `${num(xp[i] ?? 0)} XP in five minutes`}`);
  });
  root.addEventListener("mouseleave", () => {
    cross.setAttribute("x", "-9");
    tip();
  });
  return root;
}

/**
 * The recorded path on a hairline grid: scarlet, newest brightest, flights dashed violet, a start ring, an end diamond
 * and a diamond at each level. `route.sheet` is the shape the addon reads: x, y pairs with the long side at 1000 and y
 * up, `aspect` (width over height), `breaks` (the 1-based point where each leg after the first starts) and `air` (true
 * at a point reached by a flight).
 */
export function routeCard(route, { levels = [], height = 356 } = {}) {
  const W = 640;
  const H = height;
  const root = svg("svg", { viewBox: `0 0 ${W} ${H}`, width: "100%", class: "sc-svg", preserveAspectRatio: "xMidYMid slice" });
  root.style.height = `${H}px`;
  for (let gx = 40; gx < W; gx += 40) root.append(svg("rect", { x: gx, y: 0, width: 1, height: H, fill: "rgba(255,255,255,0.035)" }));
  for (let gy = 40; gy < H; gy += 40) root.append(svg("rect", { x: 0, y: gy, width: W, height: 1, fill: "rgba(255,255,255,0.035)" }));
  const sheet = route?.sheet;
  if (!sheet || sheet.points.length < 4) return root;
  const pad = 34;
  const aspect = sheet.aspect || 1;
  const boxW = Math.min(W - 2 * pad, (H - 2 * pad) * aspect);
  const boxH = boxW / aspect;
  const ox = (W - boxW) / 2;
  const oy = (H - boxH) / 2;
  const unit = boxW / (aspect >= 1 ? 1000 : 1000 * aspect);
  const pts = [];
  for (let i = 0; i + 1 < sheet.points.length; i += 2) pts.push([ox + sheet.points[i] * unit, oy + boxH - sheet.points[i + 1] * unit]);
  const starts = new Set((sheet.breaks ?? []).map((b) => b - 1));
  const air = sheet.air ?? [];
  const runs = [];
  for (let i = 1; i < pts.length; i++) {
    if (starts.has(i)) continue;
    const flown = Boolean(air[i]);
    const run = runs[runs.length - 1];
    if (run && run.flown === flown && run.end === i - 1) {
      run.pts.push(pts[i]);
      run.end = i;
    } else {
      runs.push({ flown, pts: [pts[i - 1], pts[i]], end: i });
    }
  }
  const step = Math.max(2, Math.ceil(pts.length / 6));
  for (const run of runs) {
    const size = run.flown ? run.pts.length : step;
    for (let at = 0; at < run.pts.length - 1; at += size - 1) {
      const seg = run.pts.slice(at, at + size);
      const age = (run.end - run.pts.length + 1 + at + seg.length - 1) / Math.max(1, pts.length - 1);
      root.append(
        svg("polyline", {
          points: seg.map((p) => p.map((n) => n.toFixed(1)).join(",")).join(" "), fill: "none",
          stroke: run.flown ? "#9a7df2" : "#ff2d55", "stroke-width": run.flown ? 1.6 : 2, "stroke-dasharray": run.flown ? "5 5" : null,
          "stroke-linejoin": "round", "stroke-linecap": "round", opacity: Math.min(1, 0.38 + age * 0.62),
        }),
      );
    }
  }
  const first = pts[0];
  const last = pts[pts.length - 1];
  root.append(svg("circle", { cx: first[0], cy: first[1], r: 4.5, fill: "#141416", stroke: "#f0f1f3", "stroke-width": 1.8 }));
  for (const lv of levels) {
    const p = pts[Math.min(pts.length - 1, Math.round(lv.frac * (pts.length - 1)))];
    root.append(svg("rect", { x: p[0] - 4.5, y: p[1] - 4.5, width: 9, height: 9, fill: "#f0f1f3", stroke: "#141416", "stroke-width": 1.5, transform: `rotate(45 ${p[0]} ${p[1]})` }));
  }
  root.append(svg("rect", { x: last[0] - 5.5, y: last[1] - 5.5, width: 11, height: 11, fill: "#ff2d55", stroke: "#141416", "stroke-width": 2, transform: `rotate(45 ${last[0]} ${last[1]})` }));
  return root;
}

/**
 * A collection row: the name, `6 of 11`, then every feat as a crest at 48 with its name. `draw.crest(feat)` draws one; `onPick(feat)` runs on a click.
 */
export function collectionRow(name, feats, { draw, onPick = null, picked = null, all = feats }) {
  const row = el("section", "sc-coll");
  const earned = all.filter((f) => f.tier > 0).length;
  const head = el("div");
  head.append(elText("h3", "sc-coll-h", name));
  const n = el("p", "sc-coll-n");
  n.append(`${earned} of ${all.length}`);
  head.append(n);
  const wall = el("ul", "sc-wall");
  for (const f of feats) {
    const li = el("li", `t${f.tier}${picked === f.key ? " on" : ""}`);
    const btn = el("button", "sc-wall-b");
    btn.type = "button";
    btn.title = `${f.name}${f.counted ? `: ${f.counted}` : ""}`;
    btn.append(draw.crest(f), elText("span", "", f.name));
    if (onPick) btn.onclick = () => onPick(f);
    li.append(btn);
    wall.append(li);
  }
  row.append(head, wall);
  return row;
}

const dayOf = (sec) => new Date(sec * 1000).toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" });

/**
 * A session card: date line, the title, the compact strip, one line of totals and a chip for each feat earned.
 * `onOpen` makes the whole card a link.
 */
export function sessionCard(r, { onOpen = null, who = true, chip = null } = {}) {
  const card = el(onOpen ? "a" : "div", "sc-session");
  if (onOpen) {
    card.href = "#";
    card.onclick = (e) => {
      e.preventDefault();
      onOpen();
    };
  }
  const head = el("p", "sc-session-d");
  head.append(`${dayOf(r.start)} · ${clockOf(r.start)} to ${clockOf(r.end)}`);
  if (who) head.append(` · ${displayName(r.character)}`);
  card.append(head, elText("h3", "sc-session-t", r.title.text));
  if (r.buckets?.xp?.length) card.append(strip(r, { compact: true }));
  const parts = [dur(r.played)];
  const count = (n, one, many) => `${num(n)} ${n === 1 ? one : many}`;
  if (r.counters?.kills) parts.push(count(r.counters.kills, "kill", "kills"));
  if (r.counters?.quests) parts.push(count(r.counters.quests, "quest", "quests"));
  const xp = Object.values(r.xp).reduce((a, v) => a + v, 0);
  if (xp) parts.push(`${num(xp)} XP`);
  card.append(elText("p", "sc-session-n", parts.join(" · ")));
  if (chip && r.feats?.earned?.length) {
    const row = el("div", "sc-session-f");
    for (const f of r.feats.earned) row.append(chip(f));
    card.append(row);
  }
  return card;
}

/** One hairline row: a sentence and one quiet button. Never a modal. */
export function linkRow(text, label, onClick, { hide = null } = {}) {
  const row = el("div", "sc-link");
  row.append(elText("span", "", text));
  const b = elText("button", "sc-btn quiet small", label);
  b.type = "button";
  b.onclick = onClick;
  row.append(b);
  if (hide) {
    const h = elText("button", "linkbtn muted", "Hide");
    h.type = "button";
    h.onclick = hide;
    row.append(h);
  }
  return row;
}
