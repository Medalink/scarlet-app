// Overview: one character at a glance (docs/OVERVIEW.md, "The page").
// The model comes from the `overview` command, the same rows the export
// writes to ScarletData\Digest.lua for the game. Section order: header, this week,
// experience by level and gold, dungeons, played with and deaths, play time
// by day and by zone, play history.

import { invoke, esc, num, dur, moneyHtml, dateStr, SERIES, state } from "../lib.js";

export const CLASS_COLOR = {
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
  ["xk", "Kills", SERIES[0]],
  ["xd", "Dungeon kills", SERIES[1]],
  ["xq", "Quests", SERIES[2]],
  ["xe", "Exploring", SERIES[3]],
  ["xo", "Other", SERIES[4]],
];

const WD = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MO = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const HEAT_WEEKS = 20;
const GOLD_POINTS = 30;
const HISTORY_ROWS = 10;
const HISTORY_MAX = 200;

let xpMode = "xp";
let histMode = "sessions";
let histAll = false;
let xpPinned = null;
let current = null;

export const className = (c) => CLASS_NAME[c] || (c ? c[0] + c.slice(1).toLowerCase() : "");
export const raceName = (r) => RACE_NAME[r] || String(r || "").replace(/([a-z])([A-Z])/g, "$1 $2");
export const plural = (n, one, many) => `${num(n)} ${n === 1 ? one : many}`;
export const day = (ms) => {
  const d = new Date(ms);
  return `${WD[d.getDay()]} ${d.getDate()} ${MO[d.getMonth()]}`;
};
export const hm = (ms) => new Date(ms).toTimeString().slice(0, 5);
const net = (s) => (s.g || 0) - (s.s || 0);

export function longDur(secs) {
  secs = Math.max(0, Math.floor(secs));
  const d = Math.floor(secs / 86400);
  if (d > 0) return `${d}d ${Math.floor(secs / 3600) % 24}h`;
  return dur(secs);
}

// A play time as "2h 05m" or "33m", like the site.
export function playTime(secs) {
  const m = Math.round(secs / 60);
  if (m < 60) return `${m}m`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
}

export const signed = (c) => (c ? `<span class="${c > 0 ? "pos" : "neg"}">${moneyHtml(c, true)}</span>` : `<span class="neg">${moneyHtml(0)}</span>`);

export const empty = (text) => `<div class="empty">${esc(text)}</div>`;

export function mod(cls, title, inner, head = "") {
  const h = head ? `<div class="mod-head"><h2>${esc(title)}</h2>${head}</div>` : `<h2>${esc(title)}</h2>`;
  return `<section class="mod ${cls}">${h}${inner}</section>`;
}

function segButtons(options, active, key) {
  return `<div class="seg" role="group">${options
    .map(([k, label]) => `<button type="button" data-${key}="${k}" aria-pressed="${k === active}">${esc(label)}</button>`)
    .join("")}</div>`;
}

function rank(rows) {
  const top = Math.max(...rows.map((r) => r.v), 1);
  return `<ol class="rank">${rows
    .map(
      (r) => `<li><span class="who"${r.color ? ` style="color:${r.color}"` : ""} title="${esc(r.who)}">${esc(r.who)}</span>
        <span class="val">${r.val}</span><div class="track"><i style="width:${Math.max(1, (r.v / top) * 100)}%"></i></div></li>`
    )
    .join("")}</ol>`;
}

// Header ------------------------------------------------------------------

// Time to the next level, the next tens level and level 60, as the companion
// worked them out (docs/OVERVIEW.md, "Level estimates").
function etaRows(eta) {
  if (!eta || (!eta.targets.length && !eta.basis)) return "";
  const rows = eta.targets
    .map((t) => `<li><span class="eta-lv">Level ${t.level}</span><b>${esc(t.time)}</b><span>${esc(t.date)}</span></li>`)
    .join("");
  return `<div class="eta">${rows ? `<ul>${rows}</ul>` : ""}${eta.basis ? `<p class="eta-basis">${esc(eta.basis)}</p>` : ""}</div>`;
}

