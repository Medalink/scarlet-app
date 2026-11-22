// Loot: drop rates per creature and item, one column per loot rule, and what
// each player received and rolled under each rule.

import { invoke, el, state, esc, displayName, num } from "../lib.js";

const METHOD = {
  solo: "Solo",
  group: "Group loot",
  needbeforegreed: "Need before greed",
  masterlooter: "Master looter",
  master: "Master looter",
  freeforall: "Free for all",
  roundrobin: "Round robin",
  personal: "Personal loot",
  unknown: "Unknown rule",
};
const QUALITY = ["poor", "common", "uncommon", "rare", "epic", "legendary"];
const ROW_LIMIT = 300;

function ruleLabel(r) {
  if (r.method === "solo") return "Solo";
  const bits = [];
  if (r.size) bits.push(`${r.size} ${r.size === 1 ? "player" : "players"}`);
  if (r.threshold !== "" && QUALITY[r.threshold]) bits.push(`${QUALITY[r.threshold]}+`);
  const name = METHOD[r.method] ?? r.method;
  return bits.length ? `${name} · ${bits.join(", ")}` : name;
}

function pct(drops, looted) {
  const v = (drops / looted) * 100;
  return `${v >= 10 || v === 0 ? Math.round(v) : v.toFixed(1)}%`;
}

function panel(title) {
  const p = el("div", "panel");
  p.innerHTML = `<h3>${title}</h3>`;
  return p;
}

function dropTable(view) {
  const p = panel("Drop rates");
  const byItem = new Map();
  for (const d of view.drops) {
    const key = `${d.npc}:${d.item}`;
    let row = byItem.get(key);
    if (!row) {
      row = { d, cells: new Map(), looted: 0 };
      byItem.set(key, row);
    }
    row.cells.set(d.rule, d);
    row.looted += d.looted;
  }
  const rows = [...byItem.values()].sort((a, b) => b.looted - a.looted);
  if (!rows.length) {
    p.appendChild(el("div", "empty", "No creature loot with an item list recorded yet."));
    return p;
  }

  const q = el("input");
  q.placeholder = "Filter creature or item…";
  q.style.width = "260px";
  const note = el("div", "dim");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Creature</th><th>Item</th>${view.rules
    .map((r) => `<th class="num">${esc(ruleLabel(r))}</th>`)
    .join("")}</tr></thead><tbody></tbody>`;
  const tb = t.querySelector("tbody");

  const draw = () => {
    const f = q.value.trim().toLowerCase();
    const hit = f
      ? rows.filter((r) => `${r.d.npc_name} ${r.d.item_name} ${r.d.npc} ${r.d.item}`.toLowerCase().includes(f))
      : rows;
    tb.innerHTML = hit
      .slice(0, ROW_LIMIT)
      .map(({ d, cells }) => {
        const tds = view.rules
          .map((_, i) => {
            const c = cells.get(i);
            // A rule with no corpse of this creature has no rate.
            return c
              ? `<td class="num">${pct(c.drops, c.looted)} <span class="dim">${c.drops}/${c.looted}</span></td>`
              : `<td class="num dim">—</td>`;
          })
          .join("");
        return `<tr><td>${esc(d.npc_name || `Creature ${d.npc}`)}</td><td>${esc(d.item_name || `Item ${d.item}`)}</td>${tds}</tr>`;
      })
      .join("");
    note.textContent = hit.length > ROW_LIMIT ? `Showing ${ROW_LIMIT} of ${num(hit.length)}.` : "";
  };
  let deb;
  q.oninput = () => { clearTimeout(deb); deb = setTimeout(draw, 120); };
  draw();
  const wrap = el("div", "scroll-x");
  wrap.appendChild(t);
  p.append(q, wrap, note);
  return p;
}

function attributionTable(view) {
  const p = panel("Items received and rolls");
  if (!view.attribution.length) {
    p.appendChild(el("div", "empty", "No loot recorded yet."));
    return p;
  }
  const rows = view.attribution
    .slice()
    .sort((a, b) => a.who.localeCompare(b.who) || a.rule - b.rule)
    .map((a) => {
      const q = a.quality;
      return `<tr><td>${esc(displayName(a.who))}</td><td>${esc(ruleLabel(view.rules[a.rule]))}</td>
        <td class="num">${num(a.items)}</td>
        <td class="num">${num(q[0])}</td><td class="num">${num(q[1])}</td><td class="num">${num(q[2])}</td>
        <td class="num">${num(q[3])}</td><td class="num">${num(q[4] + q[5])}</td><td class="num">${num(q[6])}</td>
        <td class="num">${num(a.rolls_won)}</td><td class="num">${num(a.rolls_lost)}</td>
        <td class="num">${num(a.rolls_passed)}</td></tr>`;
    })
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Player</th><th>Rule</th><th class="num">Items</th>
    <th class="num">Poor</th><th class="num">Common</th><th class="num">Uncommon</th><th class="num">Rare</th>
    <th class="num">Epic+</th><th class="num">Unknown</th>
    <th class="num">Won</th><th class="num">Lost</th><th class="num">Passed</th></tr></thead><tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  p.appendChild(wrap);
  return p;
}

export async function render(body) {
  const view = await invoke("loot", { ch: state.ch });
  body.innerHTML = "";
  body.appendChild(dropTable(view));
  body.appendChild(attributionTable(view));
}
