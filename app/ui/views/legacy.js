// Legacy: the selected character's Legacy trees as the in-game talent frame, their ranks over time and what each rank did.

import { invoke, el, elText, esc, num, dayKey, dayRange, shortDay, stampHtml, state, goal, tile, tiles } from "../lib.js";
import { play } from "../motion.js";
import { hydrate } from "./classblocks.js";
import { mod } from "./goals-section.js";
import { chip, treeFrame } from "./tree-frame.js";

const fixed = (v, digits = 1) => (v == null ? "—" : v.toFixed(digits));

function value(m, v) {
  if (v == null) return "—";
  return m.unit === "%" ? `${fixed(v)}%` : `${fixed(v)} ${esc(m.unit)}`;
}

function effect(m, v) {
  if (v == null) return "—";
  const sign = v > 0 ? "+" : "";
  return m.kind === "chance" ? `${sign}${fixed(v)} points` : `${sign}${fixed(v, 0)}%`;
}

const span = (r) => (r.trials ? (dayKey(r.first_ms) === dayKey(r.last_ms) ? shortDay(r.first_ms) : dayRange(r.first_ms, r.last_ms)) : "");

function effectBars(t) {
  const m = t.measure;
  const rows = m.rows.filter((r) => r.trials || r.stated != null);
  if (!rows.length) return null;
  const base = m.rows.find((r) => r.rank === 0)?.value;
  const markAt = (r) => {
    if (r.stated == null) return null;
    if (m.kind === "chance") return r.stated;
    if (!(base > 0)) return null;
    return base * (1 + ((r.effect ?? 1) < 0 ? -r.stated : r.stated) / 100);
  };
  const top = Math.max(1e-9, ...rows.flatMap((r) => [r.value ?? 0, markAt(r) ?? 0]));
  const box = el("div", "leg-effects");
  box.append(elText("div", "dim", m.what));
  for (const r of rows) {
    const line = el("div", `leg-effect${r.rank === t.rank ? " current" : ""}`);
    const fill = r.value == null ? "" : `<i class="leg-fill" style="width:${(r.value / top) * 100}%"></i>`;
    const at = markAt(r);
    const mark = at == null ? "" : `<i class="leg-mark" style="left:${(at / top) * 100}%" title="Tooltip ${fixed(r.stated, 0)}%"></i>`;
    line.innerHTML = `<span class="leg-rank">Rank ${r.rank}</span><span class="leg-track">${fill}${mark}</span><span class="leg-read"><b>${value(m, r.value)}</b> measured in ${num(r.trials)} ${esc(m.trials.toLowerCase())}${r.effect == null ? "" : ` · ${effect(m, r.effect)} against rank 0`}${r.stated == null ? "" : ` · tooltip ${fixed(r.stated, 0)}%`}${span(r) ? ` · ${span(r)}` : ""}</span>`;
    box.append(line);
  }
  return box;
}

function history(t) {
  const held = t.changes.find((c) => c.from == null);
  const moves = t.changes.filter((c) => c.from != null);
  if (!held && !moves.length) return null;
  const list = el("ol", "leg-hist");
  if (held) list.insertAdjacentHTML("beforeend", `<li><b>Rank ${held.to}</b><span>Already held when Scarlet first read this character</span></li>`);
  for (const c of moves) list.insertAdjacentHTML("beforeend", `<li><b>${c.from}→${c.to}</b><span>${stampHtml(c.ms)}</span></li>`);
  return list;
}

function nextRank(t, legacy) {
  if (!legacy || t.rank >= t.max_ranks) return {};
  const cost = legacy.rank_cost;
  return { note: `Next rank costs ${cost}`, eta: legacy.available >= cost ? "Can afford" : `${num(cost - legacy.available)} short` };
}

function talentCard(t, legacy) {
  const p = el("div", "panel leg-card");
  p.dataset.node = t.node_id;
  const head = el("div", "leg-card-head");
  head.append(chip(t.name, t.icon));
  if (t.max_ranks > 0) head.append(goal(t.rank, t.max_ranks, { label: t.name, count: false, ...nextRank(t, legacy) }));
  else head.append(el("h3", "", `${esc(t.name)} <span class="dim">· rank ${t.rank}</span>`));
  p.append(head);
  if (t.texts.length) {
    const texts = el("ul", "leg-texts");
    for (const x of t.texts) texts.insertAdjacentHTML("beforeend", `<li${x.rank === t.rank ? ' class="current"' : ""}><b>Rank ${x.rank}</b><span>${esc(x.text)}</span></li>`);
    p.append(texts);
  }
  const bars = t.measure ? effectBars(t) : null;
  if (bars) p.append(bars);
  const moved = history(t);
  if (moved) p.append(moved);
  return p;
}