function header(ov, name, eta) {
  const color = CLASS_COLOR[ov.class] || "#ff2d55";
  const identity = [ov.level ? `Level ${ov.level}` : "", raceName(ov.race), className(ov.class)].filter(Boolean).join(" ");
  const bits = [identity, ov.faction, ov.zone ? `in ${ov.zone}` : "", ov.played ? `${longDur(ov.played)} played` : ""].filter(Boolean);
  const atCap = ov.cap && ov.cap.level > 0 && ov.level >= ov.cap.level;
  let bar = atCap ? "" : `<p class="ch-note" style="text-align:left;margin-top:14px">No experience bar recorded</p>`;
  if (ov.xpMax > 0 && !atCap) {
    const pct = Math.min(100, Math.floor((ov.xp / ov.xpMax) * 100));
    bar = `<div class="lvl"><div class="lvl-bar"><i style="width:${pct}%"></i></div>
      <p>${pct}% to level ${ov.level + 1} · ${num(Math.max(0, ov.xpMax - ov.xp))} XP to go</p>${etaRows(eta)}</div>`;
  }
  const first = ov.sessions.length ? ov.sessions[ov.sessions.length - 1].t : 0;
  const since = first
    ? `Recorded since ${day(first * 1000)}${ov.firstLevel > 1 ? `, from level ${ov.firstLevel}` : ""}`
    : "";
  const holes = ov.gaps
    ? `${plural(ov.gaps, "stretch", "stretches")} of play not recorded${ov.gapSecs ? `, about ${longDur(ov.gapSecs)}` : ""}`
    : "";
  const notes = [since, holes].filter(Boolean).map((t) => `<p class="ch-note">${esc(t)}</p>`);
  const last = ov.through ? `<p class="ch-note">Last session ${day(ov.through * 1000)} ${hm(ov.through * 1000)}</p>` : "";
  return `<section class="ch-head" style="--cls:${color}">
    <div>
      <h1 class="ch-name">${esc(name)}</h1>
      <p class="ch-sub"><i class="dot"></i>${bits.map((b) => `<span>${esc(b)}</span>`).join("")}</p>
      ${bar}
    </div>
    <div>${notes.join("")}${last}</div>
  </section>`;
}

// This week -----------------------------------------------------------------

export function mondayOf(now) {
  const m = new Date(now);
  m.setHours(0, 0, 0, 0);
  m.setDate(m.getDate() - ((m.getDay() + 6) % 7));
  return m;
}

function week(ov) {
  const monday = mondayOf(new Date());
  const sunday = new Date(monday);
  sunday.setDate(monday.getDate() + 6);
  const from = monday.getMonth() === sunday.getMonth() ? "" : ` ${MO[monday.getMonth()]}`;
  const range = `Mon ${monday.getDate()}${from} to Sun ${sunday.getDate()} ${MO[sunday.getMonth()]}`;
  const rows = ov.sessions.filter((s) => s.t * 1000 >= monday.getTime());
  const sum = rows.reduce(
    (a, s) => ({ p: a.p + (s.p || 0), r: a.r + (s.r || 0), q: a.q + (s.q || 0), gold: a.gold + net(s) }),
    { p: 0, r: 0, q: 0, gold: 0 }
  );
  const zones = new Set(rows.flatMap((s) => Object.keys(s.zn || {})));
  const cells = [
    ["Played", playTime(sum.p)],
    ["Dungeons", num(sum.r)],
    ["Quests", num(sum.q)],
    ["Gold", signed(sum.gold)],
    ["Zones", num(zones.size)],
  ];
  return `<section class="week"><div><h2>This week</h2><p>${range}</p></div>${cells
    .map(([l, v]) => `<div><dt>${l}</dt><dd>${v}</dd></div>`)
    .join("")}</section>`;
}

// Experience by level --------------------------------------------------------

function levelXp(l) {
  return XP_PARTS.reduce((a, [k]) => a + (l[k] || 0), 0);
}

function levelRows(ov) {
  return Object.entries(ov.levels)
    .map(([lv, l]) => [Number(lv), l])
    .sort((a, b) => a[0] - b[0])
    .filter(([, l]) => (xpMode === "xp" ? levelXp(l) : l.complete ? l.p : 0) > 0);
}

