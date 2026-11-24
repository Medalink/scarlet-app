// Sessions: one card for each session of the chosen kind, newest first. A card opens the recap of its time range.

import { invoke, el, elText, state, sessionCard, wireTips } from "../lib.js";
import { renderRecaps } from "./recap.js";
import { linkPrompt } from "../link.js";

const PAGE = 40;

const MODES = [
  ["auto", "WoW runs"],
  ["day", "Days"],
  ["manual", "Manual"],
];

const clock = (ms) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
const day = (ms) => new Date(ms).toLocaleDateString([], { weekday: "long", day: "numeric", month: "long" });

const button = (cls, text, onClick) => {
  const b = elText("button", cls, text);
  b.type = "button";
  b.onclick = onClick;
  return b;
};

function liveCard(c, onOpen) {
  const card = el("a", "sc-session");
  card.href = "#";
  card.onclick = (e) => {
    e.preventDefault();
    onOpen();
  };
  card.append(elText("p", "sc-session-d", `${day(c.from)} · ${clock(c.from)} to now`), elText("h3", "sc-session-t", "In progress"), elText("p", "sc-session-n", "Nothing recorded yet."));
  return card;
}

let drawn = 0;
let changes = Promise.resolve();

const change = (command, args) => {
  const run = changes.then(() => invoke(command, args));
  changes = run.catch(() => {});
  return run;
};

function controls(prefs, redraw) {
  const bar = el("div", "sc-modes");
  const seg = el("div", "sc-seg");
  for (const [mode, name] of MODES) {
    seg.append(
      button(mode === prefs.mode ? "on" : "", name, async () => {
        if (mode === prefs.mode) return;
        await change("set_session_mode", { mode });
        redraw();
      }),
    );
  }
  bar.append(seg);
  if (prefs.mode !== "manual") return bar;
  const open = prefs.marks.find((m) => m.to == null);
  if (open) {
    bar.append(
      button("sc-btn", "Stop session", async () => {
        await change("session_stop");
        redraw();
      }),
      elText("span", "sc-sub", `Started ${clock(open.from)}`),
    );
  } else {
    bar.append(
      button("sc-btn", "Start session", async () => {
        await change("session_start");
        redraw();
      }),
    );
  }
  return bar;
}

async function draw(body) {
  const mine = ++drawn;
  const hash = location.hash;
  const { prefs, cards } = await invoke("session_cards", { ch: state.ch });
  if (mine !== drawn || hash !== location.hash || !body.isConnected) return;
  const shown = cards.filter((c) => c.recap || c.live);
  body.innerHTML = "";
  body.append(controls(prefs, () => draw(body)));
  if (!shown.length) {
    body.appendChild(el("div", "empty", prefs.mode === "manual" ? "Press Start session, play, then press Stop session." : "No sessions yet."));
    return;
  }
  const list = el("div", "sc-sessions");
  const more = elText("button", "sc-btn quiet small", "");
  more.type = "button";
  let count = 0;
  const add = () => {
    for (const c of shown.slice(count, count + PAGE)) {
      const onOpen = () => window.__nav(`sessions/${c.from}-${c.to}`);
      list.append(c.recap ? sessionCard(c.recap, { who: !state.ch, others: c.chars.slice(1), live: c.live, onOpen }) : liveCard(c, onOpen));
    }
    count = Math.min(shown.length, count + PAGE);
    more.textContent = `Show more (${shown.length - count} left)`;
    if (count >= shown.length) more.remove();
  };
  more.onclick = add;
  add();
  body.append(list);
  if (count < shown.length) body.append(more);
}

export async function render(body, sub) {
  if (/^\d+-\d+$/.test(sub ?? "")) return renderRun(body, sub);
  wireTips();
  await draw(body);
}

const overlap = (c, from, to) => Math.min(c.to, to) - Math.max(c.from, from);

async function renderRun(body, sub) {
  const [from, to] = sub.split("-").map(Number);
  const { prefs, sessions: cards } = await invoke("session_list", { ch: state.ch });
  const exact = cards.find((c) => c.from === from);
  const current = exact ?? cards.filter((c) => overlap(c, from, to) > 0).sort((a, b) => overlap(b, from, to) - overlap(a, from, to))[0] ?? null;
  const run = exact ? await invoke("run_summary", { from: exact.from, to: exact.to, ch: state.ch, clip: true }) : await invoke("run_summary", { from, to });
  if (!run.recaps.length) {
    body.innerHTML = "";
    body.appendChild(el("div", "empty", exact?.live ? "Nothing recorded yet." : "Nothing was recorded while WoW ran."));
    return;
  }
  await renderRecaps(body, run, { cards: current ? cards : [], current, mode: prefs.mode });
  const row = await linkPrompt("recap");
  if (row && body.isConnected) body.append(row);
}
