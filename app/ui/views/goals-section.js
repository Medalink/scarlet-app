// The Character page's goals: how far the level, skills, reputations and Legacy are from their ceilings, the pace of
// the latest run at the cap, and the character's records. Drawn from the `goals` command.

import { el, elText, num, dur, moneyHtml, tiles, goal, record, shortDate } from "../lib.js";
import { paceTiles } from "./paces.js";

export const TIERS = ["", "Hated", "Hostile", "Unfriendly", "Neutral", "Friendly", "Honored", "Revered", "Exalted"];
const REPS_SHOWN = 6;
export const FALLBACK_CAP = 60;
const EXALTED = 8;

function mod(cls, title, ...kids) {
  const s = el("section", `mod ${cls}`);
  s.append(elText("h2", "", title), ...kids);
  return s;
}

const label = (text) => elText("p", "sc-label", text);

function goalList(goals) {
  const list = el("div", "goal-list");
  goals.forEach((g, n) => {
    g.querySelector(".sg-t > i").style.setProperty("--n", String(n));
    list.append(g);
  });
  return list;
}

function etaList(eta) {
  const box = el("div", "eta");
  if (eta.targets.length) {
    const ul = el("ul");
    for (const t of eta.targets) {
      const li = el("li");
      li.append(elText("span", "eta-lv", `Level ${t.level}`), elText("b", "", t.time), elText("span", "", t.date));
      ul.append(li);
    }
    box.append(ul);
  }
  if (eta.basis) box.append(elText("p", "eta-basis", eta.basis));
  return box;
}

function levelMod(ov, eta, cap, atCap) {
  const box = el("div", "goal-stack");
  const last = eta.targets[eta.targets.length - 1];
  box.append(goal(ov.level, cap, { size: "lg", eta: !atCap && last ? `Level ${last.level} in ${last.time}` : "" }));
  if (!atCap) {
    if (ov.xpMax > 0) box.append(goal(ov.xp, ov.xpMax, { label: `To level ${ov.level + 1}`, eta: eta.targets[0]?.time ?? "" }));
    else box.append(elText("p", "sc-note", "No experience bar recorded"));
    if (eta.targets.length || eta.basis) box.append(etaList(eta));
  }
  return mod("w5 first", "Level", box);
}

function skillGoals(skills) {
  const left = (s) => s.max - s.rank;
  const ordered = [...skills].sort((a, b) => (left(a) > 0) !== (left(b) > 0) ? (left(b) > 0) - (left(a) > 0) : left(a) - left(b) || a.name.localeCompare(b.name));
  return goalList(ordered.map((s) => goal(s.rank, s.max, { label: s.name, count: false })));
}

function skillsMod(skills) {
  const kids = [];
  for (const [kind, title] of [["profession", "Professions"], ["weapon", "Weapons and defense"]]) {
    const of = skills.filter((s) => s.kind === kind);
    if (of.length) kids.push(label(title), skillGoals(of));
  }
  return mod("w7 last", "Skills", ...(kids.length ? kids : [elText("p", "sc-empty", "No skills recorded")]));
}

function repsMod(reps, cls) {
  if (!reps.length) return mod(cls, "Reputation", elText("p", "sc-empty", "No reputation recorded"));
  const byTier = new Map();
  for (const r of reps) byTier.set(r.tier, (byTier.get(r.tier) ?? 0) + 1);
  const summary = [...byTier].sort((a, b) => b[0] - a[0]).map(([tier, n]) => `${TIERS[tier]} ${n}`).join(" · ");
  const body = el("div");
  let all = false;
  const draw = () => {
    const shown = all ? reps : reps.filter((r) => r.tier < EXALTED).slice(0, REPS_SHOWN);
    const bars = shown.map((r) => goal(r.standing - r.from, r.to - r.from, { label: r.name, note: TIERS[r.tier], count: false }));
    body.replaceChildren(bars.length ? goalList(bars) : elText("p", "sc-note", "All exalted"));
    if (all || shown.length < reps.length) {
      const more = elText("button", "linkbtn", all ? "Show the closest to exalted" : `Show all ${reps.length}`);
      more.type = "button";
      more.onclick = () => {
        all = !all;
        draw();
      };
      body.append(more);
    }
  };
  draw();
  return mod(cls, "Reputation", elText("p", "sc-sub", summary), body);
}