function axisXp(v) {
  return v >= 2000 ? `${Math.round(v / 1000)}k XP` : `${num(v)} XP`;
}

export function axisTime(secs) {
  const h = secs / 3600;
  return h >= 1 ? `${h >= 10 ? Math.round(h) : h.toFixed(1)}h` : `${Math.max(1, Math.round(secs / 60))}m`;
}

function levelRead(level, l, last) {
  const head = `<b>Level ${level}${last ? " so far" : ""}</b>`;
  if (xpMode === "time") {
    const bits = [dur(l.p || 0)];
    if (l.at) bits.push(`reached ${day(l.at * 1000)}`);
    if (l.zone) bits.push(`mostly in ${l.zone}`);
    bits.push(plural(l.k || 0, "kill", "kills"), plural(l.q || 0, "quest", "quests"), plural(l.dd || 0, "death", "deaths"));
    return `${head} · ${bits.map(esc).join(" · ")}`;
  }
  const total = levelXp(l) || 1;
  const parts = XP_PARTS.filter(([k]) => (l[k] || 0) / total >= 0.005).map(
    ([k, name]) => `${name.toLowerCase()} ${Math.round((l[k] / total) * 100)}%`
  );
  const part = l.whole ? "" : " · recorded part only";
  return `${head} · ${num(total)} XP · ${parts.join(" · ")}${part}`;
}

// Below the cap the chart sits beside Gold; with max level focus it is the Levelling section.
function levelsChart(ov, focus = false) {
  const head = segButtons([["xp", "XP sources"], ["time", "Time per level"]], xpMode, "xp");
  const levels = levelRows(ov);
  const cls = focus ? "w12 first last" : "w8 first";
  const title = focus ? "Levelling" : "Experience by level";
  if (!levels.length) {
    const text = xpMode === "xp" ? "No experience recorded" : "No level recorded from its ding to the next one";
    return mod(cls, title, empty(text), head);
  }
  const total = (l) => (xpMode === "xp" ? levelXp(l) : l.p);
  const max = Math.max(...levels.map(([, l]) => total(l)), 1);
  const bars = levels
    .map(([, l]) => {
      const segs =
        xpMode === "xp"
          ? XP_PARTS.filter(([k]) => l[k])
              .map(([k, , c]) => `<i style="height:${(l[k] / max) * 100}%;background:${c}"></i>`)
              .join("")
          : `<i class="c-time" style="height:${(l.p / max) * 100}%"></i>`;
      return `<div class="xp-bar">${segs}</div>`;
    })
    .join("");
  const many = levels.length > 14;
  let lastTick = -9;
  const ticks = levels
    .map(([lv], i) => {
      // Past 14 bars, label every fifth level and the last one if it has room.
      const show = many ? lv % 5 === 0 || i === 0 || (i === levels.length - 1 && lv - lastTick >= 3) : true;
      if (show) lastTick = lv;
      return `<span>${show ? lv : ""}</span>`;
    })
    .join("");
  const yTop = xpMode === "xp" ? axisXp(max) : axisTime(max);
  const yMid = xpMode === "xp" ? axisXp(max / 2) : axisTime(max / 2);
  const key =
    xpMode === "xp"
      ? `<ul class="key">${XP_PARTS.map(([, name, c]) => `<li><i style="background:${c}"></i>${name}</li>`).join("")}</ul>`
      : "";
  return mod(
    cls,
    title,
    `<div class="xp">
      <div class="xp-plot">
        <div class="xp-y"><span>${yTop}</span><span>${yMid}</span><span>0</span></div>
        <div class="xp-bars" id="xp-bars" tabindex="0">${bars}</div>
        <div class="xp-x">${ticks}</div>
      </div>
      ${key}
      <p class="readout" id="xp-out"></p>
    </div>`,
    head
  );
}

