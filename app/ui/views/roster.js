// Characters: every recorded character at once, shown when no character is selected.
// The model comes from the `roster` command. Section order: totals, characters by game
// account, gold and transfers, played together, play time by day, time per level, professions.

import { invoke, esc, num, dur, moneyHtml, dateStr } from "../lib.js";
import { CLASS_COLOR, className, raceName, day, hm, longDur, playTime, signed, empty, mod, mondayOf, axisTime } from "./overview.js";

const DAYS = 30;
const TRANSFER_ROWS = 8;
const TONES = 8;

let data = null;
let clients = 0;
let transfersAll = false;

const tone = (i) => `var(--alt-${(i % TONES) + 1})`;
const sum = (rows, f) => rows.reduce((a, r) => a + f(r), 0);
const named = (c) => `<span class="who-c"><i class="dot-c" style="background:${CLASS_COLOR[c.class] || "var(--muted)"}"></i>${esc(c.name)}</span>`;
const nameOf = (name) => {
  const c = data.chars.find((x) => x.name === name);
  return c ? named(c) : esc(name);
};
const key = () =>
  `<ul class="key">${data.chars.map((c, i) => `<li><i style="background:${tone(i)}"></i>${esc(c.name)}</li>`).join("")}</ul>`;

function pick(name) {
  const sel = document.querySelector("#char");
  sel.value = name;
  sel.dispatchEvent(new Event("change"));
}


function totals() {
  const monday = mondayOf(new Date()).getTime();
  const week = sum(data.chars, (c) => sum(c.sessions.filter((s) => s[0] * 1000 >= monday), (s) => s[1]));
  const cells = [
    ["Characters", num(data.chars.length)],
    ["Played", longDur(sum(data.chars, (c) => c.played))],
    ["Gold", moneyHtml(sum(data.chars, (c) => c.money || 0))],
    ["This week", playTime(week)],
    ["WoW running", num(clients)],
  ];
  return `<section class="week"><div><h2>Totals</h2></div>${cells.map(([l, v]) => `<div><dt>${l}</dt><dd>${v}</dd></div>`).join("")}</section>`;
}


function levelCell(c) {
  const atCap = c.cap > 0 && c.level >= c.cap;
  const pct = atCap ? 100 : c.xp_max > 0 ? Math.min(100, Math.floor((c.xp / c.xp_max) * 100)) : 0;
  return `<span class="lvl-cell">${c.level || "–"}<span class="mini"><i style="width:${pct}%"></i></span></span>`;
}

function reachCell(c) {
  if (c.cap > 0 && c.level >= c.cap) return "At level cap";
  return c.reach && c.reach.date ? `Level ${c.reach.level} ${esc(c.reach.date)}` : "–";
}

function groupMod(group) {
  const byName = new Map(data.chars.map((c) => [c.name, c]));
  const rows = group.characters
    .map((n) => byName.get(n))
    .filter(Boolean)
    .sort((a, b) => b.through - a.through)
    .map(
      (c) => `<tr class="click" tabindex="0" data-pick="${esc(c.name)}">
        <td>${named(c)}</td><td>${levelCell(c)}</td><td>${esc(raceName(c.race))} ${esc(className(c.class))}</td>
        <td>${esc(c.zone || "–")}</td><td>${c.played ? longDur(c.played) : "–"}</td>
        <td>${c.through ? `${day(c.through * 1000)} ${hm(c.through * 1000)}` : "–"}</td>
        <td class="num">${c.money == null ? "–" : moneyHtml(c.money)}</td><td class="gap">${reachCell(c)}</td></tr>`
    )
    .join("");
  const head = ["Character", "Level", "Race and class", "Zone", "Played", "Last session", "Gold", "Level target"];
  const cols = head.map((h, i) => `<th${i === 6 ? ' class="num"' : i === 7 ? ' class="gap"' : ""}>${h}</th>`).join("");
  const title = group.account ? `Game account ${group.account}` : "Other characters";
  return mod("w12 first last", title, `<div class="history"><table class="alts"><thead><tr>${cols}</tr></thead><tbody>${rows}</tbody></table></div>`);
}


function combined() {
  const points = data.chars.flatMap((c, i) => c.balance.map(([t, m]) => ({ t, m, i }))).sort((a, b) => a.t - b.t);
  const last = new Map();
  return points.map((p) => {
    last.set(p.i, p.m);
    return { t: p.t, m: [...last.values()].reduce((a, v) => a + v, 0) };
  });
}

