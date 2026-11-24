// Loot: what each player received as a quality-coloured bar per loot rule, with their rolls, and drop rates per
// creature as chance bars.

import { invoke, el, elText, state, esc, displayName, num, dur, stack, tiles, fillRow, pace, shortDate, wireTips } from "../lib.js";
import { counted, play } from "../motion.js";
import { openAs } from "./paces.js";

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
const CREATURE_TOP = 12;
const ITEMS_PER_CREATURE = 8;
const UNCOMMON = 2;
const CHASE_PER_CREATURE = 3;
const FARMED_COLUMNS = 4;

const fixed = (v) => (v >= 10 ? num(v) : String(Math.round(v * 10) / 10));

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
  const info = new Map(view.items.map((i) => [i.item, i]));
  const killed = new Map(view.kills.map((k) => [k.npc, k.kills]));
  for (const d of view.drops) {
    let c = byNpc.get(d.npc);
    if (!c) {
      c = { npc: d.npc, name: d.npc_name || `Creature ${d.npc}`, rules: new Map(), items: new Map() };
      byNpc.set(d.npc, c);
    }
    c.rules.set(d.rule, Math.max(c.rules.get(d.rule) ?? 0, d.looted));
    let it = c.items.get(d.item);
    if (!it) {
      it = { id: d.item, name: d.item_name || `Item ${d.item}`, drops: 0, byRule: [] };
      c.items.set(d.item, it);
    }
    it.drops += d.drops;
    it.byRule.push(d);
  }
  return [...byNpc.values()]
    .map((c) => {
      const corpses = [...c.rules.values()].reduce((a, n) => a + n, 0);
      const items = [...c.items.values()].sort((a, b) => b.drops - a.drops || a.name.localeCompare(b.name));
      const kills = Math.max(killed.get(c.npc) ?? 0, corpses);
      const chase = items
        .filter((it) => it.drops > 0 && info.get(it.id)?.received === 0 && info.get(it.id)?.quality >= UNCOMMON)
        .slice(0, CHASE_PER_CREATURE)
        .map((it) => ({ name: it.name, kills, drops: it.drops, expected: Math.ceil(corpses / it.drops) }));
      return { npc: c.npc, name: c.name, rules: c.rules, corpses, items, chase, search: `${c.name} ${items.map((i) => i.name).join(" ")}`.toLowerCase() };
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
  return `<section class="lt-creature"><div class="lt-head"><h4>${esc(c.name)}</h4><span>${num(c.corpses)} ${c.corpses === 1 ? "corpse" : "corpses"}</span></div><div class="sc-bars">${rows}</div>${c.chase.length ? `<div class="lt-chase" data-npc="${esc(c.npc)}"></div>` : ""}${more > 0 ? `<p class="dim">${num(more)} more ${more === 1 ? "item" : "items"}</p>` : ""}</section>`;
}

function chaseRow(x) {
  const row = el("div", "lt-miss");
  row.append(elText("span", "lt-miss-name", x.name), elText("span", "", `dropped ${num(x.drops)}`), elText("span", "lt-miss-odds", `${num(x.kills)} ${x.kills === 1 ? "kill" : "kills"} · about 1 in ${num(x.expected)}`));
  return row;
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
  const all = elText("button", "sc-btn quiet small", "");
  all.type = "button";
  let expanded = false;
  const draw = () => {
    const f = q.value.trim().toLowerCase();
    const hit = f ? list.filter((c) => c.search.includes(f)) : list;
    const shown = hit.slice(0, f || expanded ? CREATURE_LIMIT : CREATURE_TOP);
    grid.innerHTML = shown.map((c) => creatureCard(c, view)).join("");
    for (const slot of grid.querySelectorAll(".lt-chase")) {
      const c = shown.find((x) => x.npc === slot.dataset.npc);
      slot.append(elText("p", "sc-label", "Dropped, not received"), ...c.chase.map(chaseRow));
    }
    note.textContent = shown.length < hit.length && (f || expanded) ? `Showing ${CREATURE_LIMIT} of ${num(hit.length)}.` : "";
    all.hidden = Boolean(f) || expanded || hit.length <= CREATURE_TOP;
    all.textContent = `Show all ${num(hit.length)} creatures`;
  };
  all.onclick = () => {
    expanded = true;
    draw();
  };
  let deb;
  q.oninput = () => { clearTimeout(deb); deb = setTimeout(draw, 120); };
  draw();
  p.append(q, grid, note, all);
  return p;
}

function farmTile(f) {
  const p = f.pace;
  const t = pace(p.rate, p.spread, { label: f.name, total: `${num(p.count)} in ${dur(p.played)}, ${shortDate(p.from)} · ${num(f.total)} looted`, format: fixed, text: fixed, best: p.isBest, peak: p.best, peers: p.peers });
  if (!state.ch) t.append(elText("span", "sc-sub", displayName(f.character)));
  if (f.record) {
    const a = elText("a", "sc-sub sc-pace-record", `Most in a session ${num(f.record.value)} · ${shortDate(f.record.from)}`);
    a.href = `#sessions/${f.record.from}-${f.record.to}`;
    a.onclick = openAs(f.character);
    t.append(a);
  }
  return t;
}

function firstTile(f) {
  const a = el("a", "sc-tile sc-record sc-first");
  a.href = `#sessions/${f.first[0]}-${f.first[1]}`;
  a.onclick = openAs(f.character);
  const name = elText("span", "sc-first-name", f.name);
  if (QUALITY[f.quality]) name.style.color = `var(--q-${QUALITY[f.quality]})`;
  a.append(name, elText("span", "sc-sub", `First looted ${shortDate(f.first[0])}`));
  if (!state.ch) a.append(elText("span", "sc-sub", displayName(f.character)));
  return a;
}

function farming(view) {
  const { farmed, firsts } = view.farming;
  if (!farmed.length && !firsts.length) return null;
  const p = panel("Farming");
  if (farmed.length) {
    p.append(elText("p", "sc-label", "Most looted, items an hour"), tiles(farmed.map(farmTile), Math.min(FARMED_COLUMNS, farmed.length)));
  }
  if (firsts.length) {
    p.append(elText("p", "sc-label", "First rare drops"), tiles(fillRow(firsts.map(firstTile), Math.min(FARMED_COLUMNS, firsts.length)), Math.min(FARMED_COLUMNS, firsts.length)));
  }
  return p;
}

const ownCharacters = () => new Set(state.store?.chars ?? []);

function sumOf(rows) {
  const t = { items: 0, quality: Array(SHADES.length).fill(0), rolls_won: 0, rolls_lost: 0, rolls_passed: 0 };
  for (const a of rows) {
    t.items += a.items;
    a.quality.forEach((n, i) => (t.quality[i] += n));
    for (const [, , field] of ROLLS) t[field] += a[field];
  }
  return t;
}

function lootLine(who, note, t, most) {
  const row = el("div", "lt-row");
  row.append(elText("span", "lt-who", who), elText("span", "lt-rule", note));
  const track = el("div", "lt-track");
  const parts = SHADES.map(([label, c], i) => [label, t.quality[i], c]).filter((r) => r[1] > 0);
  if (parts.length) {
    const bar = stack(parts, { key: false, h: 10 });
    bar.style.width = `${Math.max(6, (t.items / most) * 100)}%`;
    track.append(bar);
  } else {
    track.append(el("i", "lt-none"));
  }
  row.append(counted(elText("b", "lt-n", num(t.items)), t.items, num), track);
  const rolls = el("div", "lt-rolls");
  const rolled = ROLLS.filter(([, , field]) => t[field] > 0);
  if (rolled.length) {
    for (const [label, c, field] of rolled) {
      const s = elText("span", "lt-roll", num(t[field]));
      s.style.setProperty("--c", `var(${c})`);
      s.dataset.tip = `${label}|${num(t[field])} ${t[field] === 1 ? "roll" : "rolls"}`;
      rolls.append(s);
    }
    const bar = stack(rolled.map(([label, c, field]) => [label, t[field], c]), { key: false, h: 6 });
    bar.classList.add("lt-rollbar");
    rolls.append(bar);
  }
  row.append(rolls);
  return row;
}

function entry(who, note, t, most, kids) {
  const wrap = el("div", "lt-entry");
  const row = lootLine(who, note, t, most);
  wrap.append(row);
  if (!kids) return wrap;
  const sub = el("div", "lt-sub");
  sub.hidden = true;
  row.classList.add("lt-toggle");
  row.tabIndex = 0;
  row.setAttribute("role", "button");
  row.setAttribute("aria-expanded", "false");
  const flip = () => {
    if (!sub.children.length) sub.append(...kids());
    sub.hidden = !sub.hidden;
    row.setAttribute("aria-expanded", String(!sub.hidden));
  };
  row.onclick = flip;
  row.onkeydown = (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      flip();
    }
  };
  wrap.append(sub);
  return wrap;
}

