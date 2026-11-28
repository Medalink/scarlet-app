// Characters: every recorded character at once, shown when no character is selected.
// The model comes from the `roster` command. Section order: totals, characters by game
// account, gold and transfers, played together, play time by day, time per level, professions.

import { invoke, state, el, elText, esc, displayName, num, dur, money, moneyHtml, dayKey, shortDay, weekdayDay, weekStart, stampHtml, goal, wireTips, lineChart, wireLineCharts } from "../lib.js";
import { countMarked, play } from "../motion.js";
import { FALLBACK_CAP, TIERS, recordTiles } from "./goals-section.js";
import { openAs, paceTiles } from "./paces.js";
import { CLASS_COLOR, className, raceName, longDur, playTime, signed, empty, mod, axisTime, plural } from "./overview.js";

const DAYS = 30;
const DAY_TICKS = 6;
const TRANSFER_ROWS = 8;
const TONES = 8;

let data = null;
let account = null;
let transfersAll = false;
let entering = false;

const tone = (i) => `var(--alt-${(i % TONES) + 1})`;
const hue = (i) => CLASS_COLOR[data.chars[i].class] || tone(i);
const sum = (rows, f) => rows.reduce((a, r) => a + f(r), 0);
const named = (c) => `<span class="who-c"><i class="dot-c" style="background:${CLASS_COLOR[c.class] || "var(--muted)"}"></i>${esc(displayName(c.name))}</span>`;
const nameOf = (name) => {
  const c = data.chars.find((x) => x.name === name);
  return c ? named(c) : esc(displayName(name));
};
const key = () =>
  `<ul class="key">${data.chars.map((c, i) => `<li><i style="background:${hue(i)}"></i>${esc(displayName(c.name))}</li>`).join("")}</ul>`;

function pick(name) {
  document.querySelector("#char").value = name;
  state.ch = name;
  location.hash = "character";
}

function totals() {
  const monday = weekStart().getTime();
  const week = sum(data.chars, (c) => sum(c.sessions.filter((s) => s[0] * 1000 >= monday), (s) => s[1]));
  const cells = [
    ["Characters", num(data.chars.length)],
    ["Played", longDur(sum(data.chars, (c) => c.played))],
    ["Gold", moneyHtml(sum(data.chars, (c) => c.money || 0) + data.warband)],
    ["This week", playTime(week)],
  ];
  return `<section class="week" style="--n:${cells.length}"><div><h2>Totals</h2></div>${cells.map(([l, v]) => `<dl><dt>${l}</dt><dd>${v}</dd></dl>`).join("")}</section>`;
}


function capGoals(c) {
  const out = [];
  if (c.level) {
    const eta = c.reach?.date ? `Level ${c.reach.level} ${c.reach.date}` : "";
    out.push(goal(c.level, c.cap > 0 ? c.cap : FALLBACK_CAP, { label: "Level", eta, count: false }));
  }
  const caps = account?.chars.find((x) => x.name === c.name);
  if (caps?.skillsKnown) out.push(goal(caps.skillsMaxed, caps.skillsKnown, { label: "Skills maxed", count: false }));
  if (caps?.rep) out.push(goal(caps.rep.standing - caps.rep.from, caps.rep.to - caps.rep.from, { label: caps.rep.name, note: TIERS[caps.rep.tier], count: false }));
  return out;
}

function charCard(c, withAccount) {
  const time = c.played || c.recorded;
  const played = time ? `${longDur(time)}${c.played ? "" : " recorded"}` : "–";
  const last = c.through ? `<b>${stampHtml(c.through * 1000, "", { tip: true })}</b>` : "<b>–</b>";
  const acct = withAccount && c.account ? `<span class="ch-acct">${esc(c.account)}</span>` : "";
  return `<div class="ch-card" role="link" tabindex="0" data-pick="${esc(c.name)}" style="--cls:${CLASS_COLOR[c.class] || "var(--accent)"}">
    <div class="ch-top"><span class="ch-name">${esc(displayName(c.name))}</span></div>
    <p class="ch-id">${esc(raceName(c.race))} ${esc(className(c.class))}${c.zone ? ` · ${esc(c.zone)}` : ""}${acct}</p>
    <div class="ch-caps" data-caps="${esc(c.name)}"></div>
    <dl class="ch-facts">
      <div><dt>Played</dt><dd>${played}</dd></div>
      <div><dt>Gold</dt><dd>${c.money == null ? "–" : `<span data-to="${c.money}" data-as="money">${moneyHtml(c.money)}</span>`}</dd></div>
      <div><dt>Last session</dt><dd>${last}</dd></div>
    </dl>
  </div>`;
}

