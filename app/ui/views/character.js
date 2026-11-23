// Character: the overview of one character, with Class and Talents as sections of the same page.

import { invoke, el, elText, state, sessionCard } from "../lib.js";
import { cardList } from "../featcard.js";
import * as overview from "./overview.js";
import * as classSection from "./class-section.js";
import * as talentsSection from "./talents-section.js";
import { closestFor } from "./feats.js";
import { linkPrompt } from "../link.js";

const SECTIONS = [
  ["overview", "Overview"],
  ["class", "Class"],
  ["talents", "Talents"],
];

async function who() {
  if (state.ch) return state.ch;
  const chars = state.store?.chars ?? [];
  if (chars.length === 1) return chars[0];
  return (await invoke("overview", { ch: null }))?.name ?? null;
}

async function extras(name) {
  const [near, cards] = await Promise.all([closestFor(name), invoke("session_cards", { ch: name, limit: 1 })]);
  const latest = cards.filter((c) => c.recap).at(-1);
  if (!near.length && !latest) return null;
  const box = el("div", "char-extra");
  if (near.length) box.append(elText("h2", "sc-h", "Feats within reach"), cardList(near, true));
  if (latest) {
    box.append(
      elText("h2", "sc-h", "Latest session"),
      sessionCard(latest.recap, { who: false, onOpen: () => window.__nav(`sessions/${latest.from}-${latest.to}`) }),
    );
  }
  return box;
}

async function drawOverview(page) {
  await overview.render(page, state.ch);
  const name = overview.shownName();
  if (!name || !page.isConnected) return;
  const box = await extras(name);
  if (!box || !page.isConnected) return;
  const week = page.querySelector(".week");
  if (week) week.after(box);
  else page.prepend(box);
}

export async function render(body, sub) {
  const key = SECTIONS.some(([k]) => k === sub) ? sub : "overview";
  body.innerHTML = "";
  const tabs = el("div", "subnav");
  tabs.setAttribute("role", "tablist");
  for (const [k, name] of SECTIONS) {
    const a = el("a", "", name);
    a.href = k === "overview" ? "#character" : `#character/${k}`;
    a.setAttribute("role", "tab");
    a.setAttribute("aria-selected", String(k === key));
    tabs.append(a);
  }
  const page = el("div");
  body.append(tabs, page);
  if (key === "overview") return drawOverview(page);
  const name = await who();
  if (key === "class") {
    await classSection.render(page, name);
    const row = await linkPrompt("class");
    if (row && page.isConnected) page.append(row);
    return;
  }
  return talentsSection.render(page, name);
}
