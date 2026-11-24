// Legacy: the selected character's Legacy talents, their ranks over time and what each rank did.

import { invoke, el, elText, esc, num, dateStr, dateTime, state, goal, tile, tiles } from "../lib.js";
import { play } from "../motion.js";
import { mod, label, goalList } from "./goals-section.js";

const table = (head, rows) => {
  const wrap = el("div", "scroll-x");
  const t = el("table", "talents");
  t.innerHTML = `<thead><tr>${head.map(([label, cls]) => `<th class="${cls ?? ""}">${label}</th>`).join("")}</tr></thead><tbody>${rows.join("")}</tbody>`;
  wrap.appendChild(t);
  return wrap;
};

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

const span = (r) => (r.trials ? (dateStr(r.first_ms) === dateStr(r.last_ms) ? dateStr(r.first_ms) : `${dateStr(r.first_ms)} to ${dateStr(r.last_ms)}`) : "");

function measureTable(t) {
  const m = t.measure;
  const rows = m.rows.filter((r) => r.trials || r.stated != null);
  if (!rows.length) return null;
  const box = el("div");
  box.appendChild(el("div", "dim", esc(m.what)));
  box.appendChild(
    table(
      [["Rank", "num"], [esc(m.trials), "num"], ["Measured", "num"], ["Against rank 0", "num"], ["Tooltip", "num"], ["Recorded"]],
      rows.map(
        (r) =>
          `<tr${r.rank === t.rank ? ' class="current"' : ""}><td class="num">${r.rank}</td><td class="num">${num(r.trials)}</td><td class="num">${value(m, r.value)}</td><td class="num">${effect(m, r.effect)}</td><td class="num">${r.stated == null ? "—" : `${fixed(r.stated, 0)}%`}</td><td class="dim">${span(r)}</td></tr>`
      )
    )
  );
  return box;
}

function talentPanel(t) {
  const p = el("div", "panel");
  p.style.marginTop = "12px";
  if (t.max_ranks > 0) p.append(goal(t.rank, t.max_ranks, { label: t.name, count: false }));
  else p.innerHTML = `<h3>${esc(t.name)} <span class="dim">· rank ${t.rank}</span></h3>`;
  if (t.texts.length) {
    p.appendChild(
      table(
        [["Rank", "num"], ["Tooltip"]],
        t.texts.map((x) => `<tr${x.rank === t.rank ? ' class="current"' : ""}><td class="num">${x.rank}</td><td class="text">${esc(x.text)}</td></tr>`)
      )
    );
  }
  if (t.measure) {
    const m = measureTable(t);
    if (m) p.appendChild(m);
  }
  const moves = t.changes.filter((c) => c.from != null);
  if (moves.length) {
    p.appendChild(el("div", "dim", moves.map((c) => `${c.from}→${c.to} ${dateTime(c.ms)}`).join(" · ")));
  }
  return p;
}

const TREES = ["Professions", "Adventure", "Progression"];

function treeProgress(talents) {
  return TREES.map((name) => {
    const of = talents.filter((t) => t.tree === name && t.max_ranks > 0);
    return { name, rank: of.reduce((a, t) => a + t.rank, 0), max: of.reduce((a, t) => a + t.max_ranks, 0) };
  }).filter((t) => t.max > 0);
}

function nextRanks(talents, available, cost) {
  return talents
    .filter((t) => t.max_ranks > 0 && t.rank < t.max_ranks)
    .sort((a, b) => a.max_ranks - a.rank - (b.max_ranks - b.rank) || b.rank - a.rank || a.name.localeCompare(b.name))
    .slice(0, 6)
    .map((t) => goal(t.rank, t.max_ranks, { label: `${t.tree}: ${t.name}`, count: false, note: `Next rank costs ${cost}`, eta: available >= cost ? "Can afford" : `${num(cost - available)} short` }));
}

function sheet(talents, legacy) {
  const box = el("div", "sheet goals-sheet legacy-sheet");
  const trees = treeProgress(talents);
  const spent = legacy ? legacy.trees.reduce((a, t) => a + t.spent, 0) : 0;
  const head = [];
  if (legacy) {
    head.push(goal(legacy.earned, legacy.max, { size: "lg", label: "Earned" }));
    head.push(tiles([tile("Spent", num(spent), "", { size: "md" }), tile("To spend", num(legacy.available), "", { size: "md" })], 2));
  }
  const stack = el("div", "goal-stack");
  stack.append(...head);
  const first = mod("w5 first", "Points", head.length ? stack : elText("p", "sc-empty", "No Legacy balance recorded"));
  const kids = [];
  if (trees.length) kids.push(label("Ranks bought"), goalList(trees.map((t) => goal(t.rank, t.max, { label: t.name, count: false }))));
  const next = legacy ? nextRanks(talents, legacy.available, legacy.rank_cost) : [];
  if (next.length) kids.push(label("Next ranks"), goalList(next));
  box.append(first, mod("w7 last", "Trees", ...(kids.length ? kids : [elText("p", "sc-empty", "No tree ranks recorded")])));
  return box;
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
  body.appendChild(sheet(data.talents, goals?.caps.legacy ?? null));
  let tree = null;
  for (const t of data.talents) {
    if (t.tree !== tree) {
      tree = t.tree;
      body.appendChild(el("h3", "section-title", esc(tree)));
    }
    body.appendChild(talentPanel(t));
  }
  play(body);
}