function charGrid(list, withAccount) {
  const cards = [...list].sort((a, b) => b.through - a.through).map((c) => charCard(c, withAccount));
  return `<div class="ch-grid">${cards.join("")}</div>`;
}

function fillCaps(body) {
  for (const slot of body.querySelectorAll("[data-caps]")) {
    const c = data.chars.find((x) => x.name === slot.dataset.caps);
    if (c) slot.append(...capGoals(c));
  }
}

function groupMod(group) {
  const byName = new Map(data.chars.map((c) => [c.name, c]));
  const list = group.characters.map((n) => byName.get(n)).filter(Boolean);
  const title = group.account ? `Game account ${group.account}` : "Other characters";
  return mod("w12 first last", title, charGrid(list, false));
}

function charactersMod() {
  const accounts = new Set(data.chars.map((c) => c.account).filter(Boolean));
  return mod("w12 first last", "Characters", charGrid(data.chars, accounts.size > 1));
}


const combined = () => data.gold.series.map(([t, m]) => ({ t, m }));

function goldMod() {
  const total = sum(data.chars, (c) => c.money || 0) + data.warband;
  const pts = combined();
  const plot = pts.length > 1 ? `<p class="sc-label gold-what">Gold held by all characters</p>${lineChart(pts.map((p) => [p.t * 1000, p.m]), "#d9b25f", 120, money)}` : "";
  const nets = data.chars.map((c) => sum(c.sessions, (x) => x[2] - x[3]));
  const swing = Math.max(...nets.map(Math.abs), 1);
  const biggest = Math.max(...data.chars.map((c) => c.money || 0), data.warband, 1);
  const share = (v) => {
    const pct = total > 0 ? (v / total) * 100 : 0;
    return pct >= 1 ? `${Math.round(pct)}%` : pct > 0 ? "Under 1%" : "";
  };
  const balance = (v) =>
    `<div class="gl-bal"><div class="gl-t"><span data-to="${v}" data-as="money">${moneyHtml(v)}</span><b>${share(v)}</b></div><i style="--w:${v ? Math.max(1, (v / biggest) * 100) : 0}%"></i></div>`;
  const flow = (v, cls, label) => (v ? `<span class="gl-${cls}" data-tip="${label}|${esc(money(v))}">${moneyHtml(v)} <small>${cls}</small></span>` : `<span class="gl-${cls} none">–</span>`);
  const moved = data.chars.some((c) => c.received || c.sent);
  const rows = data.chars
    .map((c, i) => {
      const n = nets[i];
      const side = n < 0 ? "neg" : "pos";
      return `<div class="gl-row"><span class="gl-who">${named(c)}</span>${c.money == null ? `<span class="dim">–</span>` : balance(c.money)}
        <div class="gl-net"><div class="gl-t">${signed(n)}</div><span class="gl-div"><i class="${side}" style="--w:${(Math.abs(n) / swing) * 50}%"></i></span></div>
        ${moved ? `<div class="gl-io">${flow(c.received, "in", "Received from your other characters")}${flow(c.sent, "out", "Sent to your other characters")}</div>` : ""}</div>`;
    })
    .join("");
  const warband = data.warband ? `<div class="gl-row"><span class="who-c">Warband bank</span>${balance(data.warband)}<span class="dim">–</span>${moved ? "<span></span>" : ""}</div>` : "";
  const head = `<div class="gl-row gl-head"><span>Character</span><span>Balance and share</span><span>Net without transfers</span>${moved ? "<span>Received and sent</span>" : ""}</div>`;
  const table = `<div class="gl-list${moved ? "" : " no-io"}">${head}${rows}${warband}</div>`;
  return mod(data.transfers.length ? "w8 first" : "w12 first last", "Gold", `<p class="big">${moneyHtml(total)}</p><div class="gold-goals" id="r-gold-goals"></div>${plot}${table}`);
}

function fillGoldGoals(body) {
  const slot = body.querySelector("#r-gold-goals");
  if (!slot) return;
  const held = sum(data.chars, (c) => c.money || 0);
  const [bestAt, best] = data.gold.best ?? [0, 0];
  if (best > 0 && held > 0) {
    if (held >= best) {
      const line = el("p", "gold-best");
      line.innerHTML = `<span>Gold held, characters</span><b>${moneyHtml(held)}</b><span>Best: now</span>`;
      slot.append(line);
    } else {
      slot.append(goal(held, best, { label: "Gold held, characters", format: moneyHtml, note: `Best ${money(best)} on ${shortDay(bestAt * 1000)}`, count: false }));
    }
  }
  const latest = account?.latest;
  const rate = latest ? paceTiles(latest.paces.filter((p) => p.key === "gold_h"), latest.played, { onOpen: openAs(latest.character) }) : null;
  if (rate) slot.append(elText("p", "sc-label", `${displayName(latest.character)}, run of ${shortDay(latest.from)}`), rate);
}

