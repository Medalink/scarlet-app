// Sessions: S..ES runs with duration, zones, kills, xp, money and casts.
// Click a row to open its range in the timeline; a run summary opens for the
// session's route, and covers every character played in the run.

import { invoke, el, state, num, dur, moneyHtml, dateTime, esc, displayName, clock } from "../lib.js";
import { renderStats } from "./stats.js";

export async function render(body, sub) {
  if (/^\d+-\d+$/.test(sub ?? "")) return renderRun(body, sub);
  const rows = await invoke("sessions", { ch: state.ch });
  body.innerHTML = "";
  if (!rows.length) {
    body.appendChild(el("div", "empty", "No sessions yet."));
    return;
  }

  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr>
    <th>Start</th><th>Character</th><th>Played</th><th>Zones</th>
    <th class="num">Kills</th><th class="num">XP</th><th class="num">Gold</th><th class="num">Casts</th><th></th>
  </tr></thead><tbody></tbody>`;
  const tb = t.querySelector("tbody");
  for (const s of rows.slice().reverse()) {
    const tr = el("tr", "click");
    tr.tabIndex = 0;
    tr.innerHTML = `<td>${dateTime(s.start)}</td>
      <td class="dim">${esc(displayName(s.char_name))}</td>
      <td>${dur(s.duration)}</td>
      <td class="zones">${s.zones.map(esc).join(" · ") || "—"}</td>
      <td class="num">${num(s.kills)}</td>
      <td class="num">${num(s.xp)}</td>
      <td class="num ${s.money >= 0 ? "pos" : "neg"}">${moneyHtml(s.money, s.money !== 0)}</td>
      <td class="num">${num(s.casts)}</td>`;
    const summary = el("button", "linkbtn", "Summary");
    summary.type = "button";
    summary.onclick = (e) => {
      e.stopPropagation();
      window.__nav(`sessions/${s.start}-${s.end}`);
    };
    summary.onkeydown = (e) => e.stopPropagation();
    const cell = el("td");
    cell.appendChild(summary);
    tr.appendChild(cell);
    const openSession = () => {
      window.__tlSession = { index: s.index, from: s.start - 1000, to: s.end + 1000 };
      window.__nav("timeline");
    };
    tr.onclick = openSession;
    tr.onkeydown = (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        openSession();
      }
    };
    tb.appendChild(tr);
  }
  wrap.appendChild(t);
  body.appendChild(wrap);
}

async function renderRun(body, sub) {
  const [from, to] = sub.split("-").map(Number);
  const run = await invoke("run_summary", { from, to });
  body.innerHTML = "";
  document.querySelector("#pagetitle").textContent = "WoW run";
  if (!run.sessions.length) {
    body.appendChild(el("div", "empty", "Nothing was recorded while WoW ran."));
    return;
  }

  const head = el("p", "run-head");
  const sameDay = new Date(run.from).toDateString() === new Date(run.to).toDateString();
  const end = sameDay ? clock(run.to).slice(0, 5) : dateTime(run.to);
  head.innerHTML = `${dateTime(run.from)} – ${end} · ${dur(run.played)} played · ${run.chars
    .map((c) => esc(displayName(c)))
    .join(", ")}`;
  body.appendChild(head);

  if (run.highlights.length) {
    body.appendChild(el("h3", "section-title", "Levels, quests, achievements and deaths"));
    const list = el("div", "run-highlights");
    list.innerHTML = run.highlights
      .map(
        (h) => `<div class="run-row">
          <span class="tl-time">${clock(h.ms)}</span>
          <span class="run-char">${esc(displayName(h.char_name))}</span>
          <span class="tl-word">${esc(h.word)}</span>
          <span class="tl-text">${esc(h.text)}</span>
        </div>`
      )
      .join("");
    body.appendChild(list);
  }

  const zones = [];
  for (const s of run.sessions) {
    for (const z of s.zones) {
      if (!zones.includes(z)) zones.push(z);
    }
  }
  body.appendChild(el("h3", "section-title", "Zones visited"));
  body.appendChild(el("div", "run-zones", zones.map(esc).join(" · ") || "—"));

  renderStats(body, run.stats);
}
