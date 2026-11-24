// Loot: what each player received as a quality-coloured bar per loot rule, with their rolls, and drop rates per
// creature as chance bars.

import { invoke, el, elText, state, esc, displayName, num, stack, wireTips } from "../lib.js";
import { counted, play } from "../motion.js";

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
const SHADES = [
  ["Poor", "--q-poor"],
  ["Common", "--q-common"],
  ["Uncommon", "--q-uncommon"],
  ["Rare", "--q-rare"],
  ["Epic", "--q-epic"],
  ["Legendary", "--q-legendary"],
  ["Unknown", "--s-0"],
];
const ROLLS = [
  ["Won", "--alt-4", "rolls_won"],
  ["Lost", "--alt-1", "rolls_lost"],
  ["Passed", "--s-0", "rolls_passed"],
];
const CREATURE_LIMIT = 48;
const ITEMS_PER_CREATURE = 8;

function ruleLabel(r) {
  if (r.method === "solo") return "Solo";
  const bits = [];
  if (r.size) bits.push(`${r.size} ${r.size === 1 ? "player" : "players"}`);
  if (r.threshold !== "" && QUALITY[r.threshold]) bits.push(`${QUALITY[r.threshold]}+`);
  const name = METHOD[r.method] ?? r.method;
  return bits.length ? `${name} · ${bits.join(", ")}` : name;
}

const pctOf = (drops, looted) => (looted ? (drops / looted) * 100 : 0);

function pctText(v) {
  return `${v >= 10 || v === 0 ? Math.round(v) : v.toFixed(1)}%`;
}

function panel(title) {
  const p = el("div", "panel");
  p.innerHTML = `<h3>${title}</h3>`;
  return p;
}

function key(rows) {
  const k = el("div", "sc-key");
  for (const [label, value, c] of rows) {
    const s = el("span");
    s.style.setProperty("--c", `var(${c})`);
    s.append(`${label} `, counted(elText("b", "", num(value)), value, num));
    k.append(s);
  }
  return k;
}

function creatures(view) {
  const byNpc = new Map();
  for (const d of view.drops) {
    let c = byNpc.get(d.npc);
    if (!c) {
      c = { name: d.npc_name || `Creature ${d.npc}`, rules: new Map(), items: new Map() };
      byNpc.set(d.npc, c);
    }
    c.rules.set(d.rule, Math.max(c.rules.get(d.rule) ?? 0, d.looted));
    let it = c.items.get(d.item);
    if (!it) {
      it = { name: d.item_name || `Item ${d.item}`, drops: 0, byRule: [] };
      c.items.set(d.item, it);
    }
    it.drops += d.drops;
    it.byRule.push(d);
  }
  return [...byNpc.values()]
    .map((c) => {
      const corpses = [...c.rules.values()].reduce((a, n) => a + n, 0);
      const items = [...c.items.values()].sort((a, b) => b.drops - a.drops || a.name.localeCompare(b.name));
      return { name: c.name, rules: c.rules, corpses, items, search: `${c.name} ${items.map((i) => i.name).join(" ")}`.toLowerCase() };
    })
    .sort((a, b) => b.corpses - a.corpses || a.name.localeCompare(b.name));
}

function creatureCard(c, view) {
  const shown = c.items.slice(0, ITEMS_PER_CREATURE);
  const rows = shown
    .map((it, n) => {
      const v = pctOf(it.drops, c.corpses);
      const tip = view.rules.length > 1
        ? ` data-tip="${esc(it.name)}|${esc(it.byRule.map((d) => `${ruleLabel(view.rules[d.rule])} ${d.drops}/${c.rules.get(d.rule)}`).join(" · "))}"`
        : "";
      return `<div class="row"${tip}><span>${esc(it.name)}</span><b>${pctText(v)} <small>${num(it.drops)}/${num(c.corpses)}</small></b><i style="--w:${v.toFixed(1)}%;--n:${n}"></i></div>`;
    })
    .join("");
  const more = c.items.length - shown.length;
  return `<section class="lt-creature"><div class="lt-head"><h4>${esc(c.name)}</h4><span>${num(c.corpses)} ${c.corpses === 1 ? "corpse" : "corpses"}</span></div><div class="sc-bars">${rows}</div>${more > 0 ? `<p class="dim">${num(more)} more ${more === 1 ? "item" : "items"}</p>` : ""}</section>`;
}

