// Shared helpers: invoke wrapper, app state, formatting, small chart builders.

import { invoke, listen } from "./bridge.js";
import { clockTime, dayTime, shortDay } from "./dates.js";

export { invoke, listen };

export const state = {
  ch: null, // selected character or null for all
  codes: new Set(), // timeline code filter
  store: null, // last app_state payload
};

export const el = (tag, cls, html) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (html !== undefined) e.innerHTML = html;
  return e;
};

export const elText = (tag, cls, text) => {
  const e = el(tag, cls);
  if (text !== undefined) e.textContent = text;
  return e;
};

export const $ = (sel) => document.querySelector(sel);

// Appends to `into` why OpenMap's art is not there, from `map_art_status`: the WoW folder, then
// _forever_ in it, then OpenMap in its AddOns; the addon's page is linked where installing it helps.
export function openmapMissing(into, st) {
  const link = () => {
    const a = el("button", "linkbtn", "github.com/openmap-world/openmap");
    a.type = "button";
    a.onclick = () => invoke("open_openmap_page").catch(() => {});
    return a;
  };
  if (st.reason === "no-wow") into.append("OpenMap needs your WoW folder. Set it in Settings → General.");
  else if (st.reason === "no-forever") into.append("The WoW folder has no _forever_ folder, so Scarlet cannot find OpenMap.");
  else if (st.reason === "broken-addon") into.append("OpenMap is in AddOns without its map files. Reinstall it from ", link(), ".");
  else into.append("OpenMap is not installed. Get it at ", link(), ".");
}

