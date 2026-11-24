// Legacy: the selected character's Legacy talents, their ranks over time and what each rank did.

import { invoke, el, elText, esc, num, dateStr, dateTime, state, goal, tile, tiles } from "../lib.js";
import { play } from "../motion.js";
import { mod } from "./goals-section.js";

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

function nextRank(t, legacy) {
  if (!legacy || t.rank >= t.max_ranks) return {};
  const cost = legacy.rank_cost;
  return { note: `Next rank costs ${cost}`, eta: legacy.available >= cost ? "Can afford" : `${num(cost - legacy.available)} short` };
}

function talentPanel(t, legacy) {
  const p = el("div", "panel");
  p.style.marginTop = "12px";
  if (t.max_ranks > 0) p.append(goal(t.rank, t.max_ranks, { label: t.name, count: false, ...nextRank(t, legacy) }));
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

function sheet(legacy) {
  const box = el("div", "sheet goals-sheet legacy-sheet");
  if (!legacy) {
    box.append(mod("w12 first last", "Points", elText("p", "sc-empty", "No Legacy balance recorded")));
    return box;
  }
  const spent = legacy.trees.reduce((a, t) => a + t.spent, 0);
  const head = el("div", "goal-stack points-head");
  head.append(goal(legacy.earned, legacy.max, { size: "lg", label: "Earned" }), tiles([tile("Spent", num(spent), "", { size: "md" }), tile("To spend", num(legacy.available), "", { size: "md" })], 2));
  box.append(mod("w12 first last", "Points", head));
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
  const legacy = goals?.caps.legacy ?? null;
  body.appendChild(sheet(legacy));
  let tree = null;
  for (const t of data.talents) {
    if (t.tree !== tree) {
      tree = t.tree;
      body.appendChild(el("h3", "section-title", esc(tree)));
    }
    body.appendChild(talentPanel(t, legacy));
  }
  play(body);
}
