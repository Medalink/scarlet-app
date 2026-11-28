// The class section of Character: the website's page for the selected character's class, drawn in the app's panels.
// The site decodes the uploaded log and owns the derivations of all nine classes, so the app reads
// the finished page over the install's signed request and shows the view only for a character the
// site has a page for.

import { invoke, el, esc, barList, castRows } from "../lib.js";
import { cardList } from "../featcard.js";
import { TIER_NAMES } from "../glyphs.js";
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

async function fetchPage(ch) {
  try {
    const answer = await invoke("class_page_get", { character: ch });
    if (answer.state === "page") return { page: answer.page };
    return { page: null, failed: answer.state === "error" ? answer.message : null, off: answer.state === "off" };
  } catch (e) {
    return { page: null, failed: String(e?.message ?? e) };
  }
}

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
  const tiers = [5, 4, 3, 2, 1].filter((tier) => tier <= 3 || count(tier) > 0);
  const sum = tiers.map((tier) => `<span><b>${count(tier)}</b> ${TIER_NAMES[tier]}</span>`).join("");
  box.innerHTML = `<h3 class="section-title">${esc(page.class_name)} feats</h3><p class="feats-sum"><span><b>${earned}</b> of ${page.feats.length} feats</span>${sum}</p>`;
  box.append(cardList(page.feats.map((f) => ({ ...f, icon: f.glyph, art: f.icon })), true));
  return box;
}

async function spellsPanel(ch) {
  const charts = await invoke("charts", { ch });
  const rows = castRows(charts.spells);
  if (!rows.length) return null;
  const panel = el("div", "panel chart-panel");
  panel.innerHTML = `<h3>Spells cast most</h3>${barList(rows, "#ff2d55")}`;
  return panel;
}

async function draw(body, page, ch) {
  body.innerHTML = "";
  const view = el("div", "cv");
  view.style.setProperty("--class", CLASS_COLORS[page?.class] ?? "var(--accent)");
  if (!page) {
    const spells = await spellsPanel(ch);
    view.append(spells ?? el("div", "empty", "No spells recorded yet"));
    body.append(view);
    return;
  }
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
  const spells = await spellsPanel(ch);
  if (spells) view.append(spells);
  body.append(view);
  hydrate(view);
}

export async function render(body, ch) {
  body.innerHTML = "";
  if (!ch) {
    body.append(el("div", "empty", "No characters recorded yet"));
    return;
  }
  body.append(el("p", "loading", "Loading the class section…"));
  const got = await fetchPage(ch);
  if (!body.isConnected) return;
  if (got.failed) {
    body.innerHTML = "";
    body.append(el("p", "status err", esc(got.failed)));
    return;
  }
  await draw(body, got.page, ch);
}
