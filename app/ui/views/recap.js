// The session recap of recap-app.html, drawn from `run_summary(...).recaps`: head, route and tiles, strip, feats and
// comparison, time and experience, counts, moments, quests and loot.

import { invoke, el, elText, state, num, dur, money, moneyHtml, displayName, distanceHtml, distanceText, tile, tiles, stack, bars, compare, strip, routeCard, wireTips, clockTime, dayHeading, dayTime } from "../lib.js";
import { levelsOf, paceName, paceTiles } from "./paces.js";
import { BY_KEY } from "../feats-catalog.js";
import { GLYPHS, TIER_NAMES, svg } from "../glyphs.js";
import { crest, tierChip, countText, amount } from "../featcard.js";
import { webState } from "../link.js";
import { counted, play } from "../motion.js";

const YARDS_PER_MILE = 1760;
const clockOf = (sec) => clockTime(sec * 1000);
const dayOf = (sec) => dayHeading(sec * 1000);

const panel = (...kids) => {
  const p = el("section", "sc-panel");
  p.append(...kids.filter(Boolean));
  return p;
};
const heading = (text) => elText("h2", "sc-h", text);
const label = (text) => elText("p", "sc-label", text);

function legend(items) {
  const key = el("div", "sc-key");
  for (const [text, color] of items) {
    const s = el("span");
    s.style.setProperty("--c", color);
    s.append(text);
    key.append(s);
  }
  return key;
}

const RUN_GAP_SECS = 300;

function runsIn(sessions, character) {
  const mine = sessions.filter((x) => x.char_name === character).sort((a, b) => a.start - b.start);
  let runs = 0;
  let end = -Infinity;
  for (const x of mine) {
    if (x.start > end + RUN_GAP_SECS * 1000) runs += 1;
    end = Math.max(end, x.end);
  }
  return runs;
}

function subline(recap, run, nav) {
  const line = el("p", "recap-sub");
  line.append(elText("b", "", displayName(recap.character)));
  const played = counted(elText("span", "", dur(recap.played)), recap.played, dur);
  if (nav.exact && nav.mode === "day") {
    const n = runsIn(run.sessions, recap.character);
    line.append(` · ${n} ${n === 1 ? "session" : "sessions"} · `, played, " played");
  } else {
    line.append(` · ${dayOf(recap.start)} · ${clockOf(recap.start)} to ${nav.exact && nav.current?.live ? "now" : clockOf(recap.end)} · `, played, " played");
  }
  return line;
}

function pickerLabel(c, mode) {
  const names = c.chars.map(displayName).join(", ");
  const when = mode === "day" ? dayOf(c.from / 1000) : dayTime(c.from);
  return [when, c.live ? "in progress" : dur(c.played), names].join(" · ");
}

function picker(nav, from) {
  const select = el("select");
  select.setAttribute("aria-label", "Session");
  for (const c of nav.cards) select.append(new Option(pickerLabel(c, nav.mode), String(c.from)));
  select.value = String(nav.current.from);
  select.onchange = () => {
    const c = nav.cards.find((x) => String(x.from) === select.value);
    if (c) window.__nav(`sessions/${c.from}-${c.to}`);
  };
  return select;
}

function head(recap, paces, run, nav, onAll, onWeb) {
  const box = el("div", "recap-head");
  const text = el("div");
  text.append(elText("h1", "sc-title", nav.exact && nav.mode === "day" ? dayOf(recap.start) : recap.title.text), subline(recap, run, nav));
  const records = paces.filter((p) => p.newBest && p.isBest);
  if (records.length) {
    const marks = el("p", "recap-new");
    for (const p of records) marks.append(elText("span", "best new", `New best: ${paceName(p.key).toLowerCase()} an hour${levelsOf(p.band)}`));
    text.append(marks);
  }
  const actions = el("div", "recap-actions");
  const from = nav.current?.from ?? run.from;
  const older = nav.cards.find((c) => c.from < from);
  const newer = [...nav.cards].reverse().find((c) => c.from > from);
  const go = (c, text, label) => {
    const b = elText("button", "sc-btn quiet small", text);
    b.type = "button";
    b.setAttribute("aria-label", label);
    b.onclick = () => window.__nav(`sessions/${c.from}-${c.to}`);
    return b;
  };
  if (older) actions.append(go(older, `← ${dayTime(older.from)}`, "Older session"));
  if (nav.cards.length) actions.append(picker(nav, from));
  if (newer) actions.append(go(newer, `${dayTime(newer.from)} →`, "Newer session"));
  const log = elText("button", "sc-btn quiet small", "Log");
  log.type = "button";
  log.onclick = () => {
    window.__tlSession = { index: null, from: recap.start * 1000 - 1000, to: recap.end * 1000 + 1000 };
    window.__nav("log");
  };
  actions.append(log);
  const all = elText("button", "sc-btn quiet small", "All sessions");
  all.type = "button";
  all.onclick = onAll;
  actions.append(all);
  if (onWeb) {
    const web = elText("button", "sc-btn quiet small", "Open on the website ");
    web.type = "button";
    web.append(svg("svg", { viewBox: "0 0 24 24", width: 14, height: 14, fill: "none", stroke: "currentColor", "stroke-width": 1.9, "stroke-linecap": "round", "stroke-linejoin": "round" }));
    web.lastChild.innerHTML = GLYPHS.arrow;
    web.onclick = () => invoke("open_session_on_web", { character: recap.character, start: recap.start, end: recap.end }).catch(() => {});
    actions.append(web);
  }
  box.append(text, actions);
  return box;
}

