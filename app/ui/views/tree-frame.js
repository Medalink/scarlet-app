// The talent frame: square icon slots on a grid with arrows between them, the shape of the in-game talent window.
// Class trees and the Legacy trees both draw through it. Icons arrive after the slots are drawn and never block them:
// the page calls `hydrate` from classblocks.js once its elements are in the document.

import { el, esc } from "../lib.js";

const SLOT = 48;
const GAP = 24;
const PITCH = SLOT + GAP;
const HEAD = 7;
const LOCK = `<svg class="tf-lock" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><rect x="3.5" y="7" width="9" height="6.5" rx="1" fill="currentColor"/><path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>`;

function icon(id) {
  const i = el("i", "tf-ico");
  i.dataset.icon = iconUrl(id);
  return i;
}

export const iconUrl = (id) => (id ? `https://site/spell-icons/${id}.png` : "");

export const initial = (name) => (String(name).match(/[\p{L}\p{N}]/u)?.[0] ?? "?").toUpperCase();

export function stateOf(node) {
  if (node.locked) return "locked";
  if (node.rank <= 0) return "untaken";
  return node.max > 0 && node.rank >= node.max ? "maxed" : "partial";
}

function describe(node) {
  if (node.locked) return `${node.name}, not in the game yet`;
  const choices = node.choices?.length > 1 ? `. Choice of ${node.choices.map((c) => c.name || `Spell ${c.spell_id}`).join(" or ")}` : "";
  return `${node.name}, rank ${node.rank} of ${node.max}${choices}`;
}

export function slot(node) {
  const state = stateOf(node);
  const s = el("div", `tf-slot ${state}${node.choices?.length > 1 ? " choice" : ""}`);
  s.dataset.node = node.id;
  s.dataset.state = state;
  const text = describe(node);
  s.title = text;
  s.setAttribute("role", "img");
  s.setAttribute("aria-label", text);
  s.append(el("span", "tf-init", state === "locked" ? LOCK : esc(initial(node.name))));
  if (node.icon && state !== "locked") s.append(icon(node.icon));
  if (state !== "locked" && (node.max > 1 || node.rank > 0)) s.append(el("b", "tf-rank", `${node.rank}/${node.max}`));
  return s;
}

export const chip = (name, id) => {
  const c = el("span", "tf-chip", `<span class="tf-init">${esc(initial(name))}</span>`);
  if (id) c.append(icon(id));
  return c;
};

function arrow(from, to, lit) {
  const ax = from.col * PITCH + SLOT / 2;
  const ay = from.row * PITCH + SLOT / 2;
  const bx = to.col * PITCH + SLOT / 2;
  const by = to.row * PITCH + SLOT / 2;
  const dx = bx - ax;
  const dy = by - ay;
  const reach = Math.max(Math.abs(dx), Math.abs(dy));
  if (!reach) return "";
  const trim = (SLOT / 2 + 3) / reach;
  const x1 = ax + dx * trim;
  const y1 = ay + dy * trim;
  const x2 = bx - dx * trim;
  const y2 = by - dy * trim;
  const len = Math.hypot(x2 - x1, y2 - y1);
  const ux = (x2 - x1) / len;
  const uy = (y2 - y1) / len;
  const hx = x2 - ux * HEAD;
  const hy = y2 - uy * HEAD;
  const head = `${x2},${y2} ${hx - uy * HEAD * 0.55},${hy + ux * HEAD * 0.55} ${hx + uy * HEAD * 0.55},${hy - ux * HEAD * 0.55}`;
  return `<g class="tf-arrow${lit ? " lit" : ""}" data-from="${from.id}" data-to="${to.id}"><line x1="${x1}" y1="${y1}" x2="${hx}" y2="${hy}"/><polygon points="${head}"/></g>`;
}

/**
 * Draws `nodes` (`{ id, col, row, name, icon, rank, max, locked, choices }`) on a grid at least `cols` by `rows`, and an
 * arrow for each `links` pair `[from, to]` of node ids, lit when the source holds a rank. A node with no cell goes on
 * the rows under the grid.
 */
export function treeFrame(nodes, links, { cols = 4, rows = 0 } = {}) {
  const placed = nodes.filter((n) => n.col != null && n.row != null);
  const loose = nodes.filter((n) => n.col == null || n.row == null);
  let width = Math.max(cols, ...placed.map((n) => n.col + 1));
  let height = Math.max(rows, ...placed.map((n) => n.row + 1));
  const seen = new Set(placed.map((n) => `${n.col},${n.row}`));
  const all = [...placed];
  let at = 0;
  for (const n of loose) {
    while (seen.has(`${at % width},${height + Math.floor(at / width)}`)) at += 1;
    all.push({ ...n, col: at % width, row: height + Math.floor(at / width) });
    at += 1;
  }
  if (loose.length) height += Math.ceil((at || 1) / width);
  const frame = el("div", "tf-frame");
  frame.style.setProperty("--slot", `${SLOT}px`);
  frame.style.setProperty("--gap", `${GAP}px`);
  frame.style.width = `${width * PITCH - GAP}px`;
  frame.style.height = `${height * PITCH - GAP}px`;
  const byId = new Map(all.map((n) => [String(n.id), n]));
  const drawn = links
    .map(([from, to]) => [byId.get(String(from)), byId.get(String(to))])
    .filter(([a, b]) => a && b)
    .map(([a, b]) => arrow(a, b, a.rank > 0))
    .join("");
  const lines = el("div", "tf-lines");
  lines.innerHTML = `<svg width="${width * PITCH - GAP}" height="${height * PITCH - GAP}" aria-hidden="true">${drawn}</svg>`;
  frame.append(lines);
  for (const n of all) {
    const s = slot(n);
    s.style.left = `${n.col * PITCH}px`;
    s.style.top = `${n.row * PITCH}px`;
    frame.append(s);
  }
  return frame;
}
