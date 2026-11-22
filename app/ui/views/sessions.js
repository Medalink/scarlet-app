// Sessions: S..ES runs with duration, zones, kills, xp, money and casts.
// Click a row to open its range in the timeline.

import { invoke, el, state, num, dur, moneyHtml, dateTime, esc, displayName } from "../lib.js";

export async function render(body) {
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
    <th class="num">Kills</th><th class="num">XP</th><th class="num">Gold</th><th class="num">Casts</th>
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