function wireLevels(body, ov) {
  const bars = body.querySelector("#xp-bars");
  if (!bars) return;
  const levels = levelRows(ov);
  const out = body.querySelector("#xp-out");
  const lastIdx = levels.length - 1;
  let pinned = xpPinned === null || xpPinned > lastIdx ? lastIdx : xpPinned;
  const read = (i, live) => {
    const [lv, l] = levels[i];
    [...bars.children].forEach((b, k) => b.classList.toggle("on", k === i));
    bars.classList.toggle("has-on", !!live);
    out.innerHTML = levelRead(lv, l, lv === ov.level);
  };
  const at = (e) => {
    const r = bars.getBoundingClientRect();
    return Math.max(0, Math.min(lastIdx, Math.floor(((e.clientX - r.left) / r.width) * levels.length)));
  };
  bars.addEventListener("pointermove", (e) => read(at(e), true));
  bars.addEventListener("pointerleave", () => read(pinned));
  bars.addEventListener("click", (e) => {
    pinned = xpPinned = at(e);
    read(pinned, true);
  });
  bars.addEventListener("blur", () => read(pinned));
  bars.addEventListener("keydown", (e) => {
    const step = e.key === "ArrowLeft" ? -1 : e.key === "ArrowRight" ? 1 : 0;
    if (!step) return;
    e.preventDefault();
    pinned = xpPinned = Math.max(0, Math.min(lastIdx, pinned + step));
    read(pinned, true);
  });
  read(pinned);
}

// Gold -------------------------------------------------------------------------

function goldMod(ov, cls = "last", focus = false) {
  if (ov.money === undefined) return mod(cls, "Gold", empty("No balance recorded"));
  const recent = ov.sessions.slice(0, 7);
  const change = recent.reduce((a, s) => a + net(s), 0);
  let sub = recent.length ? `<p class="sub">${signed(change)} over the last ${plural(recent.length, "session", "sessions")}</p>` : "";
  if (focus) {
    const a = capShare(ov);
    sub =
      `<p class="sub">${moneyHtml(a.g)} earned and ${moneyHtml(a.s)} spent at max level</p>` +
      `<p class="sub">${signed(a.perHour)} an hour</p>` +
      sub;
  }
  const pts = ov.gold.slice(-GOLD_POINTS);
  let plot = "";
  if (pts.length > 1) {
    plot = `<div class="gold-plot">
        <svg id="g-svg" viewBox="0 0 300 100" preserveAspectRatio="none"></svg>
        <div class="gold-axis"><span>${day(pts[0].t * 1000)}</span><span>${day(pts[pts.length - 1].t * 1000)}</span></div>
      </div>
      <p class="readout" id="g-out"></p>`;
  }
  return mod(cls, "Gold", `<p class="big">${moneyHtml(ov.money)}</p>${sub}${plot}`);
}

function wireGold(body, ov) {
  const svg = body.querySelector("#g-svg");
  if (!svg) return;
  const out = body.querySelector("#g-out");
  const pts = ov.gold.slice(-GOLD_POINTS);
  const lo = Math.min(...pts.map((p) => p.m));
  const hi = Math.max(...pts.map((p) => p.m));
  const gx = (i) => (i / (pts.length - 1)) * 300;
  const gy = (m) => 94 - ((m - lo) / (hi - lo || 1)) * 86;
  const line = pts.map((p, i) => `${i ? "L" : "M"}${gx(i).toFixed(1)} ${gy(p.m).toFixed(1)}`).join("");
  svg.innerHTML = `<path d="${line}L300 100L0 100Z" fill="rgba(217,178,95,0.1)"/>
    <path d="${line}" fill="none" stroke="#d9b25f" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
    <line id="g-x" y1="0" y2="100" stroke="rgba(240,241,243,0.5)" stroke-width="1" vector-effect="non-scaling-stroke" visibility="hidden"/>`;
  const rest = () => {
    out.innerHTML = `Between ${moneyHtml(lo)} and ${moneyHtml(hi)} over the last ${plural(pts.length, "session", "sessions")}`;
    svg.querySelector("#g-x").setAttribute("visibility", "hidden");
  };
  svg.addEventListener("pointermove", (e) => {
    const r = svg.getBoundingClientRect();
    const i = Math.max(0, Math.min(pts.length - 1, Math.round(((e.clientX - r.left) / r.width) * (pts.length - 1))));
    const cross = svg.querySelector("#g-x");
    cross.setAttribute("x1", gx(i));
    cross.setAttribute("x2", gx(i));
    cross.setAttribute("visibility", "visible");
    out.innerHTML = `<b>${day(pts[i].t * 1000)} ${hm(pts[i].t * 1000)}</b> · ${moneyHtml(pts[i].m)}`;
  });
  svg.addEventListener("pointerleave", rest);
  rest();
}