function routePanel(recap) {
  const box = el("div", "sc-panel recap-route");
  box.style.height = "356px";
  if (!recap.route) {
    box.append(elText("p", "sc-empty", "No path was recorded."));
    return box;
  }
  const span = Math.max(1, recap.end - recap.start);
  const levels = recap.levels.map((l) => ({ frac: Math.min(1, l.at / span) }));
  box.append(routeCard(recap.route, { levels, reveal: true }));
  const flown = recap.route.sheet.air?.includes(true);
  box.append(legend([["Walked, newest brightest", "var(--s-1)"], ...(flown ? [["Teleported", "var(--m-flight)"]] : [])]));
  return box;
}

const totalXp = (recap) => Object.values(recap.xp).reduce((a, v) => a + v, 0);

function tileGrid(recap, paces) {
  const c = recap.counters;
  const xp = totalXp(recap);
  const list = [];
  const paced = (key) => paces.some((p) => p.key === key);
  if (xp && !paced("xp_h")) list.push(tile("Experience", num(xp), "", { size: "xl", count: [xp, num] }));
  if (c.kills && !paced("kills_h")) list.push(tile("Kills", num(c.kills), "", { size: "xl", count: [c.kills, num] }));
  const net = (c.earned ?? 0) - (c.spent ?? 0);
  if ((c.earned || c.spent) && !paced("gold_h")) {
    list.push(tile("Gold", moneyHtml(net, true), `Earned ${moneyText(c.earned ?? 0)} · spent ${moneyText(c.spent ?? 0)}`, { size: "xl", count: [Math.abs(net), (v) => moneyHtml(Math.sign(net) * v, true)] }));
  }
  const yards = (c.foot ?? 0) + (c.walk ?? 0) + (c.mount ?? 0) + (c.swim ?? 0) + (c.fly ?? 0);
  if (yards) {
    const modes = [["Swimming", c.swim ?? 0, "--m-swim", "swum"], ["On foot", (c.foot ?? 0) + (c.walk ?? 0), "--m-foot", "on foot"], ["Mounted", (c.mount ?? 0) + (c.fly ?? 0), "--m-mount", "mounted"]].filter((m) => m[1] > 0);
    const noteOrder = ["on foot", "mounted", "swum"];
    const note = modes.map(([, yd, , word]) => [word, yd]).sort((a, b) => noteOrder.indexOf(a[0]) - noteOrder.indexOf(b[0])).map(([word, yd]) => `${distanceText(yd)} ${word}`).join(" · ");
    list.push(tile("Distance", distanceHtml(yards), note, { size: "xl", count: [yards, distanceHtml], extra: stack(modes.map(([n, v, k]) => [n, v, k, distanceText(v)]), { key: false, h: 8 }) }));
  }
  if (list.length % 2) list[list.length - 1].style.gridColumn = "span 2";
  return list.length ? tiles(list, 2) : null;
}

function moneyText(copper) {
  const g = Math.floor(copper / 10000);
  const s = Math.floor(copper / 100) % 100;
  const p = copper % 100;
  return g ? `${num(g)}g ${s}s` : s ? `${s}s ${p}c` : `${p}c`;
}

