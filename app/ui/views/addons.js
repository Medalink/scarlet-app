// Addons: lines Scarlet recorded from other installed addons. They come from a
// separate stream and are not part of any other view's numbers.

import { invoke, el, state, esc, displayName, num, dateTime } from "../lib.js";

function panel(title) {
  const p = el("div", "panel");
  p.innerHTML = `<h3>${title}</h3>`;
  return p;
}

function summary(view) {
  const p = panel("Addons recorded");
  const rows = view.keys
    .map(
      (k) => `<tr><td>${esc(k.label)}</td><td class="num">${num(k.lines)}</td>
        <td class="dim">${dateTime(k.first_ms)}</td><td class="dim">${dateTime(k.last_ms)}</td></tr>`,
    )
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Addon</th><th class="num">Lines</th><th>First seen</th><th>Last seen</th></tr></thead>
    <tbody>${rows}</tbody>`;
  wrap.appendChild(t);
  p.appendChild(wrap);
  return p;
}

function recent(view) {
  const p = panel("Recent lines");
  const pick = el("select");
  pick.setAttribute("aria-label", "Filter by addon");
  pick.innerHTML = `<option value="">All addons</option>${view.keys
    .map((k) => `<option value="${esc(k.key)}">${esc(k.label)}</option>`)
    .join("")}`;
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Time</th><th>Addon</th><th>Character</th><th>Line</th></tr></thead><tbody></tbody>`;
  const tb = t.querySelector("tbody");
  const note = el("div", "dim");
  const draw = () => {
    const hit = view.lines.filter((l) => !pick.value || l.key === pick.value);
    tb.innerHTML = hit
      .map(
        (l) => `<tr><td class="dim">${dateTime(l.ms)}</td><td>${esc(l.label)}</td>
          <td class="dim">${esc(displayName(l.character))}</td><td>${esc(l.text)}</td></tr>`,
      )
      .join("");
    note.textContent =
      view.total > view.lines.length ? `Showing the newest ${num(view.lines.length)} of ${num(view.total)}.` : "";
  };
  pick.onchange = draw;
  draw();
  wrap.appendChild(t);
  p.append(pick, wrap, note);
  return p;
}

export async function render(body) {
  const view = await invoke("addons", { ch: state.ch });
  body.innerHTML = "";
  if (!view.total) {
    const p = panel("Addons recorded");
    p.appendChild(el("div", "empty", "No lines from other addons recorded yet."));
    body.appendChild(p);
    return;
  }
  body.appendChild(summary(view));
  body.appendChild(recent(view));
}