// Max level ---------------------------------------------------------------------------
// With an hour at the cap the page leads with what the character does there (docs/OVERVIEW.md,
// "Max level"). The cap level's share of `levels` holds time, gold, runs, kills, deaths and
// quests; `ov.cap` holds the rest.

function capShare(ov) {
  const l = (ov.cap && ov.levels[ov.cap.level]) || {};
  const p = Math.max(l.p || 0, (ov.cap && ov.cap.p) || 0);
  const hours = Math.max(p / 3600, 1);
  const g = l.g || 0;
  const s = l.s || 0;
  return { g, s, r: l.r || 0, k: l.k || 0, dd: l.dd || 0, q: l.q || 0, p, perHour: Math.round((g - s) / hours), hours };
}

function capStrip(ov) {
  const c = ov.cap;
  const a = capShare(ov);
  const first = ov.sessions.length ? ov.sessions[ov.sessions.length - 1].t : 0;
  const since = c.reached ? `since ${day(c.reached * 1000)}` : `since first seen ${first ? day(first * 1000) : ""}`.trim();
  const cells = [
    ["Time at max level", playTime(a.p)],
    ["Gold an hour", signed(a.perHour)],
    ["Dungeons and raids", num(a.r)],
    ["Deaths", num(a.dd)],
    ["Item level", c.ilvl ? num(c.ilvl) : "–"],
    ["Items looted", num(c.loot)],
  ];
  return `<section class="week cap-strip"><div><h2>At max level</h2><p>${esc(since)}</p></div>${cells
    .map(([l, v]) => `<div><dt>${l}</dt><dd>${v}</dd></div>`)
    .join("")}</section>`;
}

function capRunsMod(ov) {
  const c = ov.cap;
  const a = capShare(ov);
  if (!c.runs.length) return mod("", "Dungeons and raids", empty("No dungeon or raid runs at max level"));
  const rows = c.runs.map((r) => ({
    who: r.name,
    v: r.n,
    val: `${plural(r.n, "run", "runs")} · ${dur(r.p)} · ${plural(r.b, "boss", "bosses")}`,
  }));
  return mod("", "Dungeons and raids", `${rank(rows)}<p class="total">${plural(a.r, "run", "runs")} and ${plural(c.bosses, "boss kill", "boss kills")} at max level.</p>`);
}

function capDeathsMod(ov) {
  const c = ov.cap;
  const a = capShare(ov);
  if (!a.dd) return mod("last", "Deaths", empty("No deaths at max level"));
  const rows = c.dz.slice(0, 4).map((z) => ({ who: z.name, v: z.n, val: num(z.n) }));
  const k = c.killers[0];
  const killer = k ? ` Killed most by <span class="who">${esc(k.name)}</span>, ${plural(k.n, "time", "times")}.` : "";
  return mod("last", "Deaths", `${rows.length ? rank(rows) : ""}<p class="total">${plural(a.dd, "death", "deaths")}, ${(a.dd / a.hours).toFixed(1)} an hour.${killer}</p>`);
}

function capRepMod(ov) {
  const c = ov.cap;
  const rows = c.rep.slice(0, 5).map((r) => ({ who: r.name, v: r.n, val: `+${num(r.n)}` }));
  const skills = `<p class="total">${plural(c.skillups, "skill-up", "skill-ups")} at max level.</p>`;
  if (!rows.length) return mod("first", "Reputation and professions", empty("No reputation gained at max level") + skills);
  return mod("first", "Reputation and professions", rank(rows) + skills);
}