export function num(n) {
  return Math.round(n)
    .toString()
    .replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

// A byte count in the largest unit it reaches: 99,508 -> "97.2 KB".
export function bytes(n) {
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  let v = Number(n) || 0;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return i === 0 ? `${num(v)} ${v === 1 ? "byte" : "bytes"}` : `${v.toFixed(1)} ${units[i]}`;
}

const coinParts = (c) => {
  const v = Math.abs(Math.round(c));
  const g = Math.floor(v / 10000);
  const s = Math.floor((v % 10000) / 100);
  const p = v % 100;
  const parts = [];
  if (g) parts.push(["g", num(g)]);
  if (s) parts.push(["s", String(s)]);
  if (p && !g) parts.push(["c", String(p)]);
  return parts.length ? parts : [["c", "0"]];
};

export function money(c) {
  const sign = Math.round(c) < 0 ? "−" : "";
  return sign + coinParts(c).map(([unit, n]) => `${n}${unit}`).join(" ");
}

// Coins coloured like the game's: "68g 24s". Units that are zero are left out. `signed` adds + or −.
export function moneyHtml(c, signed = false) {
  c = Math.round(c);
  const sign = signed ? (c < 0 ? "−" : "+") : c < 0 ? "−" : "";
  return `<span class="money">${sign}${coinParts(c).map(([unit, n]) => `${n}<i class="${unit}">${unit}</i>`).join("")}</span>`;
}

// A time span: "40s", "2m 5s", "21h 48m", "3d 2h". Never a leading zero unit; a zero trailing unit is dropped.
export function dur(secs) {
  secs = Math.max(0, Math.floor(secs));
  const d = Math.floor(secs / 86400),
    h = Math.floor(secs / 3600) % 24,
    m = Math.floor(secs / 60) % 60,
    s = secs % 60;
  const two = (a, an, b, bn) => (b ? `${a}${an} ${b}${bn}` : `${a}${an}`);
  if (d > 0) return two(d, "d", h, "h");
  if (secs >= 3600) return two(h, "h", m, "m");
  if (secs >= 60) return two(m, "m", s, "s");
  return `${secs}s`;
}

// Round ticks for a value axis from `lo` up to the first round value at or above `hi`: [0, 500, 1000, 1500, 2000].
export function niceTicks(hi, count = 4, lo = 0) {
  if (!(hi > lo)) return [lo];
  const rough = (hi - lo) / count;
  const pow = 10 ** Math.floor(Math.log10(rough));
  const unit = [1, 2, 5, 10].map((k) => k * pow).find((u) => u >= rough);
  const ticks = [];
  for (let v = Math.floor(lo / unit) * unit; ; v += unit) {
    ticks.push(Math.round(v * 1e6) / 1e6);
    if (v >= hi - unit * 1e-9) break;
  }
  return ticks;
}

// The level axis of every chart: 1, 10, 20 ... up to `top`.
export function levelTicks(top, first = 1) {
  const ticks = new Set([first]);
  for (let l = Math.ceil((first + 1) / 10) * 10; l <= top; l += 10) ticks.add(l);
  return [...ticks];
}

// Cast lists leave out auto attacks and the game's internal spells.
const NOT_CASTS = /^(attack|auto attack|auto shot|shoot|throw|melee|loginEffect)$/i;
export const castRows = (rows) => (rows ?? []).filter((r) => !NOT_CASTS.test(String(Array.isArray(r) ? r[0] : (r.name ?? r.label ?? "")).trim()));

// Palette for session / series colours, scarlet family first.
export const PALETTE = [
  "#ff2d55",
  "#ff6b8e",
  "#c0102e",
  "#ffa1b6",
  "#8a2540",
  "#ff4d6d",
  "#d9637f",
  "#b84a5e",
];

// Chart series: kills, dungeon kills, quests, exploring, other.
export const SERIES = ["#ff2d55", "#9a7df2", "#98721c", "#2f9ec4", "#54545d"];

export function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

export const displayName = (name) => String(name ?? "").replace("-", " ");

export function setSwitch(btn, on) {
  const active = Boolean(on);
  btn.classList.toggle("on", active);
  btn.setAttribute("aria-checked", String(active));
}

export function makeSwitch(label, on = false, locked = false) {
  const s = el("button", "switch");
  s.type = "button";
  s.setAttribute("role", "switch");
  s.innerHTML = `<span class="track" aria-hidden="true"><span class="knob"></span></span><span class="lab"></span>`;
  s.querySelector(".lab").textContent = label;
  setSwitch(s, on);
  if (locked) {
    s.classList.add("locked");
    s.disabled = true;
  }
  return s;
}

export function highlight(text, query) {
  const s = String(text ?? "");
  const q = String(query ?? "").trim();
  if (!q) return esc(s);
  const re = new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi");
  let out = "";
  let last = 0;
  for (const m of s.matchAll(re)) {
    out += esc(s.slice(last, m.index)) + `<mark>${esc(m[0])}</mark>`;
    last = m.index + m[0].length;
  }
  return out + esc(s.slice(last));
}

const chartStore = new Map();
let chartSeq = 0;

const POINT_DOTS = 10;

const axisDays = (x0, x1) => {
  const span = x1 - x0;
  if (span < 86_400_000) return [x0, x1].map((t) => clockTime(t));
  return [x0, x0 + span / 2, x1].map((t) => shortDay(t));
};

// A series as an SVG area chart with value ticks on the left and days along the bottom; under ten points each one is a dot. series: [[ms, v], ...]
export function lineChart(series, color = "#ff2d55", h = 150, fmt = num) {
  if (!series.length) return `<div class="empty">No data yet.</div>`;
  const width = 1000;
  const pts = series.length === 1 ? [series[0], [series[0][0] + 1, series[0][1]]] : series;
  const x0 = pts[0][0],
    x1 = pts[pts.length - 1][0];
  let lo = Infinity,
    hi = -Infinity;
  for (const [, v] of pts) {
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  const flat = lo === hi;
  const ticks = flat ? [lo] : niceTicks(hi, 3, lo);
  const yLo = flat ? lo : ticks[0];
  const yHi = flat ? lo : ticks[ticks.length - 1];
  const pad = 8;
  const plot = h - 2 * pad;
  const yPct = (v) => (flat ? 50 : (pad + (1 - (v - yLo) / (yHi - yLo || 1)) * plot) / h * 100);
  const X = (t) => ((t - x0) / (x1 - x0 || 1)) * width;
  const Y = (v) => (yPct(v) / 100) * h;
  let d = `M ${X(pts[0][0]).toFixed(1)} ${Y(pts[0][1]).toFixed(1)}`;
  for (const [t, v] of pts) d += ` L ${X(t).toFixed(1)} ${Y(v).toFixed(1)}`;
  const area = `${d} L ${X(x1).toFixed(1)} ${h} L ${X(x0).toFixed(1)} ${h} Z`;
  const id = `lc${++chartSeq}`;
  if (chartStore.size > 64) chartStore.clear();
  chartStore.set(id, { pts: series, x0, x1, lo, hi, flat, h, fmt, color });
  const grid = ticks.map((v) => `<line x1="0" x2="${width}" y1="${Y(v).toFixed(1)}" y2="${Y(v).toFixed(1)}" stroke="rgba(255,255,255,0.07)" vector-effect="non-scaling-stroke"/>`).join("");
  const labels = ticks.map((v) => `<span style="top:${yPct(v).toFixed(2)}%">${fmt(v)}</span>`).join("");
  const dots = series.length < POINT_DOTS ? series.map(([t, v]) => `<i class="chart-dot" style="left:${(X(t) / width) * 100}%;top:${yPct(v).toFixed(2)}%;background:${color}"></i>`).join("") : "";
  const days = axisDays(x0, x1).map((t) => `<span>${t}</span>`).join("");
  return `<div class="chart-wrap" data-chart-id="${id}">
    <div class="chart-frame">
      <div class="chart-y">${labels}</div>
      <div class="chart-plot">
        <svg viewBox="0 0 ${width} ${h}" preserveAspectRatio="none" class="chart" style="height:${h}px">
          <defs><linearGradient id="${id}-g" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stop-color="${color}" stop-opacity="0.22"/>
            <stop offset="1" stop-color="${color}" stop-opacity="0"/>
          </linearGradient></defs>
          ${grid}
          <path d="${area}" fill="url(#${id}-g)"/>
          <path d="${d}" fill="none" stroke="${color}" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke"/>
          <line class="chart-cross" y1="0" y2="${h}" stroke="rgba(240,241,243,0.45)" stroke-width="1" vector-effect="non-scaling-stroke" visibility="hidden"/>
        </svg>
        ${dots}
      </div>
    </div>
    <div class="chart-x">${days}</div>
    <p class="chart-readout"></p>
  </div>`;
}

export function wireLineCharts(root) {
  for (const wrap of root.querySelectorAll(".chart-wrap[data-chart-id]")) {
    const info = chartStore.get(wrap.dataset.chartId);
    if (!info || wrap.dataset.wired) continue;
    wrap.dataset.wired = "1";
    const svg = wrap.querySelector("svg.chart");
    const cross = wrap.querySelector(".chart-cross");
    const out = wrap.querySelector(".chart-readout");
    const { pts, x0, x1, fmt } = info;
    const span = x1 - x0 || 1;
    svg.addEventListener("pointermove", (e) => {
      const r = svg.getBoundingClientRect();
      if (!r.width) return;
      const frac = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
      const targetT = x0 + frac * span;
      let lo = 0, hi = pts.length - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (pts[mid][0] < targetT) lo = mid + 1;
        else hi = mid;
      }
      if (lo > 0 && Math.abs(pts[lo - 1][0] - targetT) <= Math.abs(pts[lo][0] - targetT)) lo -= 1;
      const [t, v] = pts[lo];
      const x = ((t - x0) / span) * 1000;
      cross.setAttribute("x1", x.toFixed(1));
      cross.setAttribute("x2", x.toFixed(1));
      cross.setAttribute("visibility", "visible");
      out.innerHTML = `<b>${dayTime(t)}</b> · ${fmt(v)}`;
    });
    svg.addEventListener("pointerleave", () => {
      cross.setAttribute("visibility", "hidden");
      out.textContent = "";
    });
  }
}

const SMALLEST_SCALE = 5;

// Horizontal bar rows. rows: [[label, value], ...]. A list of small counts is drawn against at least 5, so one cast is not a full bar.
export function barList(rows, color = "#ff2d55", fmt = num) {
  if (!rows.length) return `<div class="empty">No data yet.</div>`;
  const max = Math.max(...rows.map((r) => r[1]), fmt === num ? SMALLEST_SCALE : 1);
  const sum = rows.reduce((a, r) => a + r[1], 0) || 1;
  return `<div class="bars">${rows
    .map(
      ([label, v]) => `<div class="bar-row" title="${Math.round((v / sum) * 100)}% of the list">
        <div class="bar-label" title="${esc(label)}">${esc(label)}</div>
        <div class="bar-track"><div class="bar-fill" style="width:${(v / max) * 100}%;background:${color}"></div></div>
        <div class="bar-value">${fmt(v)}</div>
      </div>`
    )
    .join("")}</div>`;
}

// Horizontal stacked bar with legend. splits: [[label, seconds, cssColor?], ...]
export function stackedBar(splits) {
  const color = (c, i) => c ?? SERIES[i % SERIES.length];
  return `<div class="stack">${splits
    .map(([label, s, c], i) => `<div style="flex:${s};background:${color(c, i)}" title="${esc(label)} ${dur(s)}"></div>`)
    .join("")}</div>
  <div class="legend">${splits
    .map(([label, s, c], i) => `<span><i style="background:${color(c, i)}"></i>${esc(label)} ${dur(s)}</span>`)
    .join("")}</div>`;
}

export * from "./components.js";
export * from "./dates.js";
