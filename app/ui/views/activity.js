// Activity: time played per weekday and hour, or per calendar day.

import { invoke, el, elText, num, dur, state, tile, tiles, fillRow, record, pace, goal, shortDay, weekdayDay, dayRange, keyMs, clockTime } from "../lib.js";
import { play } from "../motion.js";
import { mod } from "./goals-section.js";
import { openAs } from "./paces.js";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

const STEPS = ["rgba(255,255,255,.055)", "#4b1626", "#861a34", "#cf1f45", "#ff5d7e"];

function level(secs, max) {
  if (!max || !secs) return 0;
  const t = secs / max;
  return t < 0.25 ? 1 : t < 0.5 ? 2 : t < 0.75 ? 3 : 4;
}

const hourOf = (h) => clockTime(new Date(2000, 0, 1, h).getTime());
const tickOf = (h) => hourOf(h).replace(":00 ", " ");

const timeHtml = (secs) => dur(secs);
const dayCount = (n) => `${num(n)} ${n === 1 ? "day" : "days"}`;
const dayDate = (day) => shortDay(keyMs(day));
const spanHash = (r) => `#sessions/${r.from}-${r.to}-days`;

function recordTiles(p) {
  const list = [];
  const add = (label, value, format, when, span) => list.push(record(label, format(value), when, spanHash(span), { count: [value, format], onOpen: state.ch ? openAs(state.ch) : null }));
  if (p.longest_day) add("Longest day", p.longest_day.seconds, timeHtml, weekdayDay(p.longest_day.from), p.longest_day);
  if (p.longest_week) add("Most played week", p.longest_week.seconds, timeHtml, `Week of ${dayDate(p.longest_week.day)}`, p.longest_week);
  if (p.best_streak) add("Longest streak", p.best_streak.days, dayCount, dayRange(keyMs(p.best_streak.from_day), keyMs(p.best_streak.to_day)), p.best_streak);
  const cols = Math.min(3, list.length);
  return list.length ? tiles(fillRow(list, cols), cols) : null;
}

function weekPace(w) {
  if (!w.spread) {
    const plain = tile("Played this week through yesterday", dur(w.seconds), "", { size: "xl" });
    plain.classList.add("sc-pace");
    return plain;
  }
  const best = w.best ? { value: w.best.seconds, from: w.best.from } : null;
  return pace(w.seconds, w.spread, { label: "Played this week through yesterday", unit: "", format: timeHtml, text: dur, best: w.is_best, peak: best, peers: w.peers, noun: "weeks" });
}

function streakGoal(p) {
  const best = p.best_streak?.days ?? 0;
  const now = p.streak?.days ?? 0;
  return goal(now, Math.max(best, now, 1), { label: "Current streak", format: dayCount, doneText: "Best streak", eta: best ? `Best ${dayCount(best)}` : "" });
}

function playSheet(p) {
  if (!p.longest_day) return null;
  const sheet = el("div", "sheet goals-sheet");
  const records = recordTiles(p);
  const now = el("div", "goal-stack");
  if (p.week) now.append(tiles([weekPace(p.week)], 1));
  now.append(streakGoal(p));
  sheet.append(mod("w7 first", "Records", records), mod("w5 last", "This week", now));
  return sheet;
}

function hourGrid(data) {
  const g = el("div", "hm-grid");
  g.appendChild(el("div", "hm-lab"));
  for (let h = 0; h < 24; h += 3) g.appendChild(elText("div", "hm-toplab", tickOf(h)));
  for (let d = 0; d < 7; d++) {
    g.appendChild(elText("div", "hm-lab", DAYS[d]));
    for (let h = 0; h < 24; h++) {
      const secs = data.week[d * 24 + h] || 0;
      const cell = g.appendChild(el("div", "hm-cell"));
      cell.style.background = STEPS[level(secs, data.max_week)];
      cell.title = `${DAYS[d]} ${hourOf(h)} · ${secs ? `${dur(secs)} played` : "nothing played"}`;
    }
  }
  return g;
}

function legend() {
  const key = el("p", "heat-key");
  const swatches = STEPS.map((c) => {
    const i = el("i");
    i.style.background = c;
    return i;
  });
  key.append(elText("span", "", "Quiet"), ...swatches, elText("span", "", "Busiest"));
  return key;
}

function busiest(data) {
  const at = data.week.indexOf(data.max_week);
  return elText("p", "note hm-busiest", `Busiest hour: ${DAYS[Math.floor(at / 24)]} ${hourOf(at % 24)}`);
}

function dayRows(data) {
  const list = el("div", "hm-days");
  for (const [day, secs] of [...data.days].reverse()) {
    const r = el("div", "day-row");
    r.append(elText("div", "", weekdayDay(keyMs(day))));
    const track = el("div", "day-track");
    const fill = el("div", "day-fill");
    fill.style.width = `${(secs / (data.max_day || 1)) * 100}%`;
    track.append(fill);
    r.append(track, elText("div", "bar-value", dur(secs)));
    list.append(r);
  }
  return list;
}

export async function render(body) {
  const [data, played] = await Promise.all([invoke("heatmap", { ch: state.ch }), invoke("playtime", { ch: state.ch })]);
  body.innerHTML = "";
  if (!data.max_week) {
    body.appendChild(el("div", "empty", "No play time recorded yet"));
    return;
  }
  const sheet = playSheet(played);
  if (sheet) body.appendChild(sheet);
  let mode = sessionStorage.getItem("hm-mode") === "day" ? "day" : "week";

  const bar = el("div", "row");
  bar.style.marginTop = "18px";
  const bw = el("button", "btn", "By hour");
  const bd = el("button", "btn", "By day");
  bw.type = bd.type = "button";
  bar.append(bw, bd);
  body.appendChild(bar);

  const holder = el("div");
  holder.style.marginTop = "14px";
  body.appendChild(holder);

  function draw() {
    for (const [btn, key] of [[bw, "week"], [bd, "day"]]) {
      btn.classList.toggle("on", mode === key);
      btn.setAttribute("aria-pressed", String(mode === key));
    }
    holder.innerHTML = "";
    if (mode === "week") holder.append(hourGrid(data), legend(), busiest(data));
    else holder.append(dayRows(data));
  }

  for (const [btn, key] of [[bw, "week"], [bd, "day"]]) {
    btn.onclick = () => {
      mode = key;
      sessionStorage.setItem("hm-mode", key);
      draw();
    };
  }
  draw();
  play(body);
}