function stripPanel(recap) {
  const mins = Math.round(recap.buckets.secs / 60);
  const top = el("div", "recap-head");
  top.append(
    heading(`Experience every ${mins === 1 ? "minute" : `${mins} minutes`}`),
    legend([["XP", "var(--s-1)"], ["In combat", "var(--deep)"], ["Level reached", "var(--fg)"], ["Feat earned", "var(--tier-3)"]]),
  );
  top.lastChild.style.marginLeft = "auto";
  return panel(top, strip(recap));
}

function featsPanel(recap) {
  const { earned, near } = recap.feats;
  if (!earned.length && !near.length) return null;
  const box = panel(heading("Feats this session"));
  for (const f of earned) {
    const cat = BY_KEY.get(f.key);
    if (!cat) continue;
    const row = el("div", "lead-feat");
    row.append(crest(cat.glyph, f.tier, { size: 104, scope: cat.crest, label: `${cat.name}, ${TIER_NAMES[f.tier]}` }));
    const text = el("div");
    const name = el("p", "sc-feat-n");
    name.append(cat.name, tierChip(f.tier, TIER_NAMES[f.tier]));
    text.append(name, elText("p", "sc-feat-d", countText(cat, f.value)));
    text.append(elText("p", "sc-feat-to", `Earned at ${clockOf(recap.start + f.at)}${f.from ? `, up from ${TIER_NAMES[f.from]}` : ""}`));
    row.append(text);
    box.append(row);
  }
  if (near.length) {
    box.append(label("Closest when the session ended"));
    const list = el("div", "near-feats");
    for (const f of near) {
      const cat = BY_KEY.get(f.key);
      if (!cat) continue;
      const row = el("div", "lead-feat");
      const from = f.tier ? cat.thresholds[f.tier - 1] : 0;
      row.append(crest(cat.glyph, f.tier, { size: 48, progress: (f.value - from) / (f.next - from), scope: cat.crest, label: cat.name }));
      const text = el("div");
      const name = el("p", "sc-feat-n");
      name.append(cat.name);
      if (f.tier) name.append(tierChip(f.tier, TIER_NAMES[f.tier]));
      text.append(name, elText("p", "sc-feat-d", `${countText(cat, f.value)} · ${amount(cat, f.left)} to ${TIER_NAMES[f.tier + 1]}`));
      row.append(text);
      list.append(row);
    }
    box.append(list);
  }
  return box;
}

const METRICS = [
  ["quests_h", "Quests an hour", (v) => v.toFixed(1)],
  ["miles_h", "Miles an hour", (v) => v.toFixed(1)],
  ["deaths", "Deaths", (v) => num(v)],
  ["played", "Played", (v) => dur(v)],
];

function comparePanel(recap) {
  if (!recap.own) return null;
  const grid = el("div", "compare-grid");
  for (const [key, name, show] of METRICS) {
    const s = recap.own.metrics[key];
    if (!s) continue;
    const round = (v) => Math.round(v * 10) / 10;
    const spread = Object.fromEntries(Object.entries(s).map(([k, v]) => [k, round(v)]));
    grid.append(elText("span", "sc-sub", name), compare(spread, { name, text: show(s.you) }), elText("b", "", show(s.you)));
  }
  const top = el("div", "recap-head");
  top.append(heading(`Against the last ${recap.own.n} sessions`), legend([["This session", "var(--s-1)"], ["Middle half", "var(--muted)"]]));
  top.lastChild.style.marginLeft = "auto";
  return panel(top, grid);
}

function timeEntries(recap) {
  const c = recap.counters;
  const moving = [
    ["Swimming", c.swimTime ?? 0, "--m-swim"],
    ["On foot", (c.footTime ?? 0) + (c.walkTime ?? 0), "--m-foot"],
    ["Mounted", c.mountTime ?? 0, "--m-mount"],
    ["Flight paths", (c.taxiTime ?? 0) + (c.flyTime ?? 0), "--m-flight"],
  ];
  const used = moving.reduce((a, m) => a + m[1], 0);
  moving.push(["Standing still", Math.max(0, recap.played - used), "--m-still"]);
  return moving.filter((m) => m[1] > 0).map(([n, v, k]) => [n, v, k, dur(v)]);
}

