// Data: sizes, export, combat-log compression and the watcher switch, then the lines other addons wrote.

import { invoke, el, elText, num, bytes, state, esc, displayName, exactStamp, dayTime, stampHtml, makeSwitch, setSwitch, tile, tiles } from "../lib.js";

let lastStatus = "";
let lastStatusClass = "status";

async function renderStored(body) {
  body.innerHTML = "";

  const actions = el("div", "row");
  const exp = el("button", "btn primary", "Export now");
  exp.type = "button";
  const logs = el("button", "btn", "Compress combat logs");
  logs.type = "button";
  const sw = makeSwitch("Export when WoW saves", false);
  actions.append(exp, logs, sw);
  body.appendChild(actions);

  const status = el("div", lastStatusClass, lastStatus);
  body.appendChild(status);

  const sizes = el("div", "size-row");
  sizes.style.marginTop = "16px";
  body.appendChild(sizes);

  const st = state.store;
  if (st) {
    const open = el("button", "btn", "Open folder");
    open.type = "button";
    open.onclick = () => invoke("open_folder", { path: st.data_dir }).catch((e) => {
      status.className = "status err";
      status.textContent = String(e);
    });
    const folder = el("div", "data-folder");
    folder.append(elText("span", "sc-label", "Data folder"), elText("span", "data-path", st.data_dir), open, elText("span", "dim", `${num(st.events)} ${st.events === 1 ? "event" : "events"} loaded`));
    body.appendChild(folder);
  }

  async function refresh() {
    const s = await invoke("sizes");
    setSwitch(sw, s.watch);
    sizes.innerHTML = "";
    const notes = [`${s.sv_files} ${s.sv_files === 1 ? "file" : "files"} · SavedVariables ${bytes(s.sv_bytes)}`, `Payloads ${bytes(s.payload_bytes)}`, s.ratio ? `${s.ratio.toFixed(1)}× compression` : ""];
    sizes.append(tiles([tile("Stored", bytes(s.sv_bytes + s.payload_bytes), notes, { size: "xl" })], 1));
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

  await refresh();
}

function panel(title) {
  const p = el("div", "panel");
  p.innerHTML = `<h3>${title}</h3>`;
  return p;
}

function summary(view) {
  const p = el("div", "panel");
  const rows = view.keys
    .map(
      (k) => `<tr><td>${esc(k.label)}</td><td class="num">${num(k.lines)}</td>
        <td class="dim gap">${stampHtml(k.first_ms)}</td><td class="dim">${stampHtml(k.last_ms)}</td></tr>`,
    )
    .join("");
  const wrap = el("div", "scroll-x");
  const t = el("table");
  t.innerHTML = `<thead><tr><th>Addon</th><th class="num">Lines</th><th class="gap">First seen</th><th>Last seen</th></tr></thead>
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
        (l) => `<tr><td class="dim" title="${exactStamp(l.ms)}">${dayTime(l.ms)}</td><td>${esc(l.label)}</td>
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
  if (!view.total) return false;
  body.append(el("h3", "section-title", "Lines from other addons"), summary(view), recent(view));
  return true;
}

export async function render(body) {
  body.innerHTML = "";
  const stored = el("div");
  const other = el("div");
  body.append(stored, other);
  await Promise.all([renderStored(stored), renderOther(other)]);
}