function goldMod() {
  const total = sum(data.chars, (c) => c.money || 0);
  const pts = combined();
  const plot =
    pts.length > 1
      ? `<div class="gold-plot"><svg id="r-gold" viewBox="0 0 300 100" preserveAspectRatio="none"></svg>
        <div class="gold-axis"><span>${day(pts[0].t * 1000)}</span><span>${day(pts[pts.length - 1].t * 1000)}</span></div></div>
        <p class="readout" id="r-gold-out"></p>`
      : "";
  const rows = data.chars
    .map((c) => {
      const net = sum(c.sessions, (s) => s[2] - s[3]);
      const flow = (v) => (v ? moneyHtml(v) : "–");
      return `<tr><td>${named(c)}</td><td class="num">${c.money == null ? "–" : moneyHtml(c.money)}</td><td class="num">${signed(net)}</td><td class="num">${flow(c.sent)}</td><td class="num">${flow(c.received)}</td></tr>`;
    })
    .join("");
  const table = `<div class="history"><table class="alts"><thead><tr><th>Character</th><th class="num">Balance</th><th class="num">Net without transfers</th><th class="num">Sent</th><th class="num">Received</th></tr></thead><tbody>${rows}</tbody></table></div>`;
  return mod("w8 first", "Gold", `<p class="big">${moneyHtml(total)}</p>${plot}${table}`);
}

function wireGold(body) {
  const svg = body.querySelector("#r-gold");
  if (!svg) return;
  const out = body.querySelector("#r-gold-out");
  const pts = combined();
  const lo = Math.min(...pts.map((p) => p.m));
  const hi = Math.max(...pts.map((p) => p.m));
  const gx = (i) => (i / (pts.length - 1)) * 300;
  const gy = (m) => 94 - ((m - lo) / (hi - lo || 1)) * 86;
  const line = pts.map((p, i) => `${i ? "L" : "M"}${gx(i).toFixed(1)} ${gy(p.m).toFixed(1)}`).join("");
  svg.innerHTML = `<path d="${line}L300 100L0 100Z" class="g-area"/>
    <path d="${line}" class="g-line" vector-effect="non-scaling-stroke"/>
    <line id="r-gold-x" y1="0" y2="100" class="g-cross" vector-effect="non-scaling-stroke" visibility="hidden"/>`;
  const rest = () => {
    out.innerHTML = `Between ${moneyHtml(lo)} and ${moneyHtml(hi)} across all characters`;
    svg.querySelector("#r-gold-x").setAttribute("visibility", "hidden");
  };
  svg.addEventListener("pointermove", (e) => {
    const r = svg.getBoundingClientRect();
    const i = Math.max(0, Math.min(pts.length - 1, Math.round(((e.clientX - r.left) / r.width) * (pts.length - 1))));
    const cross = svg.querySelector("#r-gold-x");
    cross.setAttribute("x1", gx(i));
    cross.setAttribute("x2", gx(i));
    cross.setAttribute("visibility", "visible");
    out.innerHTML = `<b>${day(pts[i].t * 1000)} ${hm(pts[i].t * 1000)}</b> · ${moneyHtml(pts[i].m)}`;
  });
  svg.addEventListener("pointerleave", rest);
  rest();
}

function transfersMod() {
  if (!data.transfers.length) return mod("w4 last", "Transfers between your characters", empty("No money moved between your characters"));
  const shown = transfersAll ? data.transfers : data.transfers.slice(0, TRANSFER_ROWS);
  const rows = shown
    .map(
      (t) => `<li><span class="mv-when">${day(t.ms)} ${hm(t.ms)} · ${t.via}</span><span class="mv-who">${nameOf(t.from)} → ${nameOf(t.to)}</span>
<span class="mv-amt">${moneyHtml(t.copper)}</span></li>`
    )
    .join("");
  const total = data.transfers_total;
  const more =
    total > TRANSFER_ROWS
      ? `<button class="linkbtn" id="r-moves" type="button">${transfersAll ? `Show the latest ${TRANSFER_ROWS}` : `Show ${data.transfers.length < total ? `the newest ${num(data.transfers.length)} of ${num(total)}` : `all ${num(total)}`}`}</button>`
      : "";
  return mod("w4 last", "Transfers between your characters", `<ul class="moves">${rows}</ul>${more}`);
}


function pairsMod() {
  if (!data.pairs.length) return mod("w12 first last", "Played together", empty("No two of your characters were online or grouped together"));
  const rows = data.pairs
    .map(
      (p) => `<tr><td>${nameOf(p.a)} and ${nameOf(p.b)}</td><td class="num">${p.together ? longDur(p.together) : "–"}</td>
        <td class="num">${p.grouped ? longDur(p.grouped) : "–"}</td><td class="num">${num(p.sessions)}</td>
        <td class="num">${p.last ? `${day(p.last * 1000)} ${hm(p.last * 1000)}` : "–"}</td></tr>`
    )
    .join("");
  return mod(
    "w12 first last",
    "Played together",
    `<div class="history"><table class="alts"><thead><tr><th>Characters</th><th class="num">Online together</th><th class="num">Grouped</th><th class="num">Sessions</th><th class="num">Last time</th></tr></thead><tbody>${rows}</tbody></table></div>`
  );
}


