// The class view: the website's page for the selected character's class, drawn in the app's panels.
// The site decodes the uploaded log and owns the derivations of all nine classes, so the app reads
// the finished page over the install's signed request and shows the view only for a character the
// site has a page for.

import { invoke, el, esc, state } from "../lib.js";
import { cardList } from "../featcard.js";
import { blocks, hydrate, iconTag, say } from "./classblocks.js";

const CLASS_COLORS = {
  warrior: "#c69b6d",
  paladin: "#f48cba",
  hunter: "#aad372",
  rogue: "#fff468",
  priest: "#f0f1f3",
  shaman: "#0070dd",
  mage: "#3fc7eb",
  warlock: "#8788ee",
  druid: "#ff7c0a",
};

const KNOWN_FOR_MS = 120_000;
let known = null;

/** The character the view is for: the one picked in the header, or the only one there is. */
export function selected() {
  const chars = state.store?.chars ?? [];
  return state.ch ?? (chars.length === 1 ? chars[0] : null);
}

async function fetchPage(ch) {
  try {
    const answer = await invoke("class_page_get", { character: ch });
    if (answer.state === "page") return { page: answer.page };
    return { page: null, failed: answer.state === "error" ? answer.message : null, off: answer.state === "off" };
  } catch (e) {
    return { page: null, failed: String(e?.message ?? e) };
  }
}

/**
 * The class name of the selected character's page, or null when the site has none. The site is asked once for each
 * character in two minutes; an answer that it could not be asked (uploading off) or failed is not kept.
 */
export async function probe() {
  const ch = selected();
  if (!ch) {
    known = null;
    return null;
  }
  if (!known || known.ch !== ch || Date.now() - known.at > KNOWN_FOR_MS) {
    const got = await fetchPage(ch);
    known = got.off || got.failed ? null : { ch, at: Date.now(), ...got };
    return got.page?.class_name ?? null;
  }
  return known.page?.class_name ?? null;
}

export const label = () => known?.page?.class_name ?? "Class";

const spanOf = (module, count) => ({ W5: 5, W7: 7, W8: 8, W12: 12 })[module.span] ?? (count === 1 ? 12 : count === 2 ? 6 : 4);

function module(m, count, tag) {
  const p = el("div", "panel");
  p.style.gridColumn = `span ${spanOf(m, count)}`;
  const heading = `${iconTag(m.icon)}${esc(m.heading)}${tag ? ` <span class="tal">${esc(tag)}</span>` : ""}`;
  p.innerHTML = `<h3>${heading}</h3>${blocks(m.blocks)}${m.note ? `<p class="note">${esc(m.note)}</p>` : ""}`;
  return p;
}

function rows(list) {
  const box = el("div", "cv-rows");
  for (const row of list) {
    const r = el("div", "cv-row");
    for (const m of row.modules) r.append(module(m, row.modules.length));
    box.append(r);
  }
  return box;
}

function banner(page) {
  const line = page.character;
  const b = el("div", "cv-banner");
  const build = line.build ? `<span>${esc(line.build.tree)} ${line.build.points.join(" / ")}</span>` : "";
  b.innerHTML = `<div><div class="ch-name">${esc(page.class_name)}</div><div class="ch-sub">${iconTag(line.icon)}<span>${esc(line.name)}</span><span>Level ${line.level}</span>${build}<span>${esc(line.hours_text)} hours played</span></div></div>`;
  const open = el("button", "btn", "Open on the website");
  open.type = "button";
  open.onclick = () => invoke("class_page_open", { url: page.url }).catch(() => {});
  b.append(open);
  return b;
}

function figures(list) {
  const grid = el("div", "grid cv-figures");
  grid.innerHTML = list
    .map((f) => `<div class="card"><div class="value">${esc(f.text)}${f.of ? `<small>${esc(f.of)}</small>` : ""}</div><div class="label">${iconTag(f.icon)}${esc(f.label)}</div></div>`)
    .join("");
  return grid;
}

function section(title) {
  return el("h3", "section-title", esc(title));
}

function comparison(c, plural) {
  const box = el("div", "cv-compare");
  const bracket = c.from === c.to ? c.from : `${c.from} to ${c.to}`;
  const list = c.standings
    .map((s) => {
      const at = Math.max(0, Math.min(100, s.percent));
      return `<li><span class="rule-l">${iconTag(s.icon)}${esc(s.label)}</span><b class="rule-v">${esc(s.text)}</b><span class="rule" style="--at:${at}%"><i></i></span><span class="rule-t">More than ${at}% of ${esc(plural)}</span></li>`;
    })
    .join("");
  box.innerHTML = `<h3 class="section-title">Against other ${esc(plural)}</h3><p class="note">Public ${esc(plural)} of every ruleset, level ${bracket}. ${say("count", c.characters)} characters.</p><ol class="cv-rules">${list}</ol>`;
  return box;
}

function feats(page) {
  const box = el("div", "cv-feats");
  const earned = page.feats.filter((f) => f.tier >= 1).length;
  const count = (tier) => page.feats.filter((f) => f.tier === tier).length;
  const tiers = [
    [5, "Legend"],
    [4, "Scarlet"],
    [3, "Gold"],
    [2, "Silver"],
    [1, "Copper"],
  ].filter(([tier]) => tier <= 3 || count(tier) > 0);
  const sum = tiers.map(([tier, name]) => `<span><b>${count(tier)}</b> ${name}</span>`).join("");
  box.innerHTML = `<h3 class="section-title">${esc(page.class_name)} feats</h3><p class="feats-sum"><span><b>${earned}</b> of ${page.feats.length} feats</span>${sum}</p>`;
  box.append(cardList(page.feats.map((f) => ({ ...f, icon: f.glyph, art: f.icon })), true));
  return box;
}

function draw(body, page) {
  body.innerHTML = "";
  const view = el("div", "cv");
  view.style.setProperty("--class", CLASS_COLORS[page.class] ?? "var(--accent)");
  view.append(banner(page), figures(page.figures));
  if (page.sheet.length) view.append(rows(page.sheet));
  if (page.talents.length) {
    view.append(section("From the talents"));
    const row = el("div", "cv-rows");
    const r = el("div", "cv-row");
    for (const t of page.talents) r.append(module({ ...t, heading: t.name, span: "Third" }, page.talents.length, `${t.points} of ${t.max}`));
    row.append(r);
    view.append(row);
  }
  if (page.party.length) view.append(section("For the party"), rows(page.party));
  if (page.comparison) view.append(comparison(page.comparison, page.comparison.of));
  if (page.feats.length) view.append(feats(page));
  body.append(view);
  hydrate(view);
}

export async function render(body) {
  body.innerHTML = "";
  body.append(el("p", "loading", "Loading the class page…"));
  const ch = selected();
  const got = ch ? await fetchPage(ch) : { page: null };
  known = ch ? { ch, at: Date.now(), ...got } : null;
  if (!body.isConnected) return;
  if (got.failed) {
    body.innerHTML = "";
    body.append(el("p", "status err", esc(got.failed)));
    return;
  }
  if (!got.page) {
    location.hash = "overview";
    return;
  }
  const entry = document.querySelector('#nav .item[data-key="classpage"] .nav-label');
  if (entry) entry.textContent = label();
  draw(body, got.page);
}
