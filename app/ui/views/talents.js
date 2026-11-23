// Talents: the selected character's build by tab, the order points were taken in, and respecs.

import { invoke, el, esc, num, moneyHtml, dateTime, state } from "../lib.js";

const KIND = {
  pick: "Picked",
  refund: "Refunded",
  swap: "Switched choice",
  unseen: "Changed while not tracked",
};

const RESPEC = { refund: "Points refunded", reset: "Reset by the game", "spec change": "Spec changed by the game" };

const table = (head, rows) => {
  const wrap = el("div", "scroll-x");
  const t = el("table", "talents");
  t.innerHTML = `<thead><tr>${head.map(([label, cls]) => `<th class="${cls ?? ""}">${label}</th>`).join("")}</tr></thead><tbody>${rows.join("")}</tbody>`;
  wrap.appendChild(t);
  return wrap;
};

const level = (v) => (v == null ? "—" : v);

const talentName = (t) => esc(t.spell || t.name || (t.spell_id ? `Spell ${t.spell_id}` : `Node ${t.node_id}`));

function buildSection(body, build) {
  body.appendChild(el("h3", "section-title", "Build by tab"));
  const facts = [];
  if (build.spec) facts.push(esc(build.spec));
  if (build.groups > 1) facts.push(`Talent group ${build.group} of ${build.groups}`);
  if (build.unspent_class != null || build.unspent_spec != null) {
    facts.push(`${build.unspent_class ?? 0} class and ${build.unspent_spec ?? 0} spec points unspent`);
  }
  facts.push(`read ${dateTime(build.ms)}`);
  body.appendChild(el("div", "dim", facts.join(" · ")));
  for (const tab of build.tabs) {
    const p = el("div", "panel");
    p.style.marginTop = "12px";
    const pts = `${num(tab.spent)} ${tab.spent === 1 ? "point" : "points"}`;
    p.innerHTML = `<h3>${esc(tab.name || "Other talents")} <span class="dim">· ${pts}${tab.available != null ? `, ${num(tab.available)} available` : ""}</span></h3>`;
    if (!tab.nodes.length) {
      p.appendChild(el("div", "empty", "No points spent."));
    } else {
      p.appendChild(
        table(
          [["Talent"], ["Rank", "num"]],
          tab.nodes.map(
            (n) => `<tr><td>${talentName(n)}</td><td class="num">${n.rank}/${n.max_ranks}</td></tr>`
          )
        )
      );
    }
    body.appendChild(p);
  }
}

function orderSection(body, data) {
  body.appendChild(el("h3", "section-title", "Order points were taken"));
  const changes = data.changes.filter((c) => c.kind !== "baseline");
  if (!changes.length && !data.held.length) {
    body.appendChild(el("div", "empty", "No talent changes recorded."));
    return;
  }
  const groups = new Set(data.changes.map((c) => c.group));
  const multi = groups.size > 1;
  const head = [["Level", "num"], ["Point", "num"], ["Talent"], ["Tab"], ["Change"], ["Rank", "num"], ["Time"]];
  if (multi) head.splice(1, 0, ["Group", "num"]);
  const group = (g) => (multi ? `<td class="num">${g}</td>` : "");
  const rows = [];
  const held = data.held.map((h) => ({ ms: h.ms, row: `<tr><td class="num">${level(h.level)}</td>${group(h.group)}<td class="num dim">—</td><td colspan="4" class="dim">${num(h.points)} ${h.points === 1 ? "point" : "points"} already spent in ${num(h.nodes)} ${h.nodes === 1 ? "talent" : "talents"} when Scarlet first read group ${h.group}</td><td class="dim">${dateTime(h.ms)}</td></tr>` }));
  const taken = changes.map((c) => ({
    ms: c.ms,
    row: `<tr><td class="num">${level(c.level)}</td>${group(c.group)}<td class="num">${c.point_no ?? "—"}</td><td>${talentName(c)}</td><td class="dim">${esc(c.tab || "")}</td><td>${KIND[c.kind] ?? esc(c.kind)}</td><td class="num">${c.old_rank ?? ""}→${c.rank}/${c.max_ranks}</td><td class="dim">${dateTime(c.ms)}</td></tr>`,
  }));
  for (const r of [...held, ...taken].sort((a, b) => a.ms - b.ms)) rows.push(r.row);
  body.appendChild(table(head, rows));
}

function respecSection(body, data) {
  body.appendChild(el("h3", "section-title", "Respecs"));
  if (!data.respecs.length) {
    body.appendChild(el("div", "empty", "No respecs recorded."));
    return;
  }
  body.appendChild(
    table(
      [["Time"], ["Level", "num"], ["Group", "num"], ["Kind"], ["Points", "num"], ["Cost", "num"]],
      data.respecs.map(
        (r) =>
          `<tr><td>${dateTime(r.ms)}</td><td class="num">${level(r.level)}</td><td class="num">${r.group}</td><td>${RESPEC[r.kind] ?? esc(r.kind)}</td><td class="num">${num(r.points)}</td><td class="num">${r.cost_copper == null ? "—" : moneyHtml(r.cost_copper)}</td></tr>`
      )
    )
  );
}

export async function render(body, given = null) {
  const chars = state.store?.chars ?? [];
  const ch = given ?? state.ch ?? (chars.length === 1 ? chars[0] : null);
  if (!ch) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", "Select a character."));
    return;
  }
  const data = await invoke("talents", { ch });
  body.innerHTML = "";
  if (!data || (!data.build && !data.changes.length)) {
    body.appendChild(el("div", "empty", "No talent lines recorded for this character."));
    return;
  }
  if (data.build) buildSection(body, data.build);
  orderSection(body, data);
  respecSection(body, data);
}
