// Data: sizes, export, combat-log compression and the watcher switch, then the lines other addons wrote.

import { invoke, el, num, bytes, state, esc, displayName, dateTime, makeSwitch, setSwitch } from "../lib.js";

let lastStatus = "";
let lastStatusClass = "status";

async function renderStored(body) {
  body.innerHTML = "";

  const actions = el("div", "row");
  const exp = el("button", "btn primary", "Export now");
  exp.type = "button";
  const logs = el("button", "btn", "Compress combat logs");
  logs.type = "button";
  const sw = makeSwitch("Watch SavedVariables", false);
  actions.append(exp, logs, sw);
  body.appendChild(actions);

  const status = el("div", lastStatusClass, lastStatus);
  body.appendChild(status);

  const sizes = el("div", "size-row");
  sizes.style.marginTop = "16px";
  body.appendChild(sizes);

  async function refresh() {
    const s = await invoke("sizes");
    setSwitch(sw, s.watch);
    sizes.innerHTML = "";
    const cards = [
      ["SavedVariables", `${s.sv_files} ${s.sv_files === 1 ? "file" : "files"} · ${bytes(s.sv_bytes)}`],
      ["Payloads", bytes(s.payload_bytes)],
      ["Compression ratio", s.ratio ? `${s.ratio.toFixed(1)}×` : "—"],
      ["Auto-export", s.watch ? "On" : "Off"],
    ];
    for (const [label, value] of cards) {
      sizes.insertAdjacentHTML(
        "beforeend",
        `<div class="card"><div class="value">${value}</div><div class="label">${label}</div></div>`
      );
    }
  }

  exp.onclick = async () => {
    status.className = "status";
    status.textContent = "Exporting…";
    try {
      lastStatus = await invoke("compress_export");
      lastStatusClass = "status ok";
    } catch (e) {
      lastStatus = String(e);
      lastStatusClass = "status err";
    }
    status.className = lastStatusClass;
    status.textContent = lastStatus;
    refresh();
  };
  logs.onclick = async () => {
    status.className = "status";
    status.textContent = "Compressing combat logs…";
    try {
      lastStatus = await invoke("compress_logs");
      lastStatusClass = "status ok";
    } catch (e) {
      lastStatus = String(e);
      lastStatusClass = "status err";
    }
    status.className = lastStatusClass;
    status.textContent = lastStatus;
    refresh();
  };
  sw.onclick = async () => {
    try {
      const on = await invoke("toggle_watch", { on: !sw.classList.contains("on") });
      setSwitch(sw, on);
      refresh();
    } catch (e) {
      status.className = "status err";
      status.textContent = String(e);
    }
  };

  const st = state.store;
  if (st) {
    body.insertAdjacentHTML(
      "beforeend",
      `<div class="note">Data folder ${esc(st.data_dir)} · ${num(st.events)} ${st.events === 1 ? "event" : "events"} loaded. When watching is on, Scarlet exports automatically whenever WoW saves.</div>`
    );
  }

  await refresh();
}

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

async function renderOther(body) {
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

export async function render(body) {
  body.innerHTML = "";
  const stored = el("div");
  const other = el("div");
  body.append(stored, el("h3", "section-title", "Lines from other addons"), other);
  await Promise.all([renderStored(stored), renderOther(other)]);
}