function legacyMod(legacy) {
  const spent = legacy.trees.map((t) => `${t.name} ${t.spent}`).join(" · ");
  const kids = [goal(legacy.earned, legacy.max, { label: "Legacy points earned" }), elText("p", "sc-sub", `Spent: ${spent}`)];
  if (legacy.nodes.length) kids.push(label("Ranks bought"), goalList(legacy.nodes.map((n) => goal(n.rank, n.max, { label: `${n.tree}: ${n.name}`, count: false }))));
  return mod("w5 last", "Legacy", ...kids);
}

const fixed = (v) => (v >= 10 ? num(v) : String(Math.round(v * 10) / 10));

export const RECORDS = [
  ["xp_h", "Most XP an hour", num],
  ["gold_h", "Most gold an hour", moneyHtml],
  ["kills_h", "Most kills an hour", fixed],
  ["gathers_h", "Most gathers an hour", fixed],
  ["xp", "Most XP in a session", num],
  ["coin", "Most gold looted", moneyHtml],
  ["kills", "Most kills in a session", num],
  ["gathers", "Most gathers in a session", num],
  ["item", "Most of one item", num],
  ["played", "Longest session", dur],
  ["balance", "Highest balance", moneyHtml],
];

/**
 * The record tiles for `records`, in the order of RECORDS, null when there are none. `open(r)` gives the handler that
 * runs when a tile's run is opened; `owner(r)` the character's name shown in a tile's note.
 */
export function recordTiles(records, { open, owner = null }) {
  const list = [];
  for (const [key, name, show] of RECORDS) {
    const r = records.find((x) => x.key === key);
    if (!r) continue;
    const when = key === "balance" ? shortDate(r.at) : `${shortDate(r.at)} · level ${r.level}`;
    const note = [owner?.(r), r.item?.name].filter(Boolean).join(" · ");
    list.push(record(name, show(r.value), when, r.span ? `#sessions/${r.span[0]}-${r.span[1]}` : "", { count: [r.value, show], note, onOpen: open(r) }));
  }
  const cols = Math.min(4, list.length);
  const loose = cols ? list.length % cols : 0;
  if (loose) list[list.length - 1].style.gridColumn = `span ${cols - loose + 1}`;
  return list.length ? tiles(list, cols) : null;
}

function recordsMod(records, onOpen) {
  const body = recordTiles(records, { open: () => onOpen }) ?? elText("p", "sc-empty", "No session of 15 minutes recorded yet");
  return mod("w12 first last", "Records", body);
}

/**
 * The sheet that leads the Character page. `ov` and `eta` are the overview's, `g` the goals. At the level cap the
 * level has nothing left to estimate, so the latest run's pace and the records come first.
 */
export function goalsSheet(g, ov, eta, { atCap, onOpen }) {
  const sheet = el("div", "sheet goals-sheet");
  const cap = g.cap || ov.cap?.level || FALLBACK_CAP;
  const latest = atCap && g.latest ? paceTiles(g.latest.paces, g.latest.played, { onOpen }) : null;
  const records = recordsMod(g.records, onOpen);
  const rows = [];
  if (latest) rows.push(mod("w12 first last", `Latest run, ${shortDate(g.latest.from)}`, latest));
  if (atCap) rows.push(records);
  rows.push(levelMod(ov, eta, cap, atCap), skillsMod(g.caps.skills), repsMod(g.caps.reps, g.caps.legacy ? "w7 first" : "w12 first last"));
  if (g.caps.legacy) rows.push(legacyMod(g.caps.legacy));
  if (!atCap) rows.push(records);
  sheet.append(...rows);
  return sheet;
}