function capLegacyMod(ov) {
  const c = ov.cap;
  const rows = [
    ["Legacy Challenges", c.ltotal ? `${num(c.ldone)} of ${num(c.ltotal)}` : num(c.ldone)],
    ["Legacy points", num(c.lpoints)],
    ["Honorable kills", num(c.hks)],
    ["Honor", num(c.honor)],
  ];
  return mod("", "Legacy and PvP", `<div class="cap-list">${rows.map(([l, v]) => `<p><span>${l}</span><b>${v}</b></p>`).join("")}</div>`);
}

// Dungeons, people, deaths --------------------------------------------------------

function runsMod(ov) {
  if (!ov.runs.length) return mod("first", "Dungeons and raids", empty("No dungeon or raid runs recorded"));
  const rows = ov.runs.slice(0, 5).map((r) => ({ who: r.name, v: r.n, val: plural(r.n, "run", "runs") }));
  return mod("first", "Dungeons and raids", `${rank(rows)}<p class="total">${plural(ov.runsTotal, "run", "runs")} in total</p>`);
}

function matesMod(ov, cls = "") {
  if (!ov.mates.length) return mod(cls, "Played with", empty("No group members recorded"));
  const rows = ov.mates.slice(0, 4).map((m) => ({
    who: m.name,
    v: m.p,
    val: `${dur(m.p)} · ${plural(m.n, "session", "sessions")}`,
    color: CLASS_COLOR[m.class] || "",
  }));
  return mod(cls, "Played with", `${rank(rows)}<p class="total">Time grouped together, from the lines written when a player joins or leaves your group.</p>`);
}

function deathsMod(ov) {
  if (!ov.deathZones.length) return mod("last", "Deaths", empty("No deaths recorded"));
  const rows = ov.deathZones.slice(0, 4).map((z) => ({ who: z.name, v: z.n, val: num(z.n) }));
  const all = ov.deathZones.reduce((a, z) => a + z.n, 0);
  const k = ov.killers[0];
  const killer = k ? ` Killed most by <span class="who">${esc(k.name)}</span>, ${plural(k.n, "time", "times")}.` : "";
  return mod("last", "Deaths", `${rank(rows)}<p class="total">${plural(all, "death", "deaths")}.${killer}</p>`);
}

// Play time by day and by zone -----------------------------------------------------

function heatDays(ov) {
  const byDay = new Map();
  for (const s of ov.sessions) {
    const key = dateStr(s.t * 1000);
    const d = byDay.get(key) || { p: 0, n: 0 };
    d.p += s.p || 0;
    d.n += 1;
    byDay.set(key, d);
  }
  const start = mondayOf(new Date());
  start.setDate(start.getDate() - (HEAT_WEEKS - 1) * 7);
  const today = Date.now();
  const days = [];
  for (let i = 0; i < HEAT_WEEKS * 7; i++) {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    const hit = byDay.get(dateStr(d.getTime())) || { p: 0, n: 0 };
    days.push({ t: d.getTime(), p: hit.p, n: hit.n, future: d.getTime() > today });
  }
  return days;
}

const heatLevel = (p) => (p <= 0 ? 0 : p < 3600 ? 1 : p < 7200 ? 2 : p < 14400 ? 3 : 4);

function heatMod(ov) {
  if (!ov.sessions.length) return mod("w5 first", "Play time by day", empty("No sessions recorded"));
  const days = heatDays(ov);
  let months = "<span></span>";
  let was = -1;
  for (let w = 0; w < HEAT_WEEKS; w++) {
    const m = new Date(days[w * 7].t).getMonth();
    months += `<span>${m === was ? "" : MO[m]}</span>`;
    was = m;
  }
  const names = ["Mon", "", "Wed", "", "Fri", "", ""].map((d) => `<span>${d}</span>`).join("");
  const cells = days
    .map((d, i) => `<i data-i="${i}" data-l="${heatLevel(d.p)}"${d.future ? ' class="off"' : ""}></i>`)
    .join("");
  const key = `<p class="heat-key"><span>0</span><i style="background:rgba(255,255,255,.055)"></i><i style="background:#4b1626"></i><i style="background:#861a34"></i><i style="background:#cf1f45"></i><i style="background:#ff5d7e"></i><span>over 4h</span></p>`;
  return mod(
    "w5 first",
    "Play time by day",
    `<div class="heat" id="heat"><div class="heat-months">${months}</div><div class="heat-cells">${names}${cells}</div></div>${key}<p class="readout" id="heat-out"></p>`
  );
}

