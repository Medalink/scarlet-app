// Activity: events per weekday x hour, or per calendar day.

import { invoke, el, elText, num, dur, state, tiles, fillRow, record, pace, goal, shortDay, weekdayDay, dayRange, keyMs } from "../lib.js";
import { play } from "../motion.js";
import { mod } from "./goals-section.js";
import { openAs } from "./paces.js";

const DAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

function shade(count, max) {
  if (!max || !count) return "";
  const t = count / max;
  const step = t < 0.25 ? "#4b1626" : t < 0.5 ? "#861a34" : t < 0.75 ? "#cf1f45" : "#ff5d7e";
  return `background:${step}`;
}

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
  const spread = w.spread;
  const best = w.best ? { value: w.best.seconds, from: w.best.from } : null;
  return pace(w.seconds, spread, { label: "Played this week through yesterday", unit: "", format: timeHtml, text: dur, best: w.is_best, peak: best, peers: w.peers, noun: "weeks" });
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

export async function render(body) {
  const [data, played] = await Promise.all([invoke("heatmap", { ch: state.ch }), invoke("playtime", { ch: state.ch })]);
  body.innerHTML = "";
  const sheet = playSheet(played);
  if (sheet) body.appendChild(sheet);
  let mode = sessionStorage.getItem("hm-mode") || "week";

  const bar = el("div", "row");
  bar.style.marginTop = "18px";
  const bw = el("button", "btn" + (mode === "week" ? " on" : ""), "Weekday × hour");
  bw.type = "button";
  bw.setAttribute("aria-pressed", String(mode === "week"));
  const bd = el("button", "btn" + (mode === "day" ? " on" : ""), "Calendar day");
  bd.type = "button";
  bd.setAttribute("aria-pressed", String(mode === "day"));
  bar.append(bw, bd);
  body.appendChild(bar);

  const holder = el("div");
  holder.style.marginTop = "14px";
  body.appendChild(holder);

  function draw() {
    holder.innerHTML = "";
    if (mode === "week") {
      const g = el("div", "hm-grid");
      g.appendChild(el("div", "hm-lab"));
      for (let h = 0; h < 24; h += 1) {
        g.appendChild(el("div", "hm-toplab", h % 4 === 0 ? String(h).padStart(2, "0") : ""));
      }
      for (let d = 0; d < 7; d++) {
        g.appendChild(el("div", "hm-lab", DAYS[d]));
        for (let h = 0; h < 24; h++) {
          const c = data.week[d * 24 + h] || 0;
          const cell = g.appendChild(el("div", "hm-cell", ""));
          cell.style.cssText = shade(c, data.max_week);
          cell.title = `${DAYS[d]} ${String(h).padStart(2, "0")}:00 · ${num(c)} ${c === 1 ? "event" : "events"}`;
        }
      }
      holder.appendChild(g);
      holder.insertAdjacentHTML(
        "beforeend",
        `<div class="note">Brighter means more events. Peak: ${num(data.max_week)} ${data.max_week === 1 ? "event" : "events"} in an hour.</div>`
      );
    } else {
      if (!data.days.length) {
        holder.innerHTML = `<div class="empty">No events yet.</div>`;
        return;
      }
      for (const [day, count] of data.days) {
        const r = el("div", "day-row");
        r.innerHTML = `<div>${day}</div>
          <div class="day-track"><div class="day-fill" style="width:${(count / (data.max_day || 1)) * 100}%"></div></div>
          <div class="bar-value">${num(count)}</div>`;
        holder.appendChild(r);
      }
    }
  }

  bw.onclick = () => { mode = "week"; sessionStorage.setItem("hm-mode", mode); bw.classList.add("on"); bd.classList.remove("on"); bw.setAttribute("aria-pressed", "true"); bd.setAttribute("aria-pressed", "false"); draw(); };
  bd.onclick = () => { mode = "day"; sessionStorage.setItem("hm-mode", mode); bd.classList.add("on"); bw.classList.remove("on"); bd.setAttribute("aria-pressed", "true"); bw.setAttribute("aria-pressed", "false"); draw(); };
  draw();
  play(body);
}
