// Stats: the character's records, latest pace and trends, then six sections of lifetime facts, each with its charts.

import { invoke, el, elText, esc, stackedBar, dur, num, moneyHtml, lineChart, wireLineCharts, barList, money, state, tile, tiles, fillRow, spark, shortDate, displayName } from "../lib.js";
import { play } from "../motion.js";
import { panel, pair, picker, statLabel } from "./chartkit.js";
import { mod, recordTiles } from "./goals-section.js";
import { openAs, paceTiles, paceName, rateText } from "./paces.js";

const RECORD_KEYS = ["xp", "coin", "loot", "kills", "quests", "played"];
const PACE_KEYS = ["xp_h", "gold_h", "kills_h", "gathers_h", "quests_h", "deaths_h"];
const TREND_KEYS = ["xp_h", "gold_h", "kills_h", "quests_h", "deaths_h", "gathers_h"];

function show(value) {
  const t = String(value);
  if (/^\d{4,}$/.test(t)) return num(Number(t));
  const m = t.match(/^(?:(\d+)g)? ?(?:(\d+)s)? ?(?:(\d+)c)?$/);
  if (m && (m[1] || m[3])) return moneyHtml((+m[1] || 0) * 10000 + (+m[2] || 0) * 100 + (+m[3] || 0));
  return esc(t);
}

function facts(section) {
  const list = el("dl", "facts");
  for (const [label, value] of section.cards) list.append(el("div", "", `<dt>${esc(label)}</dt><dd>${show(value)}</dd>`));
  return list;
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

function trendTile(t) {
  const rates = t.points.map((p) => p[1]);
  const last = rates[rates.length - 1];
  const view = tile(`${paceName(t.key)} an hour`, rateText(t.key, last), `Median ${rateText(t.key, median(rates))} over ${rates.length} runs, ${shortDate(t.points[0][0])} to ${shortDate(t.points[t.points.length - 1][0])}`, { size: "md", extra: spark(rates.map((v) => Math.max(0, v)), { w: 240, h: 40 }) });
  view.classList.add("stat-trend");
  return view;
}

function trendRow(trend) {
  const list = TREND_KEYS.map((key) => trend.find((t) => t.key === key)).filter(Boolean).map(trendTile);
  return list.length ? tiles(fillRow(list, 3), 3) : null;
}

function goalsSheet({ records, latest, trend, account, who }) {
  const sheet = el("div", "sheet goals-sheet stats-goals");
  const recs = recordTiles(records, { keys: RECORD_KEYS, open: (r) => (account ? openAs(r.character) : null), owner: account ? (r) => displayName(r.character) : null });
  const paces = latest ? paceTiles(latest.paces, latest.played, { keys: PACE_KEYS, cols: 3, onOpen: account && who ? openAs(who) : null }) : null;
  const trends = trendRow(trend);
  sheet.append(mod("w12 first last", "Records", recs ?? elText("p", "sc-empty", "No session of 15 minutes recorded yet")));
  if (paces) sheet.append(mod("w12 first last", `Latest run, ${shortDate(latest.from)}${account && who ? `, ${displayName(who)}` : ""}`, paces));
  if (trends) sheet.append(mod("w12 first last", "Rates by run", trends));
  return sheet;
}

async function history() {
  if (state.ch) {
    const g = await invoke("goals", { ch: state.ch }).catch(() => null);
    return g ? { records: g.records, latest: g.latest, trend: g.trend, account: false, who: null } : null;
  }
  const a = await invoke("account_goals").catch(() => null);
  return a ? { records: a.records, latest: a.latest?.latest ?? null, trend: [], account: true, who: a.latest?.character ?? null } : null;
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
  const [data, c, tr, goals] = await Promise.all([invoke("stats", { ch: state.ch }), invoke("charts", { ch: state.ch }), invoke("trends", { ch: state.ch }), history()]);
  body.innerHTML = "";
  const jump = el("div", "subnav");
  body.append(jump);
  if (goals && data.sections.length) body.append(goalsSheet(goals));
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
    body.append(heading, facts(s));
    for (const node of chartsFor(s.title, data, c, tr)) if (node) body.append(node);
  }
  if (!data.sections.length) body.append(el("div", "empty", "No recorded time yet."));
  wireLineCharts(body);
  play(body);
}
