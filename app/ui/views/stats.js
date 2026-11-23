// Stats: six sections of tiles, each with its charts under them.

import { invoke, el, esc, stackedBar, dur, num, moneyHtml, lineChart, wireLineCharts, barList, money, state } from "../lib.js";
import { panel, pair, picker, statLabel } from "./chartkit.js";

function show(value) {
  const t = String(value);
  if (/^\d{4,}$/.test(t)) return num(Number(t));
  const m = t.match(/^(?:(\d+)g)? ?(?:(\d+)s)? ?(?:(\d+)c)?$/);
  if (m && (m[1] || m[3])) return moneyHtml((+m[1] || 0) * 10000 + (+m[2] || 0) * 100 + (+m[3] || 0));
  return esc(t);
}

function tiles(section) {
  const grid = el("div", "grid");
  for (const [label, value] of section.cards) {
    const c = el("div", "card");
    c.innerHTML = `<div class="value">${show(value)}</div><div class="label">${esc(label)}</div>`;
    grid.appendChild(c);
  }
  return grid;
}

function movement(data) {
  const p = panel("Moving and standing", data.time.length ? stackedBar(data.time.map((t) => [t.label, t.seconds])) : `<div class="empty">No recorded time yet.</div>`);
  p.style.marginTop = "16px";
  return p;
}

function splits(data) {
  if (!data.splits.length) return null;
  const q = panel("How the time was spent", "");
  q.style.marginTop = "12px";
  for (const [a, b] of data.splits) {
    const total = a.seconds + b.seconds || 1;
    const pct = Math.round((a.seconds / total) * 100);
    q.insertAdjacentHTML(
      "beforeend",
      `<div class="split">
        <div class="split-head"><span>${esc(a.label)} ${dur(a.seconds)}</span><span>${esc(b.label)} ${dur(b.seconds)}</span></div>
        <div class="split-track" title="${esc(a.label)}: ${pct}%"><div style="width:${pct}%"></div></div>
      </div>`,
    );
  }
  return q;
}

function chartsFor(title, data, c, tr) {
  const bars = (heading, rows, color, fmt) => panel(heading, barList(rows, color, fmt));
  switch (title) {
    case "Character":
      return [panel("Experience over time", lineChart(c.xp, "#ff2d55")), picker("Character stats over time", tr.stats, statLabel, "#ff2d55"), picker("In-game statistics over time", tr.statistics, (s) => s.name, "#ff2d55")];
    case "Combat":
      return [
        pair(bars("Top spells cast", c.spells, "#ff2d55"), bars("Damage by spell", c.damage_spells, "#ff2d55"), bars("Healing by spell", c.healing_spells, "#cdced4")),
        pair(bars("Killed most", c.kills, "#ff2d55"), bars("What killed you", c.killers, "#cdced4")),
      ];
    case "Travel and time":
      return [movement(data), splits(data), pair(bars("Zones by time", c.zones, "#ff2d55", dur))];
    case "Quests and world":
      return [pair(bars("Top buffs by uptime", c.buffs, "#cdced4", dur), bars("Cast time by spell", c.cast_time, "#ff2d55", dur))];
    case "Crafting and loot":
      return [panel("Gold over time", lineChart(c.money, "#d9b25f", 150, money)), pair(bars("Items gained", c.items_in, "#ff2d55"), bars("Items used or sold", c.items_out, "#cdced4"))];
    case "People":
      return [pair(bars("People grouped with", c.people, "#cdced4", dur), bars("Emotes used", c.emotes, "#ff2d55"))];
    default:
      return [];
  }
}

const anchor = (title) => `stats-${title.toLowerCase().replace(/[^a-z]+/g, "-")}`;

export async function render(body) {
  const [data, c, tr] = await Promise.all([invoke("stats", { ch: state.ch }), invoke("charts", { ch: state.ch }), invoke("trends", { ch: state.ch })]);
  body.innerHTML = "";
  const jump = el("div", "subnav");
  body.append(jump);
  for (const s of data.sections) {
    const a = el("a", "", esc(s.title));
    a.href = `#stats`;
    a.onclick = (e) => {
      e.preventDefault();
      body.querySelector(`#${anchor(s.title)}`)?.scrollIntoView({ block: "start" });
    };
    jump.append(a);
    const heading = el("h3", "section-title", esc(s.title));
    heading.id = anchor(s.title);
    body.append(heading, tiles(s));
    for (const node of chartsFor(s.title, data, c, tr)) if (node) body.append(node);
  }
  if (!data.sections.length) body.append(el("div", "empty", "No recorded time yet."));
  wireLineCharts(body);
}