function timePanel(recap) {
  const c = recap.counters;
  const box = panel(heading("Where the time went"));
  const moving = timeEntries(recap);
  if (moving.length) box.append(label("Moving and standing"), stack(moving));
  const two = (name, value, other) => {
    const rest = Math.max(0, recap.played - value);
    return [[name, value, "--s-1", dur(value)], [other, rest, "--m-still", dur(rest)]].filter((r) => r[1] > 0);
  };
  if (c.combat) box.append(label("In combat"), stack(two("In combat", c.combat, "Out of combat")));
  if (c.restedTime) box.append(label("Rested"), stack(two("Rested", c.restedTime, "Not rested")));
  if (c.groupTime) box.append(label("In a group"), stack(two("In a group", c.groupTime, "Solo")));
  return moving.length ? box : null;
}

function levelRows(recap) {
  if (!recap.levels.length) return [];
  const rows = [];
  let level = recap.level[0];
  let from = 0;
  for (const l of recap.levels) {
    rows.push([`Level ${level}`, l.at - from, dur(l.at - from)]);
    level = l.level;
    from = l.at;
  }
  const left = Math.max(0, recap.end - recap.start - from);
  rows.push([`Level ${level}`, left, `${dur(left)} so far`]);
  return rows;
}

function experiencePanel(recap) {
  const x = recap.xp;
  const rows = [["Kills", x.kills, "--s-kill"], ["Exploring", x.explore, "--s-explore"], ["Quests", x.quests, "--s-quest"], ["Dungeons", x.dungeons, "--s-dungeon"], ["Other", x.other, "--s-other"]].filter((r) => r[1] > 0);
  if (!rows.length) return null;
  const box = panel(heading("Experience by source"), stack(rows));
  const levels = levelRows(recap);
  if (levels.length) box.append(label("Time on each level"), bars(levels));
  return box;
}

function killedPanel(recap) {
  const k = recap.top?.killed ?? [];
  return k.length ? panel(heading("Killed most"), bars(k.map((r) => [r.name, r.n]))) : null;
}

function counts(recap) {
  const c = recap.counters;
  const combatMin = (c.combat ?? 0) / 60;
  const walked = (c.foot ?? 0) + (c.walk ?? 0) + (c.mount ?? 0) + (c.swim ?? 0);
  const steps = Math.round((c.foot ?? 0) / 2.5 + (c.walk ?? 0) / 1.4);
  const list = [
    ["Jumps", c.jumps, walked >= YARDS_PER_MILE / 10 ? `${num(c.jumps / (walked / YARDS_PER_MILE))} a mile` : ""],
    ["Casts", c.casts, combatMin >= 1 ? `${num(c.casts / combatMin)} a minute in combat` : ""],
    ["Damage done", c.damage, combatMin >= 1 ? `${num(c.damage / combatMin)} a minute in combat` : ""],
    ["Damage taken", c.damageTaken, c.damage ? `${Math.round((c.damageTaken / c.damage) * 100)}% of the damage done` : ""],
    ["Healing done", c.healing, ""],
    ["Steps", steps, "running and walking"],
    ["Quests turned in", c.quests, c.abandoned ? `${num(c.abandoned)} abandoned` : ""],
    ["Items looted", c.loot, ""],
    ["Places discovered", c.discoveries, ""],
    ["NPCs talked to", c.npcTalks, ""],
    ["Time AFK", c.afkTime, ""],
  ].filter((r) => r[1]);
  if (!list.length) return null;
  const grid = tiles(
    list.map(([name, v, note]) => tile(name, name === "Time AFK" ? dur(v) : num(v), note, { size: "md", count: [v, name === "Time AFK" ? dur : num] })),
    list.length <= 6 ? list.length : Math.min(6, Math.ceil(list.length / 2)),
  );
  return grid;
}

function momentIcon(m) {
  const host = el("span");
  if (m.kind === "feat") {
    const cat = BY_KEY.get(m.feat);
    host.append(crest(cat?.glyph ?? "kills", m.tier ?? 1, { size: 18, scope: cat?.crest }));
    return host;
  }
  const root = svg("svg", { viewBox: "0 0 24 24", width: 16, height: 16, fill: "none", stroke: "#f0f1f3", "stroke-width": 1.9, "stroke-linecap": "round", "stroke-linejoin": "round" });
  root.innerHTML = GLYPHS[{ death: "deaths", level: "level", boss: "dungeons", discovery: "zones" }[m.kind]] ?? "";
  host.append(root);
  return host;
}

