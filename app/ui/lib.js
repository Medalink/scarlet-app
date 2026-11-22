// Shared helpers: invoke wrapper, app state, formatting, small chart builders.

export const invoke = window.__TAURI__.core.invoke;

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

export function money(c) {
  c = Math.round(c);
  const sign = c < 0 ? "-" : "";
  const a = Math.abs(c);
  const g = Math.floor(a / 10000),
    s = Math.floor(a / 100) % 100,
    p = a % 100;
  if (g > 0) return `${sign}${num(g)}g ${s}s`;
  if (s > 0) return `${sign}${s}s ${p}c`;
  return `${sign}${p}c`;
}

// Coins coloured like the game's: "68g 24s". `signed` adds + or −.
export function moneyHtml(c, signed = false) {
  c = Math.round(c);
  const v = Math.abs(c);
  const g = Math.floor(v / 10000),
    s = Math.floor((v % 10000) / 100),
    p = v % 100;
  const parts = [];
  if (g) parts.push(`${num(g)}<i class="g">g</i>`);
  if (g || s) parts.push(`${g ? String(s).padStart(2, "0") : s}<i class="s">s</i>`);
  if (!g) parts.push(`${s ? String(p).padStart(2, "0") : p}<i class="c">c</i>`);
  const sign = signed ? (c < 0 ? "−" : "+") : c < 0 ? "−" : "";
  return `<span class="money">${sign}${parts.join("")}</span>`;
}

export function dur(secs) {
  secs = Math.max(0, Math.floor(secs));
  const h = Math.floor(secs / 3600),
    m = Math.floor(secs / 60) % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${secs % 60}s`;
  return `${secs}s`;
}

export function clock(ms) {
  const d = new Date(ms);
  const p = (x, n = 2) => String(x).padStart(n, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(Math.floor((ms % 1000) / 10))}`;
}

export function dateStr(ms) {
  const d = new Date(ms);
  const p = (x) => String(x).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function dateTime(ms) {
  return `${dateStr(ms)} ${clock(ms).slice(0, 5)}`;
}

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
export const SERIES = ["#ff2d55", "#9a7df2", "#b98b2d", "#2f9ec4", "#6e6e7a"];

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

// A cumulative series as an SVG area chart. series: [[ms, v], ...]
export function lineChart(series, color = "#ff2d55", h = 150, fmt = num) {
  if (!series.length) return `<div class="empty">No data yet.</div>`;
  const width = 1000;
  // A single point is held flat to the right edge.
  const pts = series.length === 1 ? [series[0], [series[0][0] + 1, series[0][1]]] : series;
  const x0 = pts[0][0],
    x1 = pts[pts.length - 1][0];
  let lo = Infinity,
    hi = -Infinity;
  for (const [, v] of pts) {
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  // A value that never changed: a level line through the middle.
  const flat = lo === hi;
  const X = (t) => ((t - x0) / (x1 - x0 || 1)) * width;
  const Y = (v) => (flat ? (h - 16) / 2 : 10 + (1 - (v - lo) / (hi - lo)) * (h - 30));
  let d = `M ${X(pts[0][0]).toFixed(1)} ${Y(pts[0][1]).toFixed(1)}`;
  for (const [t, v] of pts) d += ` L ${X(t).toFixed(1)} ${Y(v).toFixed(1)}`;
  const area = `${d} L ${X(x1).toFixed(1)} ${h - 16} L ${X(x0).toFixed(1)} ${h - 16} Z`;
  const id = `lc${++chartSeq}`;
  if (chartStore.size > 64) chartStore.clear();
  chartStore.set(id, { pts: series, x0, x1, lo, hi, flat, h, fmt, color });
  return `<div class="chart-wrap" data-chart-id="${id}">
    <svg viewBox="0 0 ${width} ${h}" preserveAspectRatio="none" class="chart">
      <defs><linearGradient id="${id}-g" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="${color}" stop-opacity="0.22"/>
        <stop offset="1" stop-color="${color}" stop-opacity="0"/>
      </linearGradient></defs>
      <path d="${area}" fill="url(#${id}-g)"/>
      <path d="${d}" fill="none" stroke="${color}" stroke-width="2" stroke-linejoin="round" vector-effect="non-scaling-stroke"/>
      <line class="chart-cross" y1="0" y2="${h - 16}" stroke="rgba(240,241,243,0.45)" stroke-width="1" vector-effect="non-scaling-stroke" visibility="hidden"/>
    </svg>
    <div class="chart-scale"><span>${fmt(lo)}</span><span>${flat ? "unchanged" : fmt(hi)}</span></div>
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
      out.innerHTML = `<b>${dateTime(t)}</b> · ${esc(fmt(v))}`;
    });
    svg.addEventListener("pointerleave", () => {
      cross.setAttribute("visibility", "hidden");
      out.textContent = "";
    });
  }
}

// Horizontal bar rows. rows: [[label, value], ...]
export function barList(rows, color = "#ff2d55", fmt = num) {
  if (!rows.length) return `<div class="empty">No data yet.</div>`;
  const max = Math.max(...rows.map((r) => r[1]), 1);
  return `<div class="bars">${rows
    .map(
      ([label, v]) => `<div class="bar-row">
        <div class="bar-label" title="${esc(label)}">${esc(label)}</div>
        <div class="bar-track"><div class="bar-fill" style="width:${(v / max) * 100}%;background:${color}"></div></div>
        <div class="bar-value">${fmt(v)}</div>
      </div>`
    )
    .join("")}</div>`;
}

// Horizontal stacked bar with legend. splits: [[label, seconds], ...]
export function stackedBar(splits) {
  return `<div class="stack">${splits
    .map(([label, s], i) => `<div style="flex:${s};background:${SERIES[i % SERIES.length]}" title="${esc(label)} ${dur(s)}"></div>`)
    .join("")}</div>
  <div class="legend">${splits
    .map(([label, s], i) => `<span><i style="background:${SERIES[i % SERIES.length]}"></i>${esc(label)} ${dur(s)}</span>`)
    .join("")}</div>`;
}
