// Compress: run the companion export and combat-log compression, toggle the
// watch thread, show sizes.

import { invoke, el, num, bytes, state, esc } from "../lib.js";

// Survives the store-changed re-render that an export itself triggers.
let lastStatus = "";

export async function render(body) {
  body.innerHTML = "";

  const actions = el("div", "row");
  const exp = el("button", "btn primary", "Export now");
  const logs = el("button", "btn", "Compress combat logs");
  const sw = el("span", "switch");
  sw.innerHTML = `<span class="track"><span class="knob"></span></span><span class="lab">Watch SavedVariables</span>`;
  actions.append(exp, logs, sw);
  body.appendChild(actions);

  const status = el("div", "status", lastStatus);
  body.appendChild(status);

  const sizes = el("div", "size-row");
  sizes.style.marginTop = "16px";
  body.appendChild(sizes);

  async function refresh() {
    const s = await invoke("sizes");
    sw.classList.toggle("on", s.watch);
    sizes.innerHTML = "";
    const cards = [
      ["SavedVariables", `${s.sv_files} file(s) · ${bytes(s.sv_bytes)}`],
      ["Payloads", bytes(s.payload_bytes)],
      ["Compression ratio", s.ratio ? `${s.ratio.toFixed(1)}×` : "—"],
      ["Watch", s.watch ? "Running" : "Off"],
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
      status.className = "status ok";
    } catch (e) {
      lastStatus = String(e);
      status.className = "status err";
    }
    status.textContent = lastStatus;
    refresh();
  };
  logs.onclick = async () => {
    status.className = "status";
    status.textContent = "Compressing combat logs…";
    try {
      lastStatus = await invoke("compress_logs");
      status.className = "status ok";
    } catch (e) {
      lastStatus = String(e);
      status.className = "status err";
    }
    status.textContent = lastStatus;
    refresh();
  };
  sw.onclick = async () => {
    const on = await invoke("toggle_watch", { on: !sw.classList.contains("on") });
    sw.classList.toggle("on", on);
    refresh();
  };

  const st = state.store;
  if (st) {
    body.insertAdjacentHTML(
      "beforeend",
      `<div class="note">Data folder ${esc(st.data_dir)} · ${num(st.events)} events loaded. The watch thread exports again whenever a Scarlet.lua file changes.</div>`
    );
  }

  await refresh();
}