function recordsSheet() {
  const list = account && recordTiles(account.records, { open: (r) => openAs(r.character), owner: (r) => displayName(r.character) });
  if (!list) return null;
  const sheet = el("div", "sheet goals-sheet");
  const section = el("section", "mod w12 first last");
  section.append(elText("h2", "", "Records"), list);
  sheet.append(section);
  return sheet;
}

function transfersMod() {
  if (!data.transfers.length) return "";
  const shown = transfersAll ? data.transfers : data.transfers.slice(0, TRANSFER_ROWS);
  const rows = shown
    .map(
      (t) => `<li><span class="mv-when">${stampHtml(t.ms)} · ${t.via}</span><span class="mv-who">${nameOf(t.from)} → ${nameOf(t.to)}</span>
<span class="mv-amt">${moneyHtml(t.copper)}</span></li>`
    )
    .join("");
  const total = data.transfers_total;
  const more =
    total > TRANSFER_ROWS
      ? `<button class="linkbtn" id="r-moves" type="button">${transfersAll ? `Show the latest ${TRANSFER_ROWS}` : `Show ${data.transfers.length < total ? `the latest ${num(data.transfers.length)} of ${num(total)}` : `all ${num(total)}`}`}</button>`
      : "";
  return mod("w4 last", "Transfers between your characters", `<ul class="moves">${rows}</ul>${more}`);
}


const PAIRS_AS_BARS = 3;

function pairsMod() {
  if (!data.pairs.length) return "";
  const longest = Math.max(...data.pairs.map((p) => p.together), 1);
  const lead = (p) => (p.together ? `${longDur(p.together)} online together` : "No time online together");
  const group = (p) => (p.grouped ? `, ${longDur(p.grouped)} grouped` : "");
  if (data.pairs.length < PAIRS_AS_BARS) {
    const lines = data.pairs
      .map(
        (p) => `<p class="pt-line"><span class="pt-who">${nameOf(p.a)}<small>and</small>${nameOf(p.b)}</span><span><b>${lead(p)}</b>${group(p)} · ${plural(p.sessions, "session", "sessions")}${p.last ? ` · ${stampHtml(p.last * 1000)}` : ""}</span></p>`
      )
      .join("");
    return mod("w12 first last", "Played together", lines);
  }
  const rows = data.pairs
    .map(
      (p) => `<div class="pt-row"><span class="pt-who">${nameOf(p.a)}<small>and</small>${nameOf(p.b)}</span>
        <div class="pt-bar"><span class="pt-t"><b>${lead(p)}</b>${group(p)}</span><i style="--w:${(p.together / longest) * 100}%;--g:${p.together ? Math.min(100, (p.grouped / p.together) * 100) : 0}%"></i></div>
        <span class="pt-n">${plural(p.sessions, "session", "sessions")}</span><span class="pt-last">${p.last ? stampHtml(p.last * 1000) : "–"}</span></div>`
    )
    .join("");
  const key = `<ul class="key"><li><i style="background:var(--text)"></i>Online together</li><li><i style="background:var(--accent)"></i>Grouped</li></ul>`;
  return mod("w12 first last", "Played together", `${key}<div class="pt-list">${rows}</div>`);
}


function dayBuckets() {
  const first = Math.min(...data.chars.flatMap((c) => c.sessions.map((x) => x[0] * 1000)));
  const earliest = new Date();
  earliest.setHours(0, 0, 0, 0);
  earliest.setDate(earliest.getDate() - (DAYS - 1));
  const begin = new Date(Number.isFinite(first) ? Math.max(first, earliest.getTime()) : earliest.getTime());
  begin.setHours(0, 0, 0, 0);
  const span = Math.round((Date.now() - begin.getTime()) / 86_400_000) + 1;
  const days = Array.from({ length: Math.min(DAYS, Math.max(1, span)) }, (_, i) => {
    const d = new Date(begin);
    d.setDate(begin.getDate() + i);
    return { t: d.getTime(), key: dayKey(d.getTime()), per: data.chars.map(() => 0), total: 0 };
  });
  const at = new Map(days.map((d, i) => [d.key, i]));
  data.chars.forEach((c, ci) =>
    c.sessions.forEach(([t, p]) => {
      const i = at.get(dayKey(t * 1000));
      if (i === undefined) return;
      days[i].per[ci] += p;
      days[i].total += p;
    })
  );
  return days;
}

