// Sessions: S..ES runs with duration, zones, kills, xp, money and casts.
// Click a row to open its range in the timeline.

import { invoke, el, state, dur, moneyHtml, dateTime, esc } from "../lib.js";

export async function render(body) {
  body.innerHTML = "";
  const rows = await invoke("sessions", { ch: state.ch });
  if (!rows.length) {
    body.appendChild(el("div", "empty", "No sessions yet."));
    return;
  }

  const t = el("table");
  t.innerHTML = `<thead><tr>
    <th>Start</th><th>Character</th><th>Duration</th><th>Zones</th>
    <th class="num">Kills</th><th class="num">XP</th><th class="num">Money</th><th class="num">Casts</th>
  </tr></thead><tbody></tbody>`;
  const tb = t.querySelector("tbody");
  for (const s of rows.slice().reverse()) {
    const tr = el("tr", "click");
    tr.innerHTML = `<td>${dateTime(s.start)}</td>
      <td class="dim">${esc(s.char_name)}</td>
      <td>${dur(s.duration)}</td>
      <td class="zones">${s.zones.map(esc).join(" · ") || "—"}</td>
      <td class="num">${s.kills}</td>
      <td class="num">${s.xp}</td>
      <td class="num ${s.money >= 0 ? "pos" : "neg"}">${moneyHtml(s.money, s.money !== 0)}</td>
      <td class="num">${s.casts}</td>`;
    tr.onclick = () => {
      window.__tlSession = { from: s.start - 1000, to: s.end + 1000 };
      window.__nav("timeline");
    };
    tb.appendChild(tr);
  }
  body.appendChild(t);
}