function playerEntry(who, rows, view, most) {
  const withItems = rows.filter((a) => a.items > 0).sort((a, b) => a.rule - b.rule);
  const total = sumOf(rows);
  const name = displayName(who);
  if (withItems.length < 2) {
    const note = withItems.length ? ruleLabel(view.rules[withItems[0].rule]) : "Rolls only";
    return entry(name, note, total, most, null);
  }
  const top = Math.max(...withItems.map((a) => a.items), 1);
  return entry(name, `${withItems.length} loot rules`, total, most, () =>
    withItems.map((a) => entry("", ruleLabel(view.rules[a.rule]), sumOf([a]), top, null))
  );
}

function received(view) {
  const p = panel("Items received and rolls");
  if (!view.attribution.length) {
    p.appendChild(el("div", "empty", "No loot recorded yet."));
    return p;
  }
  const qTotals = SHADES.map(([label, c], i) => [label, view.attribution.reduce((n, a) => n + a.quality[i], 0), c]).filter((r) => r[1] > 0);
  const rTotals = ROLLS.map(([label, c, field]) => [label, view.attribution.reduce((n, a) => n + a[field], 0), c]).filter((r) => r[1] > 0);
  const keys = el("div", "lt-keys");
  if (qTotals.length) keys.append(key(qTotals));
  if (rTotals.length) keys.append(key(rTotals));

  const byWho = new Map();
  for (const a of view.attribution) byWho.set(a.who, [...(byWho.get(a.who) ?? []), a]);
  const mine = ownCharacters();
  const byItems = (a, b) => sumOf(b[1]).items - sumOf(a[1]).items || a[0].localeCompare(b[0]);
  const own = [...byWho].filter(([who]) => mine.has(who)).sort(byItems);
  const others = [...byWho].filter(([who]) => !mine.has(who)).sort(byItems);

  const most = (players) => Math.max(...players.map(([, rows]) => sumOf(rows).items), 1);
  const rows = el("div", "lt-rows");
  const topMost = Math.max(most(own), others.length ? sumOf(others.flatMap(([, r]) => r)).items : 0, 1);
  for (const [who, list] of own) rows.append(playerEntry(who, list, view, topMost));
  if (others.length) {
    const everyone = others.flatMap(([, r]) => r);
    const note = `${num(others.length)} ${others.length === 1 ? "player" : "players"}`;
    rows.append(
      entry(`Group members (${num(others.length)})`, note, sumOf(everyone), topMost, () => {
        const inner = most(others);
        return others.map(([who, list]) => playerEntry(who, list, view, inner));
      })
    );
  }
  p.append(keys, rows);
  return p;
}

export async function render(body) {
  const view = await invoke("loot", { ch: state.ch });
  body.innerHTML = "";
  wireTips();
  const page = el("div", "lt");
  page.append(...[farming(view), received(view), dropRates(view)].filter(Boolean));
  body.append(page);
  play(page);
}