function daysMod() {
  const days = dayBuckets();
  const max = Math.max(...days.map((d) => d.total), 0);
  if (!max) return "";
  const bars = days
    .map((d) => {
      const segs = d.per
        .map((p, ci) => (p ? `<i style="height:${(p / max) * 100}%;background:${hue(ci)}"></i>` : ""))
        .join("");
      return `<div class="xp-bar">${segs}</div>`;
    })
    .join("");
  const step = Math.max(1, Math.ceil(days.length / DAY_TICKS));
  const ticks = days.map((d, i) => `<span>${i % step === 0 ? shortDay(d.t) : ""}</span>`).join("");
  return mod(
    "w12 first last",
    "Play time by day",
    `<div class="xp"><div class="xp-plot">
      <div class="xp-y"><span>${axisTime(max)}</span><span>${axisTime(max / 2)}</span><span>0</span></div>
      <div class="xp-bars" id="r-days" tabindex="0">${bars}</div><div class="xp-x">${ticks}</div></div>
      ${key()}<p class="readout" id="r-days-out"></p></div>`
  );
}

function wireDays(body) {
  const bars = body.querySelector("#r-days");
  if (!bars) return;
  const days = dayBuckets();
  const out = body.querySelector("#r-days-out");
  const rest = () => {
    [...bars.children].forEach((b) => b.classList.remove("on"));
    bars.classList.remove("has-on");
    out.textContent = "";
  };
  const read = (i) => {
    [...bars.children].forEach((b, k) => b.classList.toggle("on", k === i));
    bars.classList.add("has-on");
    const d = days[i];
    const who = d.per
      .map((p, ci) => [p, ci])
      .filter(([p]) => p)
      .sort((a, b) => b[0] - a[0])
      .map(([p, ci]) => `${esc(displayName(data.chars[ci].name))} ${playTime(p)}`);
    out.innerHTML = d.total ? `<b>${weekdayDay(d.t)}</b> · ${playTime(d.total)} · ${who.join(" · ")}` : `<b>${weekdayDay(d.t)}</b> · not played`;
  };
  const at = (e) => {
    const r = bars.getBoundingClientRect();
    return Math.max(0, Math.min(days.length - 1, Math.floor(((e.clientX - r.left) / r.width) * days.length)));
  };
  bars.addEventListener("pointermove", (e) => read(at(e)));
  bars.addEventListener("pointerleave", rest);
}


function levelsMod() {
  const cap = Math.max(...data.chars.map((c) => Math.max(c.cap, c.level)), 0);
  const top = Math.max(...data.chars.flatMap((c) => c.levels.map((l) => l[1])), 0);
  if (cap < 2 || !top) return "";
  const X = (lv) => ((lv - 1) / (cap - 1)) * 1000;
  const Y = (p) => 10 + (1 - p / top) * 200;
  const lines = data.chars
    .map((c, i) => {
      const pts = [...c.levels].sort((a, b) => a[0] - b[0]);
      const path = pts.map(([lv, p], k) => `${k ? "L" : "M"}${X(lv).toFixed(1)} ${Y(p).toFixed(1)}`).join("");
      const dots = pts.map(([lv, p]) => `M${X(lv).toFixed(1)} ${Y(p).toFixed(1)}h0.01`).join("");
      return `<path d="${path}" class="lv-line" style="stroke:${hue(i)}" vector-effect="non-scaling-stroke"/>
        <path d="${dots}" class="lv-dot" style="stroke:${hue(i)}" vector-effect="non-scaling-stroke"/>`;
    })
    .join("");
  const marks = [1, ...Array.from({ length: Math.floor(cap / 10) }, (_, k) => (k + 1) * 10)];
  if (!marks.includes(cap) && cap - marks[marks.length - 1] >= 4) marks.push(cap);
  const ticks = marks.map((lv) => `<span style="left:${((lv - 1) / (cap - 1)) * 100}%">${lv}</span>`).join("");
  return mod(
    "w12 first last",
    "Time per level",
    `<div class="xp"><div class="xp-plot">
      <div class="xp-y"><span>${axisTime(top)}</span><span>${axisTime(top / 2)}</span><span>0</span></div>
      <div class="alts-plot" id="r-levels"><svg viewBox="-10 0 1020 220" preserveAspectRatio="none">${lines}
        <line id="r-lv-x" y1="0" y2="220" class="g-cross" vector-effect="non-scaling-stroke" visibility="hidden"/></svg></div>
      <div class="alts-x">${ticks}</div></div>
      ${key()}<p class="readout" id="r-levels-out"></p></div>`
  );
}

