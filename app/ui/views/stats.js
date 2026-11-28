// Stats: the character's records, latest pace and trends, then six sections of lifetime facts, each with its charts.

import { invoke, el, elText, esc, stackedBar, stack, dur, num, moneyHtml, lineChart, wireLineCharts, barList, castRows, money, state, tile, tiles, fillRow, spark, shortDay, dayRange, displayName } from "../lib.js";
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
  const distance = t.match(/^(\d{4,}) (yd|mi)$/);
  if (distance) return `${num(Number(distance[1]))} ${distance[2]}`;
  const m = t.match(/^(?:(\d+)g)? ?(?:(\d+)s)? ?(?:(\d+)c)?$/);
  if (m && (m[1] || m[3])) return moneyHtml((+m[1] || 0) * 10000 + (+m[2] || 0) * 100 + (+m[3] || 0));
  return esc(t);
}

function facts(section) {
  const list = section.cards.map(([label, value]) => tile(label, show(value), "", { size: "md" }));
  if (!list.length) return null;
  const cols = Math.min(4, list.length);
  return tiles(fillRow(list, cols), cols);
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

function trendTile(t) {
  const rates = t.points.map((p) => p[1]);
  const last = rates[rates.length - 1];
  const view = tile(`${paceName(t.key)} an hour`, rateText(t.key, last), `Median ${rateText(t.key, median(rates))} over ${rates.length} runs at all levels, ${dayRange(t.points[0][0], t.points[t.points.length - 1][0])}`, { size: "md", extra: spark(rates.map((v) => Math.max(0, v)), { w: 240, h: 40 }) });
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
  if (recs) sheet.append(mod("w12 first last", "Records", recs));
  if (paces) sheet.append(mod("w12 first last", `Latest run, ${shortDay(latest.from)}${account && who ? `, ${displayName(who)}` : ""}`, paces));
  if (trends) sheet.append(mod("w12 first last", "Rates by run", trends));
  return sheet;
}

async function history() {
  if (state.ch) {
    const g = await invoke("goals", { ch: state.ch }).catch(() => null);
    return g ? { records: g.records, latest: g.latest, trend: g.trend, account: false, who: null } : null;
  }
  const a = await invoke("account_goals").catch(() => null);
  return a ? { records: a.records, latest: a.latest ?? null, trend: [], account: true, who: a.latest?.character ?? null } : null;
}

function movementColor(label) {
  if (/still|stand/i.test(label)) return "var(--m-still)";
  if (/swim/i.test(label)) return "var(--m-swim)";
  if (/mount|ride/i.test(label)) return "var(--m-mount)";
  if (/fly|flight|taxi/i.test(label)) return "var(--m-flight)";
  return "var(--m-foot)";
}

function movement(data) {
  if (!data.time.length) return null;
  const p = panel("Moving and standing", stackedBar(data.time.map((t) => [t.label, t.seconds, movementColor(t.label)])));
  p.style.marginTop = "16px";
  return p;
}

function splits(data) {
  const rows = data.splits.filter(([a]) => a.seconds > 0);
  if (!rows.length) return null;
  const q = panel("How the time was spent", "");
  for (const [a, b] of rows) {
    const rest = [[b.label, b.seconds, "--m-still", dur(b.seconds)]].filter((r) => r[1] > 0);
    const holder = el("div", "split");
    holder.append(stack([[a.label, a.seconds, "--s-1", dur(a.seconds)], ...rest], { h: 10 }));
    q.append(holder);
  }
  return q;
}

const ranked = (heading, rows, fmt) => {
  const list = castRows(rows);
  return list.length ? panel(heading, barList(list, "#ff2d55", fmt)) : null;
};

const side = (...panels) => {
  const kept = panels.filter(Boolean);
  return kept.length ? pair(...kept) : null;
};

const over = (heading, series, color, fmt) => (series.length ? panel(heading, lineChart(series, color, 150, fmt)) : null);

function chartsFor(title, data, c, tr) {
  switch (title) {
    case "Character":
      return [over("Experience over time", c.xp, "#ff2d55", num), picker("Character stats over time", tr.stats, statLabel, "#ff2d55"), picker("In-game statistics over time", tr.statistics, (s) => s.name, "#ff2d55")];
    case "Combat":
      return [side(ranked("Top spells cast", c.spells), ranked("Damage by spell", c.damage_spells), ranked("Healing by spell", c.healing_spells)), side(ranked("Killed most", c.kills), ranked("What killed you", c.killers))];
    case "Travel and time":
      return [movement(data), splits(data), side(ranked("Time by zone", c.zones, dur))];
    case "Quests and world":
      return [side(ranked("Top buffs by uptime", c.buffs, dur), ranked("Cast time by spell", c.cast_time, dur))];
    case "Crafting and loot":
      return [over("Gold over time", c.money, "#d9b25f", money), side(ranked("Items gained", c.items_in), ranked("Items used or sold", c.items_out))];
    case "People":
      return [side(ranked("People grouped with", c.people, dur), ranked("Emotes used", c.emotes))];
    default:
      return [];
  }
}

const anchor = (title) => `stats-${title.toLowerCase().replace(/[^a-z]+/g, "-")}`;

export async function render(body) {
  const [data, c, tr, goals] = await Promise.all([invoke("stats", { ch: state.ch }), invoke("charts", { ch: state.ch }), invoke("trends", { ch: state.ch }), history()]);
  body.innerHTML = "";
  const jump = el("div", "subnav sticky");
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
    body.append(heading);
    const cards = facts(s);
    if (cards) body.append(cards);
    for (const node of chartsFor(s.title, data, c, tr)) if (node) body.append(node);
  }
  if (!data.sections.length) body.append(el("div", "empty", "No recorded time yet."));
  const follow = () => {
    const links = [...jump.children];
    let at = 0;
    data.sections.forEach((s, i) => {
      const h = body.querySelector(`#${anchor(s.title)}`);
      if (h && h.getBoundingClientRect().top - body.getBoundingClientRect().top < 80) at = i;
    });
    links.forEach((a, i) => a.setAttribute("aria-current", i === at ? "true" : "false"));
  };
  body.onscroll = follow;
  follow();
  wireLineCharts(body);
  play(body);
}
