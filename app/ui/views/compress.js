// Compress: run the companion export and combat-log compression, toggle the
// watch thread, show sizes.

import { invoke, el, num, bytes, state, esc, makeSwitch, setSwitch } from "../lib.js";

// Survives the store-changed re-render that an export itself triggers.
let lastStatus = "";
let lastStatusClass = "status";

export async function render(body) {
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