function wireLevels(body) {
  const plot = body.querySelector("#r-levels");
  if (!plot) return;
  const out = body.querySelector("#r-levels-out");
  const cap = Math.max(...data.chars.map((c) => Math.max(c.cap, c.level)), 0);
  const cross = plot.querySelector("#r-lv-x");
  const say = (lv) => {
    const rows = data.chars
      .map((c, i) => ({ i, p: (c.levels.find((l) => l[0] === lv) || [0, 0])[1] }))
      .filter((r) => r.p)
      .sort((a, b) => b.p - a.p)
      .map((r) => `${esc(displayName(data.chars[r.i].name))} ${dur(r.p)}`);
    out.innerHTML = `<b>Level ${lv}</b> · ${rows.length ? rows.join(" · ") : "no time recorded"}`;
  };
  plot.addEventListener("pointermove", (e) => {
    const r = plot.getBoundingClientRect();
    const lv = Math.max(1, Math.min(cap, Math.round(1 + ((e.clientX - r.left) / r.width) * (cap - 1))));
    const x = ((lv - 1) / (cap - 1)) * 1000;
    cross.setAttribute("x1", x);
    cross.setAttribute("x2", x);
    cross.setAttribute("visibility", "visible");
    say(lv);
  });
  plot.addEventListener("pointerleave", () => {
    cross.setAttribute("visibility", "hidden");
    out.innerHTML = "";
  });
}


function professionsMod() {
  const counts = new Map();
  for (const c of data.chars) for (const s of c.professions) counts.set(s.name, (counts.get(s.name) || 0) + 1);
  const names = [...counts.keys()].sort((a, b) => counts.get(b) - counts.get(a) || a.localeCompare(b));
  if (!names.length) return "";
  const cards = names
    .map((n) => {
      const skills = data.chars
        .map((c) => [c, c.professions.find((p) => p.name === n)])
        .filter(([, p]) => p)
        .sort((a, b) => b[1].rank / (b[1].max || 1) - a[1].rank / (a[1].max || 1));
      const rows = skills
        .map(([c, p]) => {
          const full = p.max > 0 && p.rank >= p.max;
          return `<div class="pf-row${full ? " full" : ""}"><span>${named(c)}</span><b>${num(p.rank)}<small>/${num(p.max)}</small></b><i style="--w:${Math.min(100, (p.rank / (p.max || 1)) * 100)}%"></i></div>`;
        })
        .join("");
      return `<div class="pf-card"><h3>${esc(n)}</h3>${rows}</div>`;
    })
    .join("");
  return mod("w12 first last", "Professions", `<div class="pf-grid">${cards}</div>`);
}


function draw(body) {
  const top = body.scrollTop;
  body.innerHTML = "";
  if (!data || !data.chars.length) {
    body.innerHTML = `<section class="panel"><h3>Characters</h3>${empty("No characters recorded yet")}</section>`;
    return;
  }
  const records = recordsSheet();
  if (records) body.append(records);
  body.insertAdjacentHTML(
    "beforeend",
    `${totals()}<div class="sheet">${data.group_by_account ? data.groups.map(groupMod).join("") : charactersMod()}${[goldMod(), transfersMod(), pairsMod(), daysMod(), levelsMod(), professionsMod()].join("")}</div>`
  );
  fillCaps(body);
  fillGoldGoals(body);
  countMarked(body, { money: moneyHtml, num });
  wireLineCharts(body);
  wireDays(body);
  wireLevels(body);
  body.querySelectorAll("[data-pick]").forEach((row) => {
    const go = () => pick(row.dataset.pick);
    row.onclick = go;
    row.onkeydown = (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        go();
      }
    };
  });
  const more = body.querySelector("#r-moves");
  if (more) {
    more.onclick = () => {
      transfersAll = !transfersAll;
      draw(body);
    };
  }
  body.scrollTop = top;
  if (entering) {
    entering = false;
    play(body);
  }
}

export async function render(body) {
  wireTips();
  [data, account] = await Promise.all([invoke("roster"), invoke("account_goals").catch(() => null)]);
  entering = true;
  draw(body);
}