function pips(legacy, spent) {
  const box = el("div", "leg-pips");
  box.setAttribute("role", "img");
  box.setAttribute("aria-label", `${legacy.earned} of ${legacy.max} points earned, ${spent} spent, ${legacy.available} to spend`);
  for (let n = 0; n < legacy.max; n += 1) box.append(el("i", n < spent ? "spent" : n < legacy.earned ? "free" : ""));
  return box;
}

function sheet(legacy) {
  const box = el("div", "sheet goals-sheet legacy-sheet");
  if (!legacy) {
    box.append(mod("w12 first last", "Points", elText("p", "sc-empty", "No Legacy balance recorded")));
    return box;
  }
  const spent = legacy.trees.reduce((a, t) => a + t.spent, 0);
  const head = el("div", "leg-head");
  const lead = el("div", "leg-lead");
  lead.append(el("div", "leg-earned", `<span>Earned</span> <b>${num(legacy.earned)}</b> <em>/ ${num(legacy.max)}</em> <small>${legacy.max > 0 ? Math.floor((legacy.earned / legacy.max) * 100) : 0}%</small>`), pips(legacy, spent));
  const cost = legacy.rank_cost;
  const next = tile("Next rank", num(cost), legacy.available >= cost ? "Can afford" : `${num(cost - legacy.available)} short`, { size: "md" });
  head.append(lead, tiles([tile("Spent", num(spent), "", { size: "md" }), tile("To spend", num(legacy.available), "", { size: "md" }), next], 3));
  box.append(mod("w12 first last", "Points", head));
  return box;
}

function treePanel(name, talents, links, legacy) {
  const p = el("div", "panel tf-tab");
  p.dataset.tree = name;
  const spent = legacy?.trees.find((t) => t.name === name)?.spent ?? talents.reduce((a, t) => a + t.rank, 0);
  p.innerHTML = `<h3>${esc(name)} <span class="dim">· ${num(spent)} ${spent === 1 ? "point" : "points"}</span></h3>`;
  const nodes = talents.map((t) => ({ id: t.node_id, col: t.col, row: t.row, name: t.name, icon: t.icon, rank: t.rank, max: t.max_ranks, locked: t.placeholder }));
  p.append(treeFrame(nodes, links.map((l) => [l.from, l.to]), { cols: 4, rows: 4 }));
  return p;
}

export async function render(body) {
  const chars = state.store?.chars ?? [];
  const ch = state.ch ?? (chars.length === 1 ? chars[0] : null);
  if (!ch) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", "Select a character."));
    return;
  }
  const [data, goals] = await Promise.all([invoke("legacy", { ch }), invoke("goals", { ch }).catch(() => null)]);
  body.innerHTML = "";
  if (!data || !data.talents.length) {
    body.appendChild(el("div", "empty", "No Legacy lines recorded for this character."));
    return;
  }
  const legacy = goals?.caps.legacy ?? null;
  body.appendChild(sheet(legacy));
  const trees = [...new Set(data.talents.map((t) => t.tree_id))].sort();
  const row = el("div", "tf-tabs");
  for (const id of trees) {
    const talents = data.talents.filter((t) => t.tree_id === id);
    row.append(treePanel(talents[0].tree, talents, data.links.filter((l) => l.tree_id === id), legacy));
  }
  body.appendChild(row);
  for (const id of trees) {
    const cards = data.talents.filter((t) => t.tree_id === id && !t.placeholder && (t.rank > 0 || t.texts.length || t.changes.length));
    if (!cards.length) continue;
    body.appendChild(el("h3", "section-title", esc(cards[0].tree)));
    const list = el("div", "leg-cards");
    for (const t of cards) list.append(talentCard(t, legacy));
    body.appendChild(list);
  }
  hydrate(body);
  play(body);
}
