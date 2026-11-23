// Sessions: S..ES runs with duration, zones, kills, xp, money and casts.
// Click a row to open its range in the timeline; a run summary opens for the
// session's route, and covers every character played in the run.

import { invoke, el, state, num, dur, moneyHtml, dateTime, esc, displayName } from "../lib.js";
import { renderRecaps } from "./recap.js";

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
  if (!run.recaps.length) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", "Nothing was recorded while WoW ran."));
    return;
  }
  await renderRecaps(body, run);
}