function dayBuckets() {
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  start.setDate(start.getDate() - (DAYS - 1));
  const days = Array.from({ length: DAYS }, (_, i) => {
    const d = new Date(start);
    d.setDate(start.getDate() + i);
    return { t: d.getTime(), key: dateStr(d.getTime()), per: data.chars.map(() => 0), total: 0 };
  });
  const at = new Map(days.map((d, i) => [d.key, i]));
  data.chars.forEach((c, ci) =>
    c.sessions.forEach(([t, p]) => {
      const i = at.get(dateStr(t * 1000));
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
  if (!max) return mod("w12 first last", "Play time by day", empty(`No sessions in the last ${DAYS} days`));
  const bars = days
    .map((d) => {
      const segs = d.per
        .map((p, ci) => (p ? `<i style="height:${(p / max) * 100}%;background:${tone(ci)}"></i>` : ""))
        .join("");
      return `<div class="xp-bar">${segs}</div>`;
    })
    .join("");
  const ticks = days
    .map((d, i) => `<span>${(DAYS - 1 - i) % 5 === 0 && i < DAYS - 1 ? `${new Date(d.t).getDate()} ${new Date(d.t).toLocaleDateString([], { month: "short" })}` : ""}</span>`)
    .join("");
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
  const read = (i, live) => {
    [...bars.children].forEach((b, k) => b.classList.toggle("on", k === i));
    bars.classList.toggle("has-on", !!live);
    const d = days[i];
    const who = d.per
      .map((p, ci) => [p, ci])
      .filter(([p]) => p)
      .sort((a, b) => b[0] - a[0])
      .map(([p, ci]) => `${esc(data.chars[ci].name)} ${playTime(p)}`);
    out.innerHTML = d.total ? `<b>${day(d.t)}</b> · ${playTime(d.total)} · ${who.join(" · ")}` : `<b>${day(d.t)}</b> · not played`;
  };
  const at = (e) => {
    const r = bars.getBoundingClientRect();
    return Math.max(0, Math.min(days.length - 1, Math.floor(((e.clientX - r.left) / r.width) * days.length)));
  };
  bars.addEventListener("pointermove", (e) => read(at(e), true));
  bars.addEventListener("pointerleave", () => read(days.length - 1));
  read(days.length - 1);
}


function levelsMod() {
  const cap = Math.max(...data.chars.map((c) => Math.max(c.cap, c.level)), 0);
  const top = Math.max(...data.chars.flatMap((c) => c.levels.map((l) => l[1])), 0);
  if (cap < 2 || !top) return mod("w12 first last", "Time per level", empty("No time per level recorded"));
  const X = (lv) => ((lv - 1) / (cap - 1)) * 1000;
  const Y = (p) => 10 + (1 - p / top) * 200;
  const lines = data.chars
    .map((c, i) => {
      const pts = [...c.levels].sort((a, b) => a[0] - b[0]);
      const path = pts.map(([lv, p], k) => `${k ? "L" : "M"}${X(lv).toFixed(1)} ${Y(p).toFixed(1)}`).join("");
      const dots = pts.map(([lv, p]) => `M${X(lv).toFixed(1)} ${Y(p).toFixed(1)}h0.01`).join("");
      return `<path d="${path}" class="lv-line" style="stroke:${tone(i)}" vector-effect="non-scaling-stroke"/>
        <path d="${dots}" class="lv-dot" style="stroke:${tone(i)}" vector-effect="non-scaling-stroke"/>`;
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
      .map((r) => `${esc(data.chars[r.i].name)} ${dur(r.p)}`);
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
  if (!names.length) return mod("w12 first last", "Professions", empty("No professions recorded"));
  const rows = data.chars
    .map((c) => {
      const cells = names
        .map((n) => {
          const s = c.professions.find((p) => p.name === n);
          if (!s) return `<td class="dim">–</td>`;
          return `<td><span class="skill">${num(s.rank)}<span class="dim">/${num(s.max)}</span><span class="mini"><i style="width:${Math.min(100, (s.rank / (s.max || 1)) * 100)}%"></i></span></span></td>`;
        })
        .join("");
      return `<tr><td>${named(c)}</td>${cells}</tr>`;
    })
    .join("");
  const cols = names.map((n) => `<th>${esc(n)}</th>`).join("");
  return mod("w12 first last", "Professions", `<div class="history"><table class="alts"><thead><tr><th>Character</th>${cols}</tr></thead><tbody>${rows}</tbody></table></div>`);
}


function draw(body) {
  const top = body.scrollTop;
  body.innerHTML = "";
  if (!data || !data.chars.length) {
    body.innerHTML = `<section class="panel"><h3>Characters</h3>${empty("No characters recorded yet")}</section>`;
    return;
  }
  body.insertAdjacentHTML(
    "beforeend",
    `${totals()}<div class="sheet">${data.groups.map(groupMod).join("")}${goldMod()}${transfersMod()}${pairsMod()}${daysMod()}${levelsMod()}${professionsMod()}</div>`
  );
  wireGold(body);
  wireDays(body);
  wireLevels(body);
  body.querySelectorAll("tr[data-pick]").forEach((row) => {
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
}

export async function render(body) {
  [data, clients] = await Promise.all([invoke("roster"), invoke("wow_clients").catch(() => 0)]);
  draw(body);
}
