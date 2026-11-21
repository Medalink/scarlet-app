// Overview: one character at a glance (docs/OVERVIEW.md, "The page").
// The model comes from the `overview` command, the same rows the export
// writes to Digest.lua for the game.

import { invoke, el, esc, num, dur, money, dateStr, state } from "../lib.js";

const CLASS_COLOR = {
  DEATHKNIGHT: "#C41E3A",
  DEMONHUNTER: "#A330C9",
  DRUID: "#FF7C0A",
  EVOKER: "#33937F",
  HUNTER: "#AAD372",
  MAGE: "#3FC7EB",
  MONK: "#00FF98",
  PALADIN: "#F48CBA",
  PRIEST: "#FFFFFF",
  ROGUE: "#FFF468",
  SHAMAN: "#0070DD",
  WARLOCK: "#8788EE",
  WARRIOR: "#C69B6D",
};
const CLASS_NAME = { DEATHKNIGHT: "Death Knight", DEMONHUNTER: "Demon Hunter" };
const RACE_NAME = { Scourge: "Undead" };

// XP sources in stacking order, bottom first.
const XP_PARTS = [
  ["xk", "Kills", "#ff2d55"],
  ["xd", "Dungeon kills", "#8a2540"],
  ["xq", "Quests", "#e8c27a"],
  ["xe", "Exploring", "#5fb3a1"],
  ["xo", "Other", "#8a8a93"],
];

const WEEK_ZERO = { p: 0, r: 0, q: 0, gold: 0 };

let xpMode = "xp";
let histMode = "sessions";
let current = null;

const className = (c) => CLASS_NAME[c] || (c ? c[0] + c.slice(1).toLowerCase() : "");
const raceName = (r) => RACE_NAME[r] || String(r || "").replace(/([a-z])([A-Z])/g, "$1 $2");
const plural = (n, one, many) => `${num(n)} ${n === 1 ? one : many}`;

function longDur(secs) {
  secs = Math.max(0, Math.floor(secs));
  const d = Math.floor(secs / 86400);
  if (d > 0) return `${d}d ${Math.floor(secs / 3600) % 24}h`;
  return dur(secs);
}

// A copper change with its sign: "+6g 40s", "-2g 80s".
function signed(c) {
  if (!c) return `<span>0c</span>`;
  return c > 0 ? `<span class="pos">+${money(c)}</span>` : `<span class="neg">${money(c)}</span>`;
}

const net = (s) => (s.g || 0) - (s.s || 0);

function empty(text) {
  return `<div class="empty">${esc(text)}</div>`;
}

function panel(title, html) {
  const p = el("div", "panel", `<h3>${esc(title)}</h3>${html}`);
  return p;
}

function seg(options, active, key) {
  return `<div class="ov-seg">${options
    .map(([k, label]) => `<span class="chip${k === active ? " active" : ""}" data-${key}="${k}">${esc(label)}</span>`)
    .join("")}</div>`;
}

function hero(ov, name) {
  const color = CLASS_COLOR[ov.class] || "#ff2d55";
  const bits = [];
  const identity = [ov.level ? `Level ${ov.level}` : "", raceName(ov.race), className(ov.class)].filter(Boolean).join(" ");
  if (identity) bits.push(identity);
  if (ov.faction) bits.push(ov.faction);
  if (ov.zone) bits.push(`in ${ov.zone}`);
  if (ov.played) bits.push(`${longDur(ov.played)} played`);
  let bar = `<div class="empty">No experience bar recorded</div>`;
  if (ov.xpMax > 0) {
    const pct = Math.min(100, Math.floor((ov.xp / ov.xpMax) * 100));
    bar = `<div class="ov-bar"><div style="width:${pct}%"></div></div>
      <div class="ov-note">${pct}% to level ${ov.level + 1} · ${num(Math.max(0, ov.xpMax - ov.xp))} XP to go</div>`;
  }
  return `<div class="panel ov-hero" style="--cls:${color}">
    <div class="ov-ring">${ov.level || "?"}</div>
    <div class="ov-hero-main">
      <div class="ov-name">${esc(name)}</div>
      <div class="ov-sub">${esc(bits.join(" · "))}</div>
      ${bar}
    </div>
  </div>`;
}