function dropRates(view) {
  const p = panel("Drop rates");
  const list = creatures(view);
  if (!list.length) {
    p.appendChild(el("div", "empty", "No creature loot with an item list recorded yet."));
    return p;
  }
  const q = el("input");
  q.placeholder = "Filter creature or item…";
  q.style.width = "260px";
  const grid = el("div", "lt-grid");
  const note = el("div", "dim");
  const draw = () => {
    const f = q.value.trim().toLowerCase();
    const hit = f ? list.filter((c) => c.search.includes(f)) : list;
    grid.innerHTML = hit.slice(0, CREATURE_LIMIT).map((c) => creatureCard(c, view)).join("");
    note.textContent = hit.length > CREATURE_LIMIT ? `Showing ${CREATURE_LIMIT} of ${num(hit.length)}.` : "";
  };
  let deb;
  q.oninput = () => { clearTimeout(deb); deb = setTimeout(draw, 120); };
  draw();
  p.append(q, grid, note);
  return p;
}

function received(view) {
  const p = panel("Items received and rolls");
  if (!view.attribution.length) {
    p.appendChild(el("div", "empty", "No loot recorded yet."));
    return p;
  }
  const list = view.attribution.slice().sort((a, b) => a.who.localeCompare(b.who) || a.rule - b.rule);
  const qTotals = SHADES.map(([label, c], i) => [label, list.reduce((n, a) => n + a.quality[i], 0), c]).filter((r) => r[1] > 0);
  const rTotals = ROLLS.map(([label, c, field]) => [label, list.reduce((n, a) => n + a[field], 0), c]).filter((r) => r[1] > 0);
  const keys = el("div", "lt-keys");
  if (qTotals.length) keys.append(key(qTotals));
  if (rTotals.length) keys.append(key(rTotals));

  const most = Math.max(...list.map((a) => a.items), 1);
  const rows = el("div", "lt-rows");
  let last = null;
  for (const a of list) {
    const row = el("div", "lt-row");
    row.append(elText("span", "lt-who", a.who === last ? "" : displayName(a.who)), elText("span", "lt-rule", ruleLabel(view.rules[a.rule])));
    last = a.who;
    const track = el("div", "lt-track");
    const parts = SHADES.map(([label, c], i) => [label, a.quality[i], c]).filter((r) => r[1] > 0);
    if (parts.length) {
      const bar = stack(parts, { key: false, h: 10 });
      bar.style.width = `${Math.max(6, (a.items / most) * 100)}%`;
      track.append(bar);
    } else {
      track.append(el("i", "lt-none"));
    }
    const total = elText("b", "lt-n", num(a.items));
    counted(total, a.items, num);
    row.append(total, track);
    const rolls = el("div", "lt-rolls");
    const rolled = ROLLS.filter(([, , field]) => a[field] > 0);
    if (rolled.length) {
      for (const [label, c, field] of rolled) {
        const s = elText("span", "lt-roll", num(a[field]));
        s.style.setProperty("--c", `var(${c})`);
        s.dataset.tip = `${label}|${num(a[field])} ${a[field] === 1 ? "roll" : "rolls"}`;
        rolls.append(s);
      }
      const bar = stack(rolled.map(([label, c, field]) => [label, a[field], c]), { key: false, h: 6 });
      bar.classList.add("lt-rollbar");
      rolls.append(bar);
    }
    row.append(rolls);
    rows.append(row);
  }
  p.append(keys, rows);
  return p;
}

export async function render(body) {
  const view = await invoke("loot", { ch: state.ch });
  body.innerHTML = "";
  wireTips();
  const page = el("div", "lt");
  page.append(dropRates(view), received(view));
  body.append(page);
  play(page);
}
