// Feats: the website's count of each character's feats, and the toasts for new and upgraded ones.

import { invoke, el, state, displayName } from "../lib.js";
import { cardList, summary } from "../featcard.js";
import { playToasts, stopToasts } from "../feat-toast.js";

let expandNext = false;

function allFeats(entry) {
  const box = el("div", "feats-all");
  for (const [title, cards] of [
    ["Earned", entry.earned],
    ["Not yet earned", entry.unearned],
  ]) {
    if (!cards.length) continue;
    const h = el("h3", "section-title");
    h.append(document.createTextNode(`${title} `));
    const count = el("span");
    count.textContent = String(cards.length);
    h.append(count);
    box.append(h, cardList(cards));
  }
  return box;
}

function characterBlock(entry, single) {
  const block = el("section", "feats-block");
  if (!single) {
    const h = el("h3", "section-title");
    h.textContent = displayName(entry.name);
    block.append(h);
  }
  block.append(summary(entry));
  if (entry.since) {
    const note = el("p", "note");
    note.textContent = `Counted since ${entry.since}.`;
    block.append(note);
  }
  if (entry.pinned.length) block.append(cardList(entry.pinned, true));
  block.append(cardList(entry.top, true));
  const reveal = () => {
    if (block.querySelector(".feats-all")) return;
    button?.remove();
    block.append(allFeats(entry));
  };
  let button = null;
  if (single) {
    button = el("button", "btn", "View all");
    button.type = "button";
    button.onclick = reveal;
    block.append(button);
  }
  return { block, reveal };
}

function drawOff(box) {
  const text = el("p", "note");
  text.textContent = "Feats are counted on the website from what you upload.";
  const go = el("button", "btn", "Website settings");
  go.type = "button";
  go.onclick = () => (location.hash = "settings/website");
  box.append(text, go);
}

function drawCharacters(box, characters) {
  const wanted = state.ch;
  const entries = wanted ? characters.filter((c) => c.name === wanted) : characters;
  if (!entries.length) {
    const none = el("p", "empty");
    none.textContent = wanted ? "No feats counted for this character yet." : "No feats counted yet.";
    box.append(none);
    return;
  }
  const single = Boolean(wanted);
  const views = entries.map((entry) => characterBlock(entry, single));
  for (const { block } of views) box.append(block);
  if (single && expandNext) views[0].reveal();
  expandNext = false;

  const at = entries.findIndex((entry) => entry.new?.length);
  if (at < 0) return;
  const entry = entries[at];
  playToasts(entry.name, entry.new, () => {
    if (single) return views[at].reveal();
    expandNext = true;
    const select = document.querySelector("#char");
    select.value = entry.name;
    select.dispatchEvent(new Event("change"));
  });
}

async function load(box) {
  let answer;
  try {
    answer = await invoke("feats_get");
  } catch (e) {
    answer = { state: "error", message: String(e) };
  }
  if (!box.isConnected) return;
  box.innerHTML = "";
  if (answer.state === "off") return drawOff(box);
  if (answer.state === "error") {
    const err = el("p", "status err");
    err.textContent = answer.message;
    box.append(err);
    return;
  }
  drawCharacters(box, answer.characters);
}

export async function render(body) {
  stopToasts();
  body.innerHTML = "";
  const box = el("div", "feats-view");
  body.append(box);
  load(box).catch((e) => {
    box.innerHTML = "";
    const err = el("p", "status err");
    err.textContent = String(e?.message ?? e);
    box.append(err);
  });
}
