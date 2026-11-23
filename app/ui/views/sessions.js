// Sessions: one card for each S..ES session, newest first. A card opens the recap of its run.

import { invoke, el, elText, state, sessionCard, wireTips } from "../lib.js";
import { renderRecaps } from "./recap.js";
import { linkPrompt } from "../link.js";

const PAGE = 40;

export async function render(body, sub) {
  if (/^\d+-\d+$/.test(sub ?? "")) return renderRun(body, sub);
  wireTips();
  const cards = (await invoke("session_cards", { ch: state.ch })).filter((c) => c.recap).reverse();
  body.innerHTML = "";
  if (!cards.length) {
    body.appendChild(el("div", "empty", "No sessions yet."));
    return;
  }
  const list = el("div", "sc-sessions");
  const more = elText("button", "sc-btn quiet small", "");
  more.type = "button";
  let shown = 0;
  const draw = () => {
    for (const c of cards.slice(shown, shown + PAGE)) {
      list.append(sessionCard(c.recap, { who: !state.ch, onOpen: () => window.__nav(`sessions/${c.from}-${c.to}`) }));
    }
    shown = Math.min(cards.length, shown + PAGE);
    more.textContent = `Show more (${cards.length - shown} left)`;
    if (shown >= cards.length) more.remove();
  };
  more.onclick = draw;
  draw();
  body.append(list);
  if (shown < cards.length) body.append(more);
}

async function renderRun(body, sub) {
  const [from, to] = sub.split("-").map(Number);
  const run = await invoke("run_summary", { from, to });
  if (!run.recaps.length) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", "Nothing was recorded while WoW ran."));
    return;
  }
  await renderRecaps(body, run);
  const row = await linkPrompt("recap");
  if (row && body.isConnected) body.append(row);
}