function momentRows(recap) {
  const rows = recap.moments.map((m) => ({ at: m.at, kind: m.kind, text: m.text, feat: m.feat, tier: m.tier }));
  let level = recap.level[0];
  let from = null;
  for (const l of recap.levels) {
    const after = from === null ? "" : ` after ${dur(l.at - from)} on level ${level}`;
    rows.push({ at: l.at, kind: "level", text: `Level ${l.level}${after}` });
    level = l.level;
    from = l.at;
  }
  return rows.sort((a, b) => a.at - b.at);
}

function momentsPanel(recap) {
  const rows = momentRows(recap);
  if (!rows.length) return null;
  const box = panel(heading("Levels, deaths and feats"));
  for (const m of rows) {
    const row = el("div", "moment");
    row.append(elText("span", "sc-mono", clockOf(recap.start + m.at)), momentIcon(m), elText("span", "", m.text));
    box.append(row);
  }
  return box;
}

function lootPanel(recap) {
  const top = recap.top;
  if (!top) return null;
  const box = panel(heading("Quests and loot"));
  if (top.quests_by_zone.length) box.append(label("Quests turned in, by zone"), bars(top.quests_by_zone.map((z) => [z.zone, z.n])));
  if (top.looted.length) box.append(label("Looted most"), bars(top.looted.map((i) => [i.name, i.n])));
  return top.quests_by_zone.length || top.looted.length ? box : null;
}

function row(kind, ...panels) {
  const kept = panels.filter(Boolean);
  if (!kept.length) return null;
  const grid = el("div", `recap-grid ${kind}`);
  grid.style.gridTemplateColumns = `repeat(${kept.length}, minmax(0, 1fr))`;
  grid.append(...kept);
  return grid;
}

async function onTheWebsite(recap) {
  if ((await webState()).state !== "linked") return false;
  return invoke("session_on_web", { character: recap.character, start: recap.start, end: recap.end }).catch(() => false);
}

function draw(into, recap, paces, run, nav, onAll, onWeb) {
  into.replaceChildren(head(recap, paces, run, nav, onAll, onWeb));
  const c = recap.counters;
  const totals = {};
  if (c.earned || c.spent) totals.gold_h = `${money((c.earned ?? 0) - (c.spent ?? 0))} in ${dur(recap.played)} · earned ${moneyText(c.earned ?? 0)} · spent ${moneyText(c.spent ?? 0)}`;
  const selectRecordCharacter = () => {
    if (state.ch) return;
    const select = document.querySelector("#char");
    if (select) select.value = recap.character;
    state.ch = recap.character;
  };
  const rates = paceTiles(paces, recap.played, { totals, onOpen: selectRecordCharacter });
  if (rates) {
    rates.style.marginTop = "18px";
    into.append(rates);
  }
  const lead = el("div", "recap-lead");
  const grid = tileGrid(recap, paces);
  lead.append(routePanel(recap));
  if (grid) lead.append(grid);
  else lead.classList.add("solo");
  into.append(lead);
  if (totalXp(recap) || recap.levels.length) {
    const bar = stripPanel(recap);
    bar.style.marginTop = "16px";
    into.append(bar);
  }
  const rows = [
    row("two", featsPanel(recap), comparePanel(recap)),
    row("three", timePanel(recap), experiencePanel(recap), killedPanel(recap)),
    row("one", counts(recap)),
    row("two", momentsPanel(recap), lootPanel(recap)),
  ];
  for (const r of rows) if (r) into.append(r);
}

export async function renderRecaps(body, run, nav) {
  wireTips();
  body.innerHTML = "";
  const page = el("div", "recap");
  const content = el("div");
  const onAll = () => window.__nav("sessions");
  let latest = 0;
  const show = async (i) => {
    const recap = run.recaps[i];
    const mine = ++latest;
    const onWeb = await onTheWebsite(recap);
    if (mine !== latest) return;
    draw(content, recap, run.paces[i] ?? [], run, nav, onAll, onWeb);
    play(page, `${location.hash}:${recap.character}`);
  };
  if (run.recaps.length > 1) {
    const seg = el("div", "sc-seg");
    run.recaps.forEach((recap, i) => {
      const b = elText("button", i === 0 ? "on" : "", displayName(recap.character));
      b.type = "button";
      b.onclick = () => {
        for (const o of seg.children) o.classList.toggle("on", o === b);
        show(i);
      };
      seg.append(b);
    });
    page.append(seg);
  }
  page.append(content);
  body.append(page);
  await show(0);
}
