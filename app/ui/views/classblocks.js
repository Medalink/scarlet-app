// The charts of a class page, drawn from the data the website answers with: one renderer for each
// block type (flow, link, mirror, stack, bag, rank, pair, split, titled, columns). The geometry of
// the flow, link and mirror charts is the website's own, so a page looks the same in both places.
// Game icons are fetched by the app (the page may not load images from the website) and set after
// the markup is in place: `hydrate` finds every element that carries a `data-icon` address.

import { invoke, esc, num, castRows } from "../lib.js";

const f1 = (v) => v.toFixed(1);

/** A number the way the website writes it in `unit`. */
export function say(unit, value) {
  const v = Number(value) || 0;
  switch (unit) {
    case "short": {
      const abs = Math.abs(v);
      if (abs >= 1_000_000) return `${(v / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
      return abs >= 1_000 ? `${num(v / 1_000)}k` : num(v);
    }
    case "decimal":
      return v.toLocaleString("en-US", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
    case "percent":
      return `${num(v)}%`;
    case "percent_tenth":
      return `${v.toLocaleString("en-US", { minimumFractionDigits: 1, maximumFractionDigits: 1 })}%`;
    case "hours":
      return Math.abs(v) < 10 ? f1(v) : num(v);
    default:
      return num(v);
  }
}

const TONES = { class: "var(--class)", scarlet: "var(--accent)", lost: "#7b7b88", held: "var(--fg)" };

const colorOf = (tone, hue, fallback = "var(--accent)") => (hue ? hue : tone ? (TONES[tone] ?? fallback) : fallback);

const icons = new Map();
let waiting = 0;
const queue = [];
const IN_FLIGHT = 6;

function pump() {
  while (waiting < IN_FLIGHT && queue.length) {
    const next = queue.shift();
    waiting += 1;
    invoke("class_icon", { url: next.url })
      .then(next.resolve, () => next.resolve(null))
      .finally(() => {
        waiting -= 1;
        pump();
      });
  }
}

function iconFor(url) {
  if (!icons.has(url)) icons.set(url, new Promise((resolve) => (queue.push({ url, resolve }), pump())));
  return icons.get(url);
}

/** Sets the image of every `data-icon` element under `root`: an `<img>`, an SVG `<image>` or a slot. */
export function hydrate(root) {
  for (const node of root.querySelectorAll("[data-icon]")) {
    const url = node.dataset.icon;
    iconFor(url).then((data) => {
      if (!data || !node.isConnected) return;
      if (node instanceof HTMLImageElement) node.src = data;
      else if (node instanceof SVGImageElement) node.setAttribute("href", data);
      else node.style.backgroundImage = `url("${data}")`;
      node.classList.add("loaded");
    });
  }
}

const pic = (url, cls = "ic") => (url ? `<img class="${cls}" data-icon="${esc(url)}" alt="" width="22" height="22">` : "");

/** The icon at the start of a row; an empty square keeps the labels in line while another row has one. */
export const iconTag = (url, any = false) => (url ? pic(url) : any ? `<i class="ic none" aria-hidden="true"></i>` : "");

const FLOW = { width: 900, pool: 300, leftX: 204, poolX: 445, rightX: 686, node: 10, leftGap: 18, rightGap: 5, slotMin: 26, icon: 20, top: 40 };

const ribbon = (x0, a0, b0, x1, a1, b1) => {
  const m = (x0 + x1) / 2;
  return `M${x0} ${f1(a0)}C${m} ${f1(a0)} ${m} ${f1(a1)} ${x1} ${f1(a1)}L${x1} ${f1(b1)}C${m} ${f1(b1)} ${m} ${f1(b0)} ${x0} ${f1(b0)}Z`;
};

const pin = (url, x, mid, dim) =>
  url ? `<image class="fi${dim ? " dim" : ""}" data-icon="${esc(url)}" x="${f1(x)}" y="${f1(mid - FLOW.icon / 2)}" width="${FLOW.icon}" height="${FLOW.icon}"/>` : "";

function flow(block) {
  const F = FLOW;
  const left = block.left;
  const right = block.right;
  const total = left.reduce((sum, item) => sum + Math.max(0, item.count), 0);
  const rightSum = right.reduce((sum, item) => sum + Math.max(0, item.count), 0);
  if (total <= 0 || !left.length) return "";
  const scale = F.pool / total;
  const rightScale = rightSum > 0 ? total / rightSum : 0;
  const lh = left.map((item) => Math.max(0, item.count) * scale);
  const rh = right.map((item) => Math.max(0, item.count) * rightScale * scale);
  const slots = rh.map((h) => Math.max(h, F.slotMin));
  const leftHeight = lh.reduce((a, b) => a + b, 0) + F.leftGap * (lh.length - 1);
  const rightHeight = slots.reduce((a, b) => a + b, 0) + F.rightGap * Math.max(0, slots.length - 1);
  const body = Math.max(F.pool, leftHeight, rightHeight);
  const poolTop = F.top + (body - F.pool) / 2;
  let out = "";
  let y = F.top + (body - leftHeight) / 2;
  let t = poolTop;
  left.forEach((item, i) => {
    const h = lh[i];
    const mid = y + h / 2;
    const tx = F.leftX - 20 - F.icon;
    out += `<path class="fl ${item.tone}" d="${ribbon(F.leftX + F.node, y, y + h, F.poolX, t, t + h)}"/><rect class="fn ${item.tone}" x="${F.leftX}" y="${f1(y)}" width="${F.node}" height="${f1(Math.max(h, 1.6))}" rx="1.5"/>${pin(item.icon, F.leftX - 12 - F.icon, mid, false)}<text class="fl-l" x="${tx}" y="${f1(mid - 2)}" text-anchor="end">${esc(item.label)}</text><text class="fl-v" x="${tx}" y="${f1(mid + 14)}" text-anchor="end">${num(item.count)} · ${num(Math.round((item.count / total) * 100))}%</text>`;
    y += h + F.leftGap;
    t += h;
  });
  y = F.top + (body - rightHeight) / 2;
  t = poolTop;
  right.forEach((item, i) => {
    const h = rh[i];
    const a = y + (slots[i] - h) / 2;
    const drawn = Math.max(h, 1.6);
    const mid = y + slots[i] / 2;
    out += `<path class="fl ${item.tone}" d="${ribbon(F.poolX + F.node, t, t + h, F.rightX, a, a + drawn)}"/><rect class="fn ${item.tone}" x="${F.rightX}" y="${f1(a)}" width="${F.node}" height="${f1(drawn)}" rx="1.5"/>${pin(item.icon, F.rightX + F.node + 10, mid, item.tone === "lost")}<text x="${F.rightX + F.node + 18 + F.icon}" y="${f1(mid + 4.5)}"><tspan class="fl-l">${esc(item.label)}</tspan><tspan class="fl-v" dx="9">${num(item.count)}</tspan></text>`;
    y += slots[i] + F.rightGap;
    t += h;
  });
  out += `<rect class="fn held" x="${F.poolX}" y="${f1(poolTop)}" width="10" height="300" rx="1.5"/><text class="fl-n" x="${F.poolX + 5}" y="${f1(poolTop - 13)}" text-anchor="middle">${num(total)} ${esc(block.unit)}</text>`;
  return `<div class="plot flow"><svg viewBox="0 0 ${F.width} ${f1(F.top + body + 12)}" role="img" aria-label="${esc(block.label)}">${out}</svg></div>`;
}

const LINK = { width: 900, body: 300, leftX: 204, rightX: 686, node: 10, leftGap: 14, rightGap: 10, slotMin: 26, icon: 20, top: 40 };

function linkTops(slots, heights, gap, all, body) {
  let y = LINK.top + (body - all) / 2;
  return slots.map((slot, i) => {
    const top = y + (slot - heights[i]) / 2;
    y += slot + gap;
    return top;
  });
}

function link(block) {
  const L = LINK;
  const pairs = block.pairs.filter((p) => block.left[p.left] && block.right[p.right] && p.count > 0);
  if (!pairs.length) return "";
  const total = pairs.reduce((sum, p) => sum + p.count, 0);
  const leftSums = block.left.map(() => 0);
  const rightSums = block.right.map(() => 0);
  for (const p of pairs) {
    leftSums[p.left] += p.count;
    rightSums[p.right] += p.count;
  }
  const scale = L.body / total;
  const lh = leftSums.map((s) => s * scale);
  const rh = rightSums.map((s) => s * scale);
  const ls = lh.map((h) => Math.max(h, L.slotMin));
  const rs = rh.map((h) => Math.max(h, L.slotMin));
  const leftTotal = ls.reduce((a, b) => a + b, 0) + L.leftGap * (ls.length - 1);
  const rightTotal = rs.reduce((a, b) => a + b, 0) + L.rightGap * (rs.length - 1);
  const body = Math.max(leftTotal, rightTotal);
  const leftTops = linkTops(ls, lh, L.leftGap, leftTotal, body);
  const rightTops = linkTops(rs, rh, L.rightGap, rightTotal, body);
  const leftAt = [...leftTops];
  const rightAt = [...rightTops];
  const fromLeft = new Map();
  const fromRight = new Map();
  const index = pairs.map((_, i) => i);
  for (const i of [...index].sort((a, b) => pairs[a].left - pairs[b].left || pairs[a].right - pairs[b].right)) {
    fromLeft.set(i, leftAt[pairs[i].left]);
    leftAt[pairs[i].left] += pairs[i].count * scale;
  }
  for (const i of [...index].sort((a, b) => pairs[a].right - pairs[b].right || pairs[a].left - pairs[b].left)) {
    fromRight.set(i, rightAt[pairs[i].right]);
    rightAt[pairs[i].right] += pairs[i].count * scale;
  }
  const m = (L.leftX + L.node + L.rightX) / 2;
  const x0 = L.leftX + L.node;
  let out = pairs
    .map((p, i) => {
      const h = p.count * scale;
      const y0 = fromLeft.get(i);
      const y1 = fromRight.get(i);
      return `<path class="fl ${block.right[p.right].tone}" d="M${x0} ${f1(y0)}C${m} ${f1(y0)} ${m} ${f1(y1)} ${L.rightX} ${f1(y1)}L${L.rightX} ${f1(y1 + h)}C${m} ${f1(y1 + h)} ${m} ${f1(y0 + h)} ${x0} ${f1(y0 + h)}Z"/>`;
    })
    .join("");
  const share = (sum) => `${num(sum)} · ${num(Math.round((sum / total) * 100))}%`;
  block.left.forEach((item, i) => {
    const mid = leftTops[i] + lh[i] / 2;
    const tx = L.leftX - 20 - L.icon;
    out += `<rect class="fn class" x="${L.leftX}" y="${f1(leftTops[i])}" width="${L.node}" height="${f1(Math.max(lh[i], 1.6))}" rx="1.5"/>${pin(item.icon, L.leftX - 12 - L.icon, mid, false)}<text class="fl-l" x="${tx}" y="${f1(mid - 2)}" text-anchor="end">${esc(item.label)}</text><text class="fl-v" x="${tx}" y="${f1(mid + 14)}" text-anchor="end">${share(leftSums[i])}</text>`;
  });
  block.right.forEach((item, i) => {
    const mid = rightTops[i] + rh[i] / 2;
    out += `<rect class="fn ${item.tone}" x="${L.rightX}" y="${f1(rightTops[i])}" width="${L.node}" height="${f1(Math.max(rh[i], 1.6))}" rx="1.5"/>${pin(item.icon, L.rightX + L.node + 10, mid, item.tone === "lost")}<text x="${L.rightX + L.node + 18 + L.icon}" y="${f1(mid + 4.5)}"><tspan class="fl-l">${esc(item.label)}</tspan><tspan class="fl-v" dx="9">${share(rightSums[i])}</tspan></text>`;
  });
  out += `<text class="fl-n" x="${(L.leftX + L.rightX + L.node) / 2}" y="27" text-anchor="middle">${num(total)} ${esc(block.unit)}</text>`;
  return `<div class="plot flow"><svg viewBox="0 0 ${L.width} ${f1(L.top + body + 12)}" role="img" aria-label="${esc(block.label)}">${out}</svg></div>`;
}

const NICE = [1, 2, 3, 4, 5, 6, 8, 10];

function niceMax(value) {
  const power = 10 ** Math.floor(Math.log10(value));
  return (NICE.find((step) => step * power >= value) ?? 10) * power;
}

function mirrorLevels(first, last) {
  if (last - first < 12) return Array.from({ length: last - first + 1 }, (_, i) => first + i);
  const tens = [];
  for (let level = Math.ceil(first / 10) * 10; level <= last; level += 10) tens.push(level);
  return tens.length ? tens : [...new Set([first, last])];
}

function mirror(block) {
  const W = 760;
  const H = 290;
  const left = 44;
  const points = block.points;
  const top = points.reduce((m, p) => Math.max(m, p.up, p.down), 0);
  if (!points.length || top <= 0) return "";
  const max = niceMax(top);
  const half = (H - 10 - 24) / 2;
  const mid = 10 + half;
  const first = points[0].level;
  const last = points[points.length - 1].level;
  const span = last - first + 1;
  const plot = W - left - 6;
  const x = (level) => left + ((level - (first - 0.5)) / span) * plot;
  const barWidth = (plot / span) * 0.72;
  let out = "";
  for (const tick of [max, max / 2, 0, -max / 2, -max]) {
    const y = mid - (tick / max) * half;
    out += `<line class="${tick === 0 ? "axis" : "grid"}" x1="44" x2="754" y1="${f1(y)}" y2="${f1(y)}"/><text x="36" y="${f1(y + 3.5)}" text-anchor="end">${tick === 0 ? "0" : say(block.unit, Math.abs(tick))}</text>`;
  }
  for (const level of mirrorLevels(first, last)) out += `<text x="${f1(x(level))}" y="284" text-anchor="middle">${level}</text>`;
  const peak = points.reduce((best, p) => (p.up > best.up ? p : best), points[0]);
  for (const p of points) {
    const up = (p.up / max) * half;
    const down = (p.down / max) * half;
    const on = p === peak ? " on" : "";
    out += `<rect class="bar up${on}" style="fill:${block.up.hue}" x="${f1(x(p.level) - barWidth / 2)}" y="${f1(mid - up)}" width="${f1(barWidth)}" height="${f1(up)}" rx="1.5"/><rect class="bar down${on}" style="fill:${block.down.hue}" x="${f1(x(p.level) - barWidth / 2)}" y="${f1(mid + 1)}" width="${f1(barWidth)}" height="${f1(down)}" rx="1.5"/>`;
  }
  out += `<text class="lab" x="${f1(x(peak.level))}" y="${f1(mid - (peak.up / max) * half - 6)}" text-anchor="middle">level ${peak.level}</text>`;
  return `<div class="plot taps"><svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(block.label)}">${out}</svg></div>${key([block.up, block.down])}`;
}

const key = (series) => `<ul class="key">${series.map((s) => `<li><i style="background:${s.hue}"></i>${esc(s.label)}</li>`).join("")}</ul>`;

function stack(block) {
  if (!block.columns.length) return "";
  const many = block.columns.length > 12;
  const labelled = (column, i) => typeof column.label !== "number" || !many || i === 0 || column.label % 10 === 0;
  const bars = block.columns
    .map((column) => {
      const cells = block.series
        .map((s, i) => `<i style="height:${f1(Math.max(0, Math.min(1, column.shares[i] ?? 0)) * 100)}%;background:${s.hue}"></i>`)
        .join("");
      return `<div class="xp-bar">${cells}</div>`;
    })
    .join("");
  const axis = block.columns.map((column, i) => `<span>${labelled(column, i) ? esc(column.label) : ""}</span>`).join("");
  return `<div class="xp-plot"><div class="xp-y" aria-hidden="true"><span>100%</span><span>50%</span><span>0</span></div><div class="xp-bars" role="img" aria-label="${esc(block.label)}">${bars}</div><div class="xp-x" aria-hidden="true">${axis}</div></div>${key(block.series)}`;
}

function bag(block) {
  if (block.slots <= 0) return "";
  const held = [];
  for (const item of block.items) for (let i = 0; i < item.slots && held.length < block.slots; i += 1) held.push(item.icon);
  let slots = "";
  for (let i = 0; i < block.slots; i += 1) {
    if (i >= held.length) slots += `<i class="slot"></i>`;
    else slots += held[i] ? `<i class="slot held pic" data-icon="${esc(held[i])}"></i>` : `<i class="slot held"></i>`;
  }
  return `<div class="bagslots" role="img" aria-label="${held.length} of ${block.slots} bag slots hold a ${esc(block.noun)}">${slots}</div>`;
}

function rank(block) {
  const list = castRows(block.rows);
  if (!list.length) return "";
  const any = list.some((row) => row.icon);
  const full = block.of ?? Math.max(0, ...list.map((row) => row.value));
  const rows = list
    .map((row) => {
      const share = row.share ?? (full > 0 ? row.value / full : 0);
      const color = row.tone || row.hue ? `;background:${colorOf(row.tone, row.hue)}` : "";
      const aside = row.aside ? ` <small class="pet">${esc(row.aside)}</small>` : "";
      return `<li><span class="who">${iconTag(row.icon, any)}${esc(row.label)}${aside}</span><span class="val">${esc(row.text)}</span><span class="track"><i style="width:${share > 0 ? f1(Math.max(1, Math.min(100, share * 100))) : 0}%${color}"></i></span></li>`;
    })
    .join("");
  return `<ol class="rank${block.plain ? " plain" : ""}">${rows}</ol>`;
}

function pair(block) {
  const cards = block.rows
    .map((row) => `<div class="card"><div class="value">${esc(row.text)}${row.small ? `<small>${esc(row.small)}</small>` : ""}</div><div class="label">${esc(row.label)}</div></div>`)
    .join("");
  return `<div class="grid cv-pair">${cards}</div>`;
}

function split(block) {
  const bar = block.parts.map((part) => `<i style="flex:${part.value};background:${colorOf(part.tone, part.hue)}"></i>`).join("");
  const names = block.parts.map((part) => `<li><i style="background:${colorOf(part.tone, part.hue)}"></i>${esc(part.label)}</li>`).join("");
  return `<div class="cv-split" aria-hidden="true">${bar}</div><ul class="key">${names}</ul>`;
}

function titled(block) {
  return `<h4 class="cv-chart-name">${pic(block.icon)}${esc(block.title)}</h4>${blocks(block.blocks)}`;
}

function columns(block) {
  return `<div class="cv-cols" style="--n:${block.columns.length}">${block.columns.map((column) => `<div>${titled(column)}</div>`).join("")}</div>`;
}

const DRAW = { flow, link, mirror, stack, bag, rank, pair, split, titled, columns };

/** The markup of a list of blocks. */
export function blocks(list) {
  return list.map((block) => (DRAW[block.type] ? DRAW[block.type](block) : "")).join("");
}