function sparkline(points) {
  if (points.length < 2) return "";
  const lo = Math.min(...points);
  const hi = Math.max(...points);
  const pts = points
    .map((v, i) => {
      const x = (i / (points.length - 1)) * 100;
      const y = hi === lo ? 18 : 33 - ((v - lo) / (hi - lo)) * 30;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return `<svg class="ov-spark" viewBox="0 0 100 36" preserveAspectRatio="none">
    <polyline fill="none" stroke="#ff6b8e" stroke-width="2" vector-effect="non-scaling-stroke" points="${pts}"/></svg>`;
}

function goldCard(ov) {
  if (ov.money === undefined) return panel("Gold", empty("No balance recorded"));
  const recent = ov.sessions.slice(0, 7);
  const change = recent.reduce((a, s) => a + net(s), 0);
  const change7 = recent.length
    ? `<div class="ov-note">${signed(change)} over the last ${plural(recent.length, "session", "sessions")}</div>`
    : "";
  return panel(
    "Gold",
    `<div class="ov-big">${esc(money(ov.money))}</div>${change7}${sparkline(ov.gold.map((g) => g.m))}`
  );
}

function runsCard(ov) {
  if (!ov.runs.length) return panel("Dungeons and raids", empty("No dungeon or raid runs recorded"));
  const rows = ov.runs
    .slice(0, 5)
    .map((r) => `<div class="ov-row"><span>${esc(r.name)}</span><span>${plural(r.n, "run", "runs")}</span></div>`)
    .join("");
  return panel("Dungeons and raids", `${rows}<div class="ov-note">${num(ov.runsTotal)} in total</div>`);
}

function matesCard(ov) {
  if (!ov.mates.length) return panel("Played with", empty("No group members recorded"));
  const top = ov.mates.slice(0, 3);
  const max = Math.max(...top.map((m) => m.p), 1);
  const rows = top
    .map((m) => {
      const color = CLASS_COLOR[m.class] || "";
      return `<div class="ov-row"><span${color ? ` style="color:${color}"` : ""}>${esc(m.name)}</span>
        <span>${dur(m.p)} · ${plural(m.n, "session", "sessions")}</span></div>
        <div class="ov-bar slim"><div style="width:${Math.max(2, (m.p / max) * 100)}%"></div></div>`;
    })
    .join("");
  return panel("Played with", rows);
}

function deathsCard(ov) {
  if (!ov.deathZones.length) return panel("Deaths", empty("No deaths recorded"));
  const rows = ov.deathZones
    .slice(0, 3)
    .map((z) => `<div class="ov-row"><span>${esc(z.name)}</span><span>${num(z.n)}</span></div>`)
    .join("");
  const k = ov.killers[0];
  const killer = k
    ? `<div class="ov-row"><span>Killed most by</span><span>${esc(k.name)} (${num(k.n)})</span></div>`
    : "";
  return panel("Deaths", rows + killer);
}

// Monday 00:00 local time to now.
function weekCard(ov) {
  const monday = new Date();
  monday.setHours(0, 0, 0, 0);
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7));
  const sunday = new Date(monday);
  sunday.setDate(monday.getDate() + 6);
  const fmt = (d) => d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" });
  const title = `This week · ${fmt(monday)} – ${fmt(sunday)}`;
  const week = ov.sessions.filter((s) => s.t * 1000 >= monday.getTime());
  if (!week.length) return panel(title, empty("No sessions this week"));
  const sum = week.reduce(
    (a, s) => ({ p: a.p + (s.p || 0), r: a.r + (s.r || 0), q: a.q + (s.q || 0), gold: a.gold + net(s) }),
    WEEK_ZERO
  );
  const zones = new Set(week.flatMap((s) => Object.keys(s.zn || {})));
  const tiles = [
    [dur(sum.p), "Played"],
    [num(sum.r), "Dungeons"],
    [num(sum.q), "Quests"],
    [signed(sum.gold), "Gold"],
    [num(zones.size), "Zones"],
  ];
  return panel(
    title,
    `<div class="ov-tiles">${tiles.map(([v, l]) => `<div class="ov-tile"><b>${v}</b><small>${l}</small></div>`).join("")}</div>`
  );
}

function levelXp(l) {
  return XP_PARTS.reduce((a, [k]) => a + (l[k] || 0), 0);
}

function levelsChart(ov) {
  const levels = Object.entries(ov.levels)
    .map(([lv, l]) => [Number(lv), l])
    .sort((a, b) => a[0] - b[0])
    .filter(([, l]) => (xpMode === "xp" ? levelXp(l) : l.p) > 0);
  const head = seg([["xp", "XP sources"], ["time", "Time per level"]], xpMode, "xp");
  if (!levels.length) return panel("Experience by level", head + empty("No experience recorded"));
  const total = (l) => (xpMode === "xp" ? levelXp(l) : l.p);
  const max = Math.max(...levels.map(([, l]) => total(l)), 1);
  const cols = levels
    .map(([lv, l]) => {
      const tip = xpMode === "xp"
        ? `Level ${lv}: ${XP_PARTS.filter(([k]) => l[k]).map(([k, name]) => `${name} ${num(l[k])}`).join(", ")}`
        : `Level ${lv}: ${dur(l.p)}`;
      const segs = xpMode === "xp"
        ? XP_PARTS.filter(([k]) => l[k])
            .map(([k, , c]) => `<i style="height:${(l[k] / max) * 100}%;background:${c}"></i>`)
            .join("")
        : `<i style="height:${(l.p / max) * 100}%;background:#ff2d55"></i>`;
      return `<div class="ov-col" title="${esc(tip)}">${segs}</div>`;
    })
    .join("");
  const range = levels.length > 1 ? `Levels ${levels[0][0]} to ${levels[levels.length - 1][0]}` : `Level ${levels[0][0]}`;
  const legend = xpMode === "xp"
    ? XP_PARTS.map(([, name, c]) => `<span><i style="background:${c}"></i>${name}</span>`).join("")
    : "";
  return panel(
    "Experience by level",
    `${head}<div class="ov-chart">${cols}</div><div class="legend"><span>${range}</span>${legend}</div>`
  );
}

