// Overview: one character at a glance (docs/OVERVIEW.md, "The page").
// The model comes from the `overview` command, the same rows the export
// writes to ScarletData\Digest.lua for the game. Section order: header, this week,
// experience by level and gold, dungeons, played with and deaths, play time
// by day and by zone, play history.

import { invoke, esc, displayName, num, dur, moneyHtml, dateStr, dateTime, SERIES, state, wireTips } from "../lib.js";
import { countMarked, play } from "../motion.js";
import { goalsSheet } from "./goals-section.js";

export const CLASS_COLOR = {
  DEATHKNIGHT: "#E04A64",
  DEMONHUNTER: "#C46CE6",
  DRUID: "#FF8A2B",
  EVOKER: "#4AB8A2",
  HUNTER: "#AAD372",
  MAGE: "#5CC8EE",
  MONK: "#00FF98",
  PALADIN: "#F48CBA",
  PRIEST: "#F0F1F3",
  ROGUE: "#FFF468",
  SHAMAN: "#4A94FF",
  WARLOCK: "#A79BF5",
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
const MAX_PIPS = 40;

let xpMode = "xp";
let histMode = "sessions";
let histAll = false;
let xpPinned = null;
let current = null;
let life = null;
let goals = null;
let groups = null;
let sessionMode = "auto";
let renders = 0;
let entering = false;

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

const atLevelCap = (ov) => Boolean(ov.cap && ov.cap.level > 0 && ov.level >= ov.cap.level);

function header(ov, name) {
  const color = CLASS_COLOR[ov.class] || "#ff2d55";
  const identity = [ov.level ? `Level ${ov.level}` : "", raceName(ov.race), className(ov.class)].filter(Boolean).join(" ");
  const bits = [identity, ov.faction, ov.zone ? `in ${ov.zone}` : "", ov.played ? `${longDur(ov.played)} played` : ""].filter(Boolean);
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
      <h1 class="ch-name">${esc(displayName(name))}</h1>
      <p class="ch-sub"><i class="dot"></i>${bits.map((b) => `<span>${esc(b)}</span>`).join("")}</p>
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
    .map(([l, v]) => `<dl><dt>${l}</dt><dd>${v}</dd></dl>`)
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
    const text = xpMode === "xp" ? "No experience recorded." : "No full level recorded from start to finish.";
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
  const pts = goldPoints(ov);
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

function goldPoints(ov) {
  const own = ov.gold.slice(-GOLD_POINTS);
  const before = ((ov.imported && ov.imported.gold) || []).filter((p) => !own.length || p.t < own[0].t).slice(-GOLD_POINTS);
  const source = ov.imported ? ov.imported.sources.join(" and ") : "";
  return [...before.map((p) => ({ ...p, from: source })), ...own];
}

function wireGold(body, ov) {
  const svg = body.querySelector("#g-svg");
  if (!svg) return;
  const out = body.querySelector("#g-out");
  const pts = goldPoints(ov);
  const lead = pts.filter((p) => p.from).length;
  const lo = Math.min(...pts.map((p) => p.m));
  const hi = Math.max(...pts.map((p) => p.m));
  const gx = (i) => (i / (pts.length - 1)) * 300;
  const gy = (m) => 94 - ((m - lo) / (hi - lo || 1)) * 86;
  const trace = (from, to) => pts.slice(from, to).map((p, k) => `${k ? "L" : "M"}${gx(from + k).toFixed(1)} ${gy(p.m).toFixed(1)}`).join("");
  const start = Math.max(lead - 1, 0);
  const line = trace(start, pts.length);
  const earlier = lead > 1 ? trace(0, lead) : "";
  svg.innerHTML = `<path d="${line}L300 100L${gx(start).toFixed(1)} 100Z" fill="rgba(217,178,95,0.1)"/>
    ${earlier ? `<path d="${earlier}" fill="none" stroke="#d9b25f" stroke-width="2" stroke-dasharray="5 4" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>` : ""}
    <path d="${line}" fill="none" stroke="#d9b25f" stroke-width="2" stroke-linejoin="round" stroke-linecap="round" vector-effect="non-scaling-stroke"/>
    <line id="g-x" y1="0" y2="100" stroke="rgba(240,241,243,0.5)" stroke-width="1" vector-effect="non-scaling-stroke" visibility="hidden"/>`;
  const rest = () => {
    out.innerHTML = `Between ${moneyHtml(lo)} and ${moneyHtml(hi)} over the last ${plural(pts.length - lead, "session", "sessions")}${lead ? ` and ${plural(lead, "earlier day", "earlier days")} from ${esc(pts[0].from)}` : ""}`;
    svg.querySelector("#g-x").setAttribute("visibility", "hidden");
  };
  svg.addEventListener("pointermove", (e) => {
    const r = svg.getBoundingClientRect();
    const i = Math.max(0, Math.min(pts.length - 1, Math.round(((e.clientX - r.left) / r.width) * (pts.length - 1))));
    const cross = svg.querySelector("#g-x");
    cross.setAttribute("x1", gx(i));
    cross.setAttribute("x2", gx(i));
    cross.setAttribute("visibility", "visible");
    out.innerHTML = pts[i].from
      ? `<b>${day(pts[i].t * 1000)}</b> · ${moneyHtml(pts[i].m)} · ${esc(pts[i].from)}`
      : `<b>${day(pts[i].t * 1000)} ${hm(pts[i].t * 1000)}</b> · ${moneyHtml(pts[i].m)}`;
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
    .map(([l, v]) => `<dl><dt>${l}</dt><dd>${v}</dd></dl>`)
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
  return mod("first", "Dungeons and raids", `${rank(rows)}<p class="total">${plural(ov.runsTotal, "run", "runs")} in total.</p>`);
}

function matesMod(ov, cls = "") {
  if (!ov.mates.length) return mod(cls, "Played with", empty("No group members recorded."));
  const rows = ov.mates.slice(0, 4).map((m) => ({
    who: displayName(m.name),
    v: m.p,
    val: `${dur(m.p)} · ${plural(m.n, "session", "sessions")}`,
    color: CLASS_COLOR[m.class] || "",
  }));
  return mod(cls, "Played with", `${rank(rows)}<p class="total">Time grouped together, from when a player joins and leaves your group.</p>`);
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

function levelSpan(lo, hi) {
  if (!lo && !hi) return "";
  return lo === hi || !lo ? String(hi) : `${lo} → ${hi}`;
}

function levelChip(lo, hi) {
  if (!lo && !hi) return `<span class="ph-lv"></span>`;
  if (lo && hi && lo !== hi) return `<span class="ph-lv"><em class="ph-chip" data-tip="Level ${lo} to level ${hi}">${lo} → ${hi}</em></span>`;
  return `<span class="ph-lv"><em class="ph-at">Level ${hi || lo}</em></span>`;
}

function rowFigures(rows) {
  const zones = new Set(rows.flatMap((s) => Object.keys(s.zn || {})));
  const lows = rows.map((s) => s.l0 || 0).filter(Boolean);
  return {
    p: rows.reduce((a, s) => a + (s.p || 0), 0),
    lo: lows.length ? Math.min(...lows) : 0,
    hi: Math.max(0, ...rows.map((s) => s.l1 || 0)),
    q: rows.reduce((a, s) => a + (s.q || 0), 0),
    gold: rows.reduce((a, s) => a + net(s), 0),
    zones: zones.size || Math.max(0, ...rows.map((s) => s.z || 0)),
  };
}

const secs = (ms) => Math.floor(ms / 1000);

function holdsStart(s, g) {
  return s.t >= secs(g.from) && s.t * 1000 <= g.to;
}

function overlapsGroup(s, g) {
  return s.t * 1000 < g.to && (s.t + (s.p || 0)) * 1000 > g.from;
}

function sessionsByGroup(ov, list) {
  const bins = list.map(() => []);
  const oldestFirst = list.map((_, i) => i).reverse();
  for (const s of ov.sessions) {
    const at = oldestFirst.find((i) => holdsStart(s, list[i])) ?? oldestFirst.find((i) => overlapsGroup(s, list[i]));
    if (at !== undefined) bins[at].push(s);
  }
  return bins;
}

function sessionGroups(ov) {
  if (!groups || !groups.length) {
    return ov.sessions.map((s) => ({ from: s.t * 1000, to: (s.t + (s.p || 0)) * 1000, live: false, linked: false, ...rowFigures([s]) }));
  }
  const bins = sessionsByGroup(ov, groups);
  return groups.map((g, i) => {
    const f = rowFigures(bins[i]);
    return { from: g.from, to: g.to, live: g.live, linked: true, ...f, p: sessionMode === "manual" || !f.p ? g.played : f.p };
  });
}

const figure = (n, one, many) => (n ? `<span class="ph-fig"><b>${num(n)}</b> ${n === 1 ? one : many}</span>` : `<span class="ph-fig zero">–</span>`);

const figures = (q, gold, zones) =>
  `${figure(q, "quest", "quests")}${gold ? `<span class="ph-fig">${signed(gold)}</span>` : `<span class="ph-fig zero">–</span>`}${figure(zones, "zone", "zones")}`;

const playedBar = (p, most) => `<span class="ph-played"><b>${playTime(p)}</b><i style="--w:${p ? Math.max(1.5, (p / most) * 100) : 0}%"></i></span>`;

function sessionRows(ov) {
  const all = sessionGroups(ov);
  const most = Math.max(...all.map((g) => g.p), 1);
  return {
    name: "sessions",
    rows: all.map((g) => {
      const when = g.live ? "In progress" : `${day(g.from)} <small>${hm(g.from)}</small>`;
      const cells = `<span class="ph-when">${when}</span>${playedBar(g.p, most)}${levelChip(g.lo, g.hi)}${figures(g.q, g.gold, g.zones)}`;
      return g.linked ? `<a class="ph-row" href="#sessions/${g.from}-${g.to}">${cells}</a>` : `<div class="ph-row">${cells}</div>`;
    }),
  };
}

function dayRows(ov) {
  const days = new Map();
  for (const s of ov.sessions) {
    const key = dateStr(s.t * 1000);
    const d = days.get(key) || { t: s.t * 1000, rows: [] };
    d.rows.push(s);
    days.set(key, d);
  }
  const all = [...days.values()].map((d) => ({ t: d.t, ...rowFigures(d.rows) }));
  const most = Math.max(...all.map((d) => d.p), 1);
  return {
    name: "days",
    rows: all.map((d) => `<div class="ph-row"><span class="ph-when">${day(d.t)}</span>${playedBar(d.p, most)}${levelChip(d.lo, d.hi)}${figures(d.q, d.gold, d.zones)}</div>`),
  };
}

function levelRowsHtml(ov) {
  const all = Object.entries(ov.levels)
    .map(([lv, l]) => [Number(lv), l])
    .sort((a, b) => b[0] - a[0]);
  const most = Math.max(...all.map(([, l]) => (l.complete ? l.p || 0 : 0)), 1);
  return {
    name: "levels",
    rows: all.map(([lv, l]) => {
      const bar = l.complete
        ? playedBar(l.p || 0, most)
        : `<span class="ph-played part" data-tip="Not recorded for the whole level"><b>–</b><i style="--w:0%"></i></span>`;
      const bits = [`${num(levelXp(l))} XP`, plural(l.k || 0, "kill", "kills"), plural(l.q || 0, "quest", "quests"), plural(l.dd || 0, "death", "deaths")];
      const where = [l.zone, l.at ? `reached ${dateStr(l.at * 1000)}` : "before recording"].filter(Boolean);
      return `<div class="ph-row ph-level"><span class="ph-when">Level ${lv}</span>${bar}<span class="ph-bits">${esc(bits.join(" · "))}</span><span class="ph-where">${esc(where.join(" · "))}</span></div>`;
    }),
  };
}

function historyMod(ov) {
  const head = segButtons([["sessions", "Sessions"], ["days", "Days"], ["levels", "Levels"]], histMode, "hist");
  if (!ov.sessions.length && histMode !== "levels") return mod("w12 first last", "Play history", empty("No sessions recorded"), head);
  const { rows, name } = histMode === "levels" ? levelRowsHtml(ov) : histMode === "days" ? dayRows(ov) : sessionRows(ov);
  if (!rows.length) return mod("w12 first last", "Play history", empty("No levels recorded"), head);
  const cap = histAll ? HISTORY_MAX : HISTORY_ROWS;
  let more = "";
  if (rows.length > HISTORY_ROWS) {
    more = `<button class="linkbtn" id="hist-more" type="button">${histAll ? `Show the latest ${HISTORY_ROWS}` : `Show ${rows.length > HISTORY_MAX ? `the latest ${HISTORY_MAX} of ${num(rows.length)}` : `all ${num(rows.length)}`} ${name}`}</button>`;
  }
  return mod("w12 first last", "Play history", `<div class="ph-list ph-${name}">${rows.slice(0, cap).join("")}</div>${more}`, head);
}


const IMPORT_DAYS = 14;
const IMPORT_ITEMS = 10;

function importedMod(ov) {
  const im = ov.imported;
  if (!im || (!im.sales && !im.buys && !im.gold.length)) return "";
  const source = im.sources.join(" and ");
  const seen = im.dropped.sales + im.dropped.buys;
  const lines = [
    im.sales || im.buys
      ? `<p class="sub">${plural(im.sales, "sale", "sales")} for ${moneyHtml(im.sales_copper)} and ${plural(im.buys, "purchase", "purchases")} for ${moneyHtml(im.buys_copper)}, ${dateStr(im.first)} to ${dateStr(im.last)}.</p>`
      : "",
    `<p class="sub">${esc(source)} kept these and Scarlet did not.${seen ? ` ${num(seen)} more matched what Scarlet recorded and are not counted twice.` : ""} Other views and totals leave them out.</p>`,
  ].join("");
  const days = im.days.slice(0, IMPORT_DAYS);
  const peak = Math.max(...days.flatMap((d) => [d.sold_copper, d.bought_copper]), 1);
  const side = (n, copper, cls, verb) =>
    !n && !copper
      ? `<span class="im-side none"></span>`
      : `<span class="im-side ${cls}"><span class="im-t"><b>${moneyHtml(copper)}</b> ${verb} ${plural(n, "item", "items")}</span><i style="--w:${copper ? Math.max(1.5, (copper / peak) * 100) : 0}%"></i></span>`;
  const dayRows = days.length
    ? `<h3>Sales and purchases by day</h3><ul class="sc-key im-key"><li><i style="background:var(--alt-4)"></i>Received</li><li><i style="background:var(--alt-1)"></i>Paid</li></ul>
      <div class="im-list">${days
        .map((d) => `<div class="im-day"><span class="im-when">${day(d.d * 1000)}</span>${side(d.sold, d.sold_copper, "got", "for")}${side(d.bought, d.bought_copper, "paid", "for")}</div>`)
        .join("")}</div>`
    : "";
  const sold = im.items.filter((i) => i.sold > 0).sort((a, b) => b.sold_copper - a.sold_copper).slice(0, IMPORT_ITEMS);
  const top = Math.max(...sold.map((i) => i.sold_copper), 1);
  const itemRows = sold.length
    ? `<h3>Sale prices by item</h3><div class="im-list">${sold
        .map(
          (i) => `<div class="im-item"><span class="im-name">${esc(i.name || `Item ${i.id}`)}</span>
            <span class="im-side got"><span class="im-t"><b>${moneyHtml(i.sold_copper)}</b> received</span><i style="--w:${Math.max(1.5, (i.sold_copper / top) * 100)}%"></i></span>
            <span class="im-n"><b>${num(i.sold)}</b> sold</span><span class="im-each">${i.low === i.high ? moneyHtml(i.low) : `${moneyHtml(i.low)} to ${moneyHtml(i.high)}`} each</span></div>`
        )
        .join("")}</div>`
    : "";
  return mod("w12 first last", `Imported from ${source}`, lines + dayRows + itemRows);
}

const until = (sec) => {
  const d = sec - Date.now() / 1000;
  if (d <= 0) return "reset";
  const days = Math.floor(d / 86400);
  const hours = Math.floor((d % 86400) / 3600);
  return days ? `in ${days}d ${hours}h` : `in ${hours}h ${Math.floor((d % 3600) / 60)}m`;
};

function lockRow(l) {
  const known = l.done != null && l.total != null && l.total > 0;
  const bar =
    known && l.total <= MAX_PIPS
      ? `<span class="lk-pips" data-tip="${l.done} of ${l.total} bosses defeated">${Array.from({ length: l.total }, (_, i) => `<i${i < l.done ? ' class="on"' : ""}></i>`).join("")}</span>`
      : "";
  const pips = known ? `${bar}<b>${l.done}/${l.total}</b>` : `<span class="dim">–</span>`;
  const difficulty = l.kind === "world boss" ? "World boss" : l.difficulty || "–";
  return `<div class="lk-row"><span class="lk-name">${esc(l.name)}${l.extended ? ' <em class="ph-at">extended</em>' : ""}</span><span class="lk-diff">${esc(difficulty)}</span>
    <span class="lk-boss">${pips}</span><span class="lk-reset" data-tip="${dateTime(l.reset_at * 1000)}">Resets ${until(l.reset_at)}</span></div>`;
}

function lifetimeMod(life) {
  if (!life) return "";
  const counts = [
    ["Quests completed", life.quests],
    ["Titles known", life.titles],
    ["Zone maps explored", life.explored],
  ].filter(([, c]) => c?.total);
  if (!counts.length && !life.locks.length) return "";
  const countTiles = counts.length
    ? `<div class="sc-grid" style="grid-template-columns:repeat(${counts.length},minmax(0,1fr))">${counts
        .map(([label, c]) => `<div class="sc-tile"><span class="sc-label">${label}</span><span class="sc-num sc-num-xl" data-to="${c.total}" data-as="num">${num(c.total)}</span><span class="sc-sub">Last read ${dateStr(c.at * 1000)}</span></div>`)
        .join("")}</div>`
    : "";
  const lockRows = life.locks.length ? `<h3>Instance locks</h3><div class="lk-list">${life.locks.map(lockRow).join("")}</div>` : "";
  return mod("w12 first last", "Whole life", countTiles + lockRows);
}

// Page --------------------------------------------------------------------------------

function selectShownCharacter() {
  if (state.ch || !current) return;
  const select = document.querySelector("#char");
  if (select) select.value = current.name;
  state.ch = current.name;
}

function draw(body, refocus = null) {
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
    ? `${header(ov, name)}${capStrip(ov)}
    <div class="sheet">${lifetimeMod(life)}${goldMod(ov, "first", true)}${capRunsMod(ov)}${capDeathsMod(ov)}${capRepMod(ov)}${capLegacyMod(ov)}${matesMod(ov, "last")}${heatMod(ov)}${zonesMod(ov)}${sawLevelling ? levelsChart(ov, true) : ""}${historyMod(ov)}${importedMod(ov)}</div>`
    : `${header(ov, name)}${week(ov)}
    <div class="sheet">${lifetimeMod(life)}${levelsChart(ov)}${goldMod(ov)}${runsMod(ov)}${matesMod(ov)}${deathsMod(ov)}${heatMod(ov)}${zonesMod(ov)}${historyMod(ov)}${importedMod(ov)}</div>`;
  body.insertAdjacentHTML("beforeend", layout);
  if (goals) body.querySelector(".ch-head").after(goalsSheet(goals, ov, eta, { atCap: atLevelCap(ov), onOpen: selectShownCharacter }));
  countMarked(body, { num, money: moneyHtml });
  wireLevels(body, ov);
  wireGold(body, ov);
  wireHeat(body, ov);

  body.querySelectorAll("[data-xp]").forEach((n) => {
    n.onclick = () => {
      xpMode = n.dataset.xp;
      xpPinned = null;
      draw(body, `[data-xp="${xpMode}"]`);
    };
  });
  body.querySelectorAll("a.ph-row").forEach((link) => {
    link.onclick = selectShownCharacter;
  });
  body.querySelectorAll("[data-hist]").forEach((n) => {
    n.onclick = () => {
      histMode = n.dataset.hist;
      histAll = false;
      draw(body, `[data-hist="${histMode}"]`);
    };
  });
  const more = body.querySelector("#hist-more");
  if (more) {
    more.onclick = () => {
      histAll = !histAll;
      draw(body, "#hist-more");
    };
  }
  body.scrollTop = top;
  if (refocus) body.querySelector(refocus)?.focus({ preventScroll: true });
  if (entering) {
    entering = false;
    play(body);
  }
}

export const shownName = () => current?.name ?? null;

async function load(ch) {
  const first = ch ? null : await invoke("overview", { ch });
  const name = ch ?? first?.name ?? null;
  const [overview, lifetime, listed, reached] = await Promise.all([
    first ?? invoke("overview", { ch }),
    name ? invoke("lifetime", { ch: name }) : null,
    name ? invoke("session_list", { ch: name }).catch(() => null) : null,
    name ? invoke("goals", { ch: name }).catch(() => null) : null,
  ]);
  return { overview, lifetime, listed, reached };
}

export async function render(body, ch = state.ch) {
  wireTips();
  const mine = ++renders;
  const { overview, lifetime, listed, reached } = await load(ch);
  if (mine !== renders) return;
  current = overview;
  life = lifetime;
  goals = reached;
  groups = listed?.sessions ?? null;
  sessionMode = listed?.prefs.mode ?? "auto";
  xpPinned = null;
  entering = true;
  draw(body);
}
