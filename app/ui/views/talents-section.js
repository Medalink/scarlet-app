// The talents section of Character: the selected character's build as the in-game talent frame, the order points were taken in, and respecs.

import { invoke, el, esc, num, moneyHtml, stampHtml, goal, stack, tile, tiles, shownCharacter } from "../lib.js";
import { play } from "../motion.js";
import { hydrate } from "./classblocks.js";
import { chip, treeFrame } from "./tree-frame.js";

const KIND = {
  pick: "Picked",
  refund: "Refunded",
  swap: "Switched choice",
  unseen: "Changed while not tracked",
};

const RESPEC = { refund: "Points refunded", reset: "Reset by the game", "spec change": "Spec changed by the game" };

const TAB_COLORS = ["--s-1", "--s-2", "--s-4"];

const ROWS = 7;

const plainName = (t) => t.spell || t.name || (t.spell_id ? `Spell ${t.spell_id}` : `Node ${t.node_id}`);

const points = (n) => `${num(n)} ${n === 1 ? "point" : "points"}`;

function pointsHead(build, spent, total) {
  const box = el("div", "goal-stack points-head");
  box.append(goal(spent, total, { size: "lg", label: "Points spent" }));
  const unspent = [["class", build.unspent_class], ["spec", build.unspent_spec]].filter(([, n]) => n != null);
  box.append(tiles([tile("Unspent", num(unspent.reduce((a, [, n]) => a + n, 0)), unspent.map(([kind, n]) => `${num(n)} ${kind}`).join(" · "), { size: "md" })], 1));
  return box;
}

function tabFrame(tab) {
  const nodes = tab.nodes.map((n) => ({ id: n.node_id, col: n.col, row: n.row, name: plainName(n), icon: n.icon, rank: n.rank, max: n.max_ranks, choices: n.choices }));
  const links = tab.nodes.flatMap((n) => n.to.map((to) => [n.node_id, to]));
  return treeFrame(nodes, links, { cols: 4, rows: ROWS });
}

function buildSection(body, build) {
  const spent = build.tabs.reduce((a, t) => a + t.spent, 0);
  const known = build.unspent_class != null || build.unspent_spec != null;
  const total = spent + (build.unspent_class ?? 0) + (build.unspent_spec ?? 0);
  const facts = [];
  if (build.spec) facts.push(esc(build.spec));
  if (build.groups > 1) facts.push(`Talent group ${build.group} of ${build.groups}`);
  if (known) facts.push(`${num(spent)} / ${num(total)}`);
  facts.push(stampHtml(build.ms, "Read "));
  const line = el("div", "title-line");
  line.append(el("h3", "section-title", "Talents"), el("span", "dim", facts.join(" · ")));
  body.appendChild(line);
  if (known && total > 0) {
    const head = el("div", "panel compact");
    head.style.marginTop = "12px";
    head.append(pointsHead(build, spent, total));
    if (spent > 0) {
      const split = stack(build.tabs.map((t, n) => [t.name || "Other talents", t.spent, TAB_COLORS[n % TAB_COLORS.length]]), { h: 10 });
      split.classList.add("tab-split");
      head.append(split);
    }
    body.appendChild(head);
  }
  const row = el("div", "tf-tabs");
  for (const tab of build.tabs) {
    const p = el("div", "panel tf-tab");
    p.dataset.tab = tab.id;
    p.innerHTML = `<h3>${esc(tab.name || "Other talents")} <span class="dim">· ${points(tab.spent)}</span></h3>`;
    if (tab.nodes.length) p.appendChild(tabFrame(tab));
    else p.appendChild(el("div", "empty", "No points spent."));
    row.appendChild(p);
  }
  body.appendChild(row);
}

function timelineSection(body, data) {
  const changes = data.changes.filter((c) => c.kind !== "baseline");
  if (!changes.length) return;
  body.appendChild(el("h3", "section-title", "Order points were taken"));
  const multi = new Set(data.changes.map((c) => c.group)).size > 1;
  const where = (g) => (multi ? ` · group ${g}` : "");
  const entries = [
    ...data.held.map((h) => ({ ms: h.ms, level: h.level, held: h })),
    ...changes.map((c) => ({ ms: c.ms, level: c.level, change: c })),
  ].sort((a, b) => a.ms - b.ms);
  const list = el("ol", "tp");
  let at = null;
  let picks = null;
  for (const e of entries) {
    if (!picks || e.level !== at) {
      at = e.level;
      const step = el("li", "tp-level");
      step.append(el("h4", "tp-lv", at == null ? "Level unknown" : `Level ${at}`));
      picks = el("ul", "tp-picks");
      step.append(picks);
      list.append(step);
    }
    const row = el("li", e.held ? "tp-pick held" : `tp-pick ${e.change.kind}`);
    if (e.held) {
      const h = e.held;
      row.innerHTML = `<span class="tf-chip none"></span><span class="tp-text"><span class="tp-name">${points(h.points)} already spent in ${num(h.nodes)} ${h.nodes === 1 ? "talent" : "talents"} when Scarlet first read group ${h.group}</span><span class="tp-meta">${stampHtml(h.ms, "Read ")}${where(h.group)}</span></span>`;
    } else {
      const c = e.change;
      row.append(chip(plainName(c), c.icon));
      const meta = [KIND[c.kind] ?? esc(c.kind), c.point_no != null ? `point ${c.point_no}` : "", esc(c.tab || ""), stampHtml(c.ms) + where(c.group)].filter(Boolean).join(" · ");
      row.insertAdjacentHTML("beforeend", `<span class="tp-text"><span class="tp-name">${esc(plainName(c))}</span><span class="tp-meta">${meta}</span></span><span class="tp-rank">${c.old_rank ?? ""}→${c.rank}/${c.max_ranks}</span>`);
    }
    picks.append(row);
  }
  body.appendChild(list);
}

function respecSection(body, data) {
  if (!data.respecs.length) return;
  body.appendChild(el("h3", "section-title", "Respecs"));
  const cards = el("div", "rs-cards");
  for (const r of data.respecs) {
    const c = el("div", `rs-card ${r.kind.replace(" ", "-")}`);
    const facts = [r.level == null ? "" : `Level ${r.level}`, `Group ${r.group}`].filter(Boolean).join(" · ");
    c.innerHTML = `<span class="rs-when">${stampHtml(r.ms)}</span><span class="rs-kind">${RESPEC[r.kind] ?? esc(r.kind)}</span><span class="rs-points"><b>${num(r.points)}</b> ${r.points === 1 ? "point" : "points"}</span><span class="rs-facts">${facts}</span><span class="rs-cost">${r.cost_copper == null ? "No cost recorded" : `Cost ${moneyHtml(r.cost_copper)}`}</span>`;
    cards.append(c);
  }
  body.appendChild(cards);
}

export async function render(body, given = null) {
  const ch = given ?? (await shownCharacter());
  if (!ch) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", "No characters recorded yet"));
    return;
  }
  const data = await invoke("talents", { ch });
  body.innerHTML = "";
  if (!data || (!data.build && !data.changes.length)) {
    body.appendChild(el("div", "empty", "No talents recorded yet"));
    return;
  }
  if (data.build) buildSection(body, data.build);
  timelineSection(body, data);
  respecSection(body, data);
  hydrate(body);
  play(body);
}