const HISTORY_ROWS = 60;

function th(label, cls = "") {
  return `<th${cls ? ` class="${cls}"` : ""}>${label}</th>`;
}

function levelSpan(lo, hi) {
  if (!lo && !hi) return "";
  return lo === hi || !lo ? String(hi) : `${lo} → ${hi}`;
}

function historyTable(ov) {
  if (histMode === "levels") {
    const rows = Object.entries(ov.levels)
      .map(([lv, l]) => [Number(lv), l])
      .sort((a, b) => b[0] - a[0]);
    if (!rows.length) return empty("No levels recorded");
    const body = rows
      .map(
        ([lv, l]) => `<tr><td>${lv}</td><td>${l.at ? dateStr(l.at * 1000) : "Before recording"}</td>
          <td class="num">${dur(l.p || 0)}</td><td class="num">${num(levelXp(l))}</td><td class="num">${num(l.k || 0)}</td>
          <td class="num">${num(l.q || 0)}</td><td class="num">${num(l.dd || 0)}</td><td>${esc(l.zone || "")}</td></tr>`
      )
      .join("");
    return `<table><tr>${th("Level")}${th("Reached")}${th("Played", "num")}${th("XP", "num")}${th("Kills", "num")}${th("Quests", "num")}${th("Deaths", "num")}${th("Main zone")}</tr>${body}</table>`;
  }

  const head = `<tr>${th("When")}${th("Played", "num")}${th("Levels")}${th("Quests", "num")}${th("Gold", "num")}${th("Zones", "num")}</tr>`;
  if (!ov.sessions.length) return empty("No sessions recorded");
  let rows;
  if (histMode === "days") {
    const days = new Map();
    for (const s of ov.sessions) {
      const key = dateStr(s.t * 1000);
      const d = days.get(key) || { p: 0, lo: 0, hi: 0, q: 0, gold: 0, zones: new Set() };
      d.p += s.p || 0;
      d.lo = d.lo ? Math.min(d.lo, s.l0 || d.lo) : s.l0 || 0;
      d.hi = Math.max(d.hi, s.l1 || 0);
      d.q += s.q || 0;
      d.gold += net(s);
      Object.keys(s.zn || {}).forEach((z) => d.zones.add(z));
      days.set(key, d);
    }
    rows = [...days.entries()].map(([key, d]) => [key, d.p, levelSpan(d.lo, d.hi), d.q, d.gold, d.zones.size]);
  } else {
    rows = ov.sessions.map((s) => [
      `${dateStr(s.t * 1000)} ${new Date(s.t * 1000).toTimeString().slice(0, 5)}`,
      s.p || 0,
      levelSpan(s.l0 || 0, s.l1 || 0),
      s.q || 0,
      net(s),
      s.z || 0,
    ]);
  }
  const shown = rows.slice(0, HISTORY_ROWS);
  const body = shown
    .map(
      ([when, p, lv, q, gold, z]) =>
        `<tr><td>${esc(when)}</td><td class="num">${dur(p)}</td><td>${esc(lv)}</td><td class="num">${num(q)}</td>
        <td class="num">${signed(gold)}</td><td class="num">${num(z)}</td></tr>`
    )
    .join("");
  const more = rows.length > shown.length ? `<div class="ov-note">Newest ${shown.length} of ${num(rows.length)}</div>` : "";
  return `<table>${head}${body}</table>${more}`;
}

function historyCard(ov) {
  const head = seg([["sessions", "Sessions"], ["days", "Days"], ["levels", "Levels"]], histMode, "hist");
  return panel("Play history", head + historyTable(ov));
}

function draw(body) {
  body.innerHTML = "";
  if (!current) {
    body.appendChild(panel("Overview", empty("No characters recorded yet")));
    return;
  }
  const { name, ov } = current;
  body.insertAdjacentHTML("beforeend", hero(ov, name));
  const cols = el("div", "ov-cols");
  const left = el("div", "ov-stack");
  const right = el("div", "ov-stack");
  for (const card of [goldCard(ov), runsCard(ov), matesCard(ov), deathsCard(ov)]) left.appendChild(card);
  for (const card of [weekCard(ov), levelsChart(ov), historyCard(ov)]) right.appendChild(card);
  cols.append(left, right);
  body.appendChild(cols);

  body.querySelectorAll("[data-xp]").forEach((n) => {
    n.onclick = () => {
      xpMode = n.dataset.xp;
      draw(body);
    };
  });
  body.querySelectorAll("[data-hist]").forEach((n) => {
    n.onclick = () => {
      histMode = n.dataset.hist;
      draw(body);
    };
  });
}

export async function render(body) {
  current = await invoke("overview", { ch: state.ch });
  draw(body);
}
