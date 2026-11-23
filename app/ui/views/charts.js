// Charts: cumulative xp and money, top spells, buff uptime, zone time,
// items in and out, people grouped with.

import { invoke, el, lineChart, wireLineCharts, barList, dur, money, state } from "../lib.js";
import { panel, pair, picker, statLabel } from "./chartkit.js";

export async function render(body) {
  const [c, tr] = await Promise.all([
    invoke("charts", { ch: state.ch }),
    invoke("trends", { ch: state.ch }),
  ]);
  body.innerHTML = "";

  body.appendChild(panel("Experience over time", lineChart(c.xp, "#ff2d55")));
  body.appendChild(panel("Gold over time", lineChart(c.money, "#d9b25f", 150, money)));

  body.appendChild(
    pair(
      panel("Top spells cast", barList(c.spells, "#ff2d55")),
      panel("Top buffs by uptime", barList(c.buffs, "#cdced4", dur)),
      panel("Zones by time", barList(c.zones, "#ff2d55", dur)),
      panel("People grouped with", barList(c.people, "#cdced4", dur)),
    ),
  );
  body.appendChild(
    pair(
      panel("Damage by spell", barList(c.damage_spells, "#ff2d55")),
      panel("Healing by spell", barList(c.healing_spells, "#cdced4")),
      panel("Killed most", barList(c.kills, "#ff2d55")),
      panel("What killed you", barList(c.killers, "#cdced4")),
    ),
  );
  body.appendChild(pair(panel("Cast time by spell", barList(c.cast_time, "#ff2d55", dur)), panel("Emotes used", barList(c.emotes, "#cdced4"))));
  body.appendChild(pair(panel("Items gained", barList(c.items_in, "#ff2d55")), panel("Items used or sold", barList(c.items_out, "#cdced4"))));

  for (const p of [picker("Character stats over time", tr.stats, statLabel, "#ff2d55"), picker("In-game statistics over time", tr.statistics, (s) => s.name, "#ff2d55")]) {
    if (p) body.appendChild(p);
  }
  wireLineCharts(body);
}
