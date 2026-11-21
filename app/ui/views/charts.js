// Charts: cumulative xp and money, top spells, buff uptime, zone time,
// items in and out, people grouped with.

import { invoke, el, lineChart, barList, dur, state, esc } from "../lib.js";

function panel(title, inner) {
  const p = el("div", "panel chart-panel");
  p.innerHTML = `<h3>${title}</h3>`;
  if (typeof inner === "string") p.insertAdjacentHTML("beforeend", inner);
  else p.appendChild(inner);
  return p;
}

export async function render(body) {
  body.innerHTML = "";
  const c = await invoke("charts", { ch: state.ch });

  body.appendChild(panel("Experience over time", lineChart(c.xp, "#ff2d55")));
  body.appendChild(panel("Money over time (cumulative)", lineChart(c.money, "#d9b25f")));

  const grid = el("div", "two-col");
  grid.appendChild(panel("Top spells cast", barList(c.spells, "#ff2d55")));
  grid.appendChild(panel("Top buffs by uptime", barList(c.buffs, "#cdced4", dur)));
  grid.appendChild(panel("Zones by time", barList(c.zones, "#ff2d55", dur)));
  grid.appendChild(panel("People grouped with", barList(c.people, "#cdced4", dur)));
  body.appendChild(grid);

  const fight = el("div", "two-col");
  fight.appendChild(panel("Damage by spell", barList(c.damage_spells, "#ff2d55")));
  fight.appendChild(panel("Healing by spell", barList(c.healing_spells, "#cdced4")));
  fight.appendChild(panel("Killed most", barList(c.kills, "#ff2d55")));
  fight.appendChild(panel("What killed you", barList(c.killers, "#cdced4")));
  body.appendChild(fight);

  const life = el("div", "two-col");
  life.appendChild(panel("Time casting, by spell", barList(c.cast_time, "#ff2d55", dur)));
  life.appendChild(panel("Emotes used", barList(c.emotes, "#cdced4")));
  body.appendChild(life);

  const two = el("div", "two-col");
  two.appendChild(panel("Items in", barList(c.items_in, "#ff2d55")));
  two.appendChild(panel("Items out", barList(c.items_out, "#cdced4")));
  body.appendChild(two);

  const tr = await invoke("trends", { ch: state.ch });

  // One line chart redrawn from a picker of series.
  const picker = (title, list, label, color) => {
    if (!list.length) return;
    const holder = el("div");
    const sel = el("select");
    sel.innerHTML = list
      .map((s) => `<option value="${esc(s.key)}">${esc(label(s))}</option>`)
      .join("");
    const chart = el("div");
    const draw = () => {
      const s = list.find((x) => x.key === sel.value);
      chart.innerHTML = s ? lineChart(s.series, color) : "";
    };
    sel.addEventListener("change", draw);
    draw();
    holder.appendChild(sel);
    holder.appendChild(chart);
    body.appendChild(panel(title, holder));
  };

  picker("Character stats over time", tr.stats, (s) => s.key, "#ff2d55");
  picker("Statistics over time", tr.statistics, (s) => s.name, "#ff2d55");
}