function wireHeat(body, ov) {
  const heat = body.querySelector("#heat");
  if (!heat) return;
  const days = heatDays(ov);
  const out = body.querySelector("#heat-out");
  const say = (d) => {
    out.innerHTML = d.p
      ? `<b>${day(d.t)}</b> · ${playTime(d.p)} in ${plural(d.n, "session", "sessions")}`
      : `<b>${day(d.t)}</b> · not played`;
  };
  const pick = (e) => {
    const c = e.target.closest("i[data-i]");
    if (!c || c.classList.contains("off")) return;
    heat.querySelectorAll("i.on").forEach((n) => n.classList.remove("on"));
    c.classList.add("on");
    say(days[+c.dataset.i]);
  };
  heat.addEventListener("pointerover", pick);
  heat.addEventListener("click", pick);
  const played = days.filter((d) => d.p);
  if (played.length) say(played[played.length - 1]);
}

function zonesMod(ov) {
  const zones = [...ov.zones].filter((z) => z.p > 0).sort((a, b) => b.p - a.p);
  if (!zones.length) return mod("w7 last", "Time by zone", empty("No zones recorded"));
  const rows = zones.slice(0, 6).map((z) => ({ who: z.name, v: z.p, val: `${longDur(z.p)} · ${plural(z.k || 0, "kill", "kills")}` }));
  const more = zones.length > 6 ? `<p class="total">Top 6 of ${num(zones.length)} zones.</p>` : "";
  return mod("w7 last", "Time by zone", rank(rows) + more);
}

// Play history ----------------------------------------------------------------------

const th = (label, cls = "") => `<th${cls ? ` class="${cls}"` : ""}>${label}</th>`;

function levelSpan(lo, hi) {
  if (!lo && !hi) return "";
  return lo === hi || !lo ? String(hi) : `${lo} → ${hi}`;
}

function historyRows(ov) {
  if (histMode === "levels") {
    const rows = Object.entries(ov.levels)
      .map(([lv, l]) => [Number(lv), l])
      .sort((a, b) => b[0] - a[0]);
    return {
      head: [th("Level"), th("Reached"), th("Played", "num"), th("XP", "num"), th("Kills", "num"), th("Quests", "num"), th("Deaths", "num"), th("Main zone")],
      rows: rows.map(
        ([lv, l]) => `<td>Level ${lv}</td><td>${l.at ? dateStr(l.at * 1000) : "Before recording"}</td>
          <td class="num">${l.complete ? playTime(l.p || 0) : `<span class="dim" title="Not recorded from its ding to the next one">–</span>`}</td><td class="num">${num(levelXp(l))}</td><td class="num">${num(l.k || 0)}</td>
          <td class="num">${num(l.q || 0)}</td><td class="num">${num(l.dd || 0)}</td><td>${esc(l.zone || "")}</td>`
      ),
      name: "levels",
    };
  }
  const head = [th(histMode === "days" ? "Day" : "When"), th("Played"), th("Levels"), th("Quests", "num"), th("Gold", "num"), th("Zones", "num")];
  let rows;
  if (histMode === "days") {
    const days = new Map();
    for (const s of ov.sessions) {
      const key = dateStr(s.t * 1000);
      const d = days.get(key) || { t: s.t * 1000, p: 0, lo: 0, hi: 0, q: 0, gold: 0, zones: new Set() };
      d.p += s.p || 0;
      d.lo = d.lo ? Math.min(d.lo, s.l0 || d.lo) : s.l0 || 0;
      d.hi = Math.max(d.hi, s.l1 || 0);
      d.q += s.q || 0;
      d.gold += net(s);
      Object.keys(s.zn || {}).forEach((z) => d.zones.add(z));
      days.set(key, d);
    }
    rows = [...days.values()].map((d) => [day(d.t), d.p, levelSpan(d.lo, d.hi), d.q, d.gold, d.zones.size]);
  } else {
    rows = ov.sessions.map((s) => [`${day(s.t * 1000)} ${hm(s.t * 1000)}`, s.p || 0, levelSpan(s.l0 || 0, s.l1 || 0), s.q || 0, net(s), s.z || 0]);
  }
  return {
    head,
    rows: rows.map(
      ([when, p, lv, q, gold, z]) =>
        `<td>${esc(when)}</td><td>${playTime(p)}</td><td>${esc(lv)}</td><td class="num">${num(q)}</td><td class="num">${signed(gold)}</td><td class="num">${num(z)}</td>`
    ),
    name: histMode,
  };
}

