// The session recap of recap-app.html, drawn from `run_summary(...).recaps`: head, route and tiles, strip, feats and
// comparison, time and experience, counts, moments, quests and loot.

import { invoke, el, elText, num, dur, moneyHtml, displayName, distanceHtml, tile, tiles, stack, bars, compare, strip, routeCard, wireTips } from "../lib.js";
import { BY_KEY } from "../feats-catalog.js";
import { GLYPHS, TIER_NAMES, svg } from "../glyphs.js";
import { seal, tierChip, countText, amount } from "../featcard.js";

const YARDS_PER_MILE = 1760;
const clockOf = (sec) => {
  const d = new Date(sec * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};
const dayOf = (sec) => new Date(sec * 1000).toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" });
const shortDay = (ms) => new Date(ms).toLocaleDateString([], { day: "numeric", month: "short" });
const miles = (yd) => (yd / YARDS_PER_MILE).toFixed(1);
const rate = (n, secs) => (secs > 0 ? n / (secs / 3600) : 0);

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

function neighbors(recap, rows) {
  const from = recap.start * 1000;
  const to = recap.end * 1000;
  const mine = rows.filter((r) => r.char_name === recap.character);
  return {
    earlier: [...mine].reverse().find((r) => r.end <= from) ?? null,
    later: mine.find((r) => r.start >= to) ?? null,
  };
}

function head(recap, near, onAll) {
  const box = el("div", "recap-head");
  const text = el("div");
  text.append(elText("h1", "sc-title", recap.title.text));
  const line = el("p", "recap-sub");
  line.append(elText("b", "", displayName(recap.character)), ` · ${dayOf(recap.start)} · ${clockOf(recap.start)} to ${clockOf(recap.end)} · ${dur(recap.played)} played`);
  text.append(line);
  const actions = el("div", "recap-actions");
  const go = (row, text) => {
    const b = elText("button", "sc-btn quiet small", text);
    b.type = "button";
    b.onclick = () => window.__nav(`sessions/${row.start}-${row.end}`);
    return b;
  };
  if (near.earlier) actions.append(go(near.earlier, `← ${shortDay(near.earlier.start)}`));
  if (near.later) actions.append(go(near.later, `${shortDay(near.later.start)} →`));
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
  box.append(routeCard(recap.route, { levels }));
  const flown = recap.route.sheet.air?.includes(true);
  box.append(legend([["Walked, newest brightest", "var(--s-1)"], ...(flown ? [["Teleported", "var(--m-flight)"]] : [])]));
  return box;
}

function tileGrid(recap) {
  const c = recap.counters;
  const xp = Object.values(recap.xp).reduce((a, v) => a + v, 0);
  const best = (key) => (recap.bests.includes(key) ? "Best of the last 30 sessions" : "");
  const list = [];
  if (xp) list.push(tile("Experience", num(xp), `${num(rate(xp, recap.played))} an hour`, { size: "xl", best: best("xp") }));
  if (c.kills) list.push(tile("Kills", num(c.kills), `${num(rate(c.kills, recap.played))} an hour`, { size: "xl", best: best("kills") }));
  const net = (c.earned ?? 0) - (c.spent ?? 0);
  if (c.earned || c.spent) {
    list.push(tile("Gold", moneyHtml(net, true), `Earned ${moneyText(c.earned ?? 0)} · spent ${moneyText(c.spent ?? 0)}`, { size: "xl", best: best("earned") }));
  }
  const yards = (c.foot ?? 0) + (c.walk ?? 0) + (c.mount ?? 0) + (c.swim ?? 0) + (c.fly ?? 0);
  if (yards) {
    const modes = [["Swimming", c.swim ?? 0, "--m-swim"], ["On foot", (c.foot ?? 0) + (c.walk ?? 0), "--m-foot"], ["Mounted", c.mount ?? 0, "--m-mount"], ["Flown", c.fly ?? 0, "--m-flight"]].filter((m) => m[1] > 0);
    const note = modes.map(([name, yd]) => `${miles(yd)} ${name === "On foot" ? "on foot" : name === "Swimming" ? "swum" : name === "Mounted" ? "mounted" : "flown"}`).join(" · ");
    list.push(tile("Distance", distanceHtml(yards), note, { size: "xl", best: best("distance"), extra: stack(modes.map(([n, v, k]) => [n, v, k, `${miles(v)} mi`]), { key: false, h: 8 }) }));
  }
  return tiles(list, 2);
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
    legend([["XP", "var(--s-1)"], ["In combat", "var(--deep)"], ["Level reached", "var(--fg)"], ["Feat earned", "var(--t-gold)"]]),
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
    row.append(seal(cat.glyph, f.tier, { size: 104, progress: 0, family: cat.family, label: `${cat.name}, ${TIER_NAMES[f.tier]}` }));
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
      row.append(seal(cat.glyph, f.tier, { size: 48, progress: f.value / f.next, family: cat.family, label: cat.name }));
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
  ["xp_h", "XP an hour", (v) => num(v)],
  ["kills_h", "Kills an hour", (v) => num(v)],
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
  if (c.groupTime) box.append(label("In a group"), stack(two("In a group", c.groupTime, "Alone")));
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
  const rows = [["Kills", x.kills, "--s-kill"], ["Dungeons", x.dungeons, "--s-dungeon"], ["Quests", x.quests, "--s-quest"], ["Exploring", x.explore, "--s-explore"], ["Other", x.other, "--s-other"]].filter((r) => r[1] > 0);
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
    list.map(([name, v, note]) => tile(name, name === "Time AFK" ? dur(v) : num(v), note, { size: "md" })),
    list.length <= 6 ? list.length : Math.min(6, Math.ceil(list.length / 2)),
  );
  return grid;
}

function momentIcon(m) {
  const host = el("span");
  if (m.kind === "feat") {
    const cat = BY_KEY.get(m.feat);
    host.append(seal(cat?.glyph ?? "kills", m.tier ?? 1, { size: 18, family: cat?.family }));
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

function draw(into, recap, near, onAll) {
  into.replaceChildren(head(recap, near, onAll));
  const lead = el("div", "recap-lead");
  lead.append(routePanel(recap), tileGrid(recap));
  into.append(lead);
  const bar = stripPanel(recap);
  bar.style.marginTop = "16px";
  into.append(bar);
  const rows = [
    row("two", featsPanel(recap), comparePanel(recap)),
    row("three", timePanel(recap), experiencePanel(recap), killedPanel(recap)),
    row("one", counts(recap)),
    row("two", momentsPanel(recap), lootPanel(recap)),
  ];
  for (const r of rows) if (r) into.append(r);
}

export async function renderRecaps(body, run) {
  wireTips();
  body.innerHTML = "";
  const rows = await invoke("sessions", { ch: null });
  const page = el("div", "recap");
  const content = el("div");
  const onAll = () => window.__nav("sessions");
  const show = (recap) => draw(content, recap, neighbors(recap, rows), onAll);
  if (run.recaps.length > 1) {
    const seg = el("div", "sc-seg");
    run.recaps.forEach((recap, i) => {
      const b = elText("button", i === 0 ? "on" : "", displayName(recap.character));
      b.type = "button";
      b.onclick = () => {
        for (const o of seg.children) o.classList.toggle("on", o === b);
        show(recap);
      };
      seg.append(b);
    });
    page.append(seg);
  }
  page.append(content);
  body.append(page);
  show(run.recaps[0]);
}
