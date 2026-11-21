// Stats: stat cards by section, the movement breakdown and the
// either-or time splits (combat, group, resting, AFK).

import { invoke, el, esc, stackedBar, dur, state } from "../lib.js";

export async function render(body) {
  body.innerHTML = "";
  const data = await invoke("stats", { ch: state.ch });

  for (const s of data.sections) {
    body.appendChild(el("h3", "section-title", esc(s.title)));
    const grid = el("div", "grid");
    for (const [label, value] of s.cards) {
      const c = el("div", "card");
      c.innerHTML = `<div class="value">${esc(value)}</div><div class="label">${esc(label)}</div>`;
      grid.appendChild(c);
    }
    body.appendChild(grid);
  }

  const p = el("div", "panel");
  p.style.marginTop = "16px";
  p.innerHTML = `<h3>Moving and standing</h3>`;
  if (data.time.length) {
    p.insertAdjacentHTML("beforeend", stackedBar(data.time.map((t) => [t.label, t.seconds])));
  } else {
    p.insertAdjacentHTML("beforeend", `<div class="empty">No recorded time yet.</div>`);
  }
  body.appendChild(p);

  if (data.splits.length) {
    const q = el("div", "panel");
    q.style.marginTop = "12px";
    q.innerHTML = `<h3>How the time was spent</h3>`;
    for (const [a, b] of data.splits) {
      const total = a.seconds + b.seconds || 1;
      const pct = Math.round((a.seconds / total) * 100);
      q.insertAdjacentHTML(
        "beforeend",
        `<div class="split">
          <div class="split-head"><span>${esc(a.label)} ${dur(a.seconds)}</span><span>${esc(b.label)} ${dur(b.seconds)}</span></div>
          <div class="split-track" title="${pct}%"><div style="width:${pct}%"></div></div>
        </div>`
      );
    }
    body.appendChild(q);
  }
}