function historyMod(ov) {
  const head = segButtons([["sessions", "Sessions"], ["days", "Days"], ["levels", "Levels"]], histMode, "hist");
  if (!ov.sessions.length && histMode !== "levels") return mod("w12 first last", "Play history", empty("No sessions recorded"), head);
  const { head: cols, rows, name } = historyRows(ov);
  if (!rows.length) return mod("w12 first last", "Play history", empty("No levels recorded"), head);
  const cap = histAll ? HISTORY_MAX : HISTORY_ROWS;
  const body = rows.slice(0, cap).map((r) => `<tr>${r}</tr>`).join("");
  let more = "";
  if (rows.length > HISTORY_ROWS) {
    more = `<button class="linkbtn" id="hist-more" type="button">${histAll ? `Show the latest ${HISTORY_ROWS}` : `Show ${rows.length > HISTORY_MAX ? `the newest ${HISTORY_MAX} of ${num(rows.length)}` : `all ${num(rows.length)}`} ${name}`}</button>`;
  }
  return mod("w12 first last", "Play history", `<div class="history"><table><thead><tr>${cols.join("")}</tr></thead><tbody>${body}</tbody></table></div>${more}`, head);
}

// Page --------------------------------------------------------------------------------

function draw(body) {
  const top = body.scrollTop;
  body.innerHTML = "";
  if (!current) {
    body.innerHTML = `<section class="panel"><h3>Overview</h3>${empty("No characters recorded yet")}</section>`;
    return;
  }
  const { name, ov, eta, focus } = current;
  // An hour at the cap: max level data first, the levelling content in its own section below.
  const sawLevelling = !(ov.firstLevel && ov.cap && ov.firstLevel >= ov.cap.level);
  const layout = focus
    ? `${header(ov, name, eta)}${capStrip(ov)}
    <div class="sheet">${goldMod(ov, "first", true)}${capRunsMod(ov)}${capDeathsMod(ov)}${capRepMod(ov)}${capLegacyMod(ov)}${matesMod(ov, "last")}${heatMod(ov)}${zonesMod(ov)}${sawLevelling ? levelsChart(ov, true) : ""}${historyMod(ov)}</div>`
    : `${header(ov, name, eta)}${week(ov)}
    <div class="sheet">${levelsChart(ov)}${goldMod(ov)}${runsMod(ov)}${matesMod(ov)}${deathsMod(ov)}${heatMod(ov)}${zonesMod(ov)}${historyMod(ov)}</div>`;
  body.insertAdjacentHTML("beforeend", layout);
  wireLevels(body, ov);
  wireGold(body, ov);
  wireHeat(body, ov);

  body.querySelectorAll("[data-xp]").forEach((n) => {
    n.onclick = () => {
      xpMode = n.dataset.xp;
      xpPinned = null;
      draw(body);
    };
  });
  body.querySelectorAll("[data-hist]").forEach((n) => {
    n.onclick = () => {
      histMode = n.dataset.hist;
      histAll = false;
      draw(body);
    };
  });
  const more = body.querySelector("#hist-more");
  if (more) {
    more.onclick = () => {
      histAll = !histAll;
      draw(body);
    };
  }
  body.scrollTop = top;
}

export async function render(body) {
  current = await invoke("overview", { ch: state.ch });
  xpPinned = null;
  draw(body);
}
