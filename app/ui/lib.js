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
  return i === 0 ? `${num(v)} bytes` : `${v.toFixed(1)} ${units[i]}`;
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

export function esc(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

// A cumulative series as an SVG area chart. series: [[ms, v], ...]
export function lineChart(series, color = "#ff2d55", h = 150) {
  if (!series.length) return `<div class="empty">No data yet.</div>`;
  const width = 1000;
  // A single point is held flat to the right edge.
  if (series.length === 1) series = [series[0], [series[0][0] + 1, series[0][1]]];
  const x0 = series[0][0],
    x1 = series[series.length - 1][0];
  let lo = Infinity,
    hi = -Infinity;
  for (const [, v] of series) {
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  // A value that never changed: a level line through the middle.
  const flat = lo === hi;
  const X = (t) => ((t - x0) / (x1 - x0 || 1)) * width;
  const Y = (v) => (flat ? (h - 16) / 2 : 10 + (1 - (v - lo) / (hi - lo)) * (h - 30));
  let d = `M ${X(series[0][0]).toFixed(1)} ${Y(series[0][1]).toFixed(1)}`;
  for (const [t, v] of series) d += ` L ${X(t).toFixed(1)} ${Y(v).toFixed(1)}`;
  const area = `${d} L ${X(x1).toFixed(1)} ${h - 16} L ${X(x0).toFixed(1)} ${h - 16} Z`;
  const id = "g" + Math.random().toString(36).slice(2, 8);
  return `<svg viewBox="0 0 ${width} ${h}" preserveAspectRatio="none" class="chart">
    <defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${color}" stop-opacity="0.35"/>
      <stop offset="1" stop-color="${color}" stop-opacity="0"/>
    </linearGradient></defs>
    <path d="${area}" fill="url(#${id})"/>
    <path d="${d}" fill="none" stroke="${color}" stroke-width="1.5"/>
  </svg>
  <div class="chart-scale"><span>${num(lo)}</span><span>${flat ? "unchanged" : num(hi)}</span></div>`;
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
  const total = splits.reduce((a, [, s]) => a + s, 0) || 1;
  return `<div class="stack">${splits
    .map(([label, s], i) => `<div style="flex:${s};background:${PALETTE[i % PALETTE.length]}" title="${label} ${dur(s)}"></div>`)
    .join("")}</div>
  <div class="legend">${splits
    .map(([label, s], i) => `<span><i style="background:${PALETTE[i % PALETTE.length]}"></i>${label} ${dur(s)}</span>`)
    .join("")}</div>`;
}
