// Settings: WoW folder and data folder, what is missing on this PC, and where they are stored.

import { invoke, el } from "../lib.js";

export async function render(body) {
  body.innerHTML = "";
  const s = await invoke("get_settings");

  const form = el("div");
  form.innerHTML = `
    <div class="status err" id="missing"></div>
    <div class="field"><label>WoW folder</label></div>
    <div class="field"><label>Data folder</label></div>
    <button class="btn primary" id="save">Save</button>
    <div class="status" id="st"></div>
    <div class="note" id="where"></div>`;
  const missing = form.querySelector("#missing");
  for (const p of s.problems) {
    const line = el("div");
    line.textContent = p;
    missing.appendChild(line);
  }
  missing.hidden = s.problems.length === 0;
  form.querySelector("#where").textContent = s.keep_off_c
    ? `Settings are in ${s.file}. The data folder must not be on C:.`
    : `Settings are in ${s.file}.`;
  const [wow, data] = form.querySelectorAll(".field");
  const wowIn = el("input");
  wowIn.value = s.wow;
  const dataIn = el("input");
  dataIn.value = s.data;
  wow.appendChild(wowIn);
  data.appendChild(dataIn);
  body.appendChild(form);

  const sw = el("span", "switch");
  sw.innerHTML = `<span class="track"><span class="knob"></span></span><span class="lab">Open when WoW Forever starts</span>`;
  sw.classList.toggle("on", !!s.open_with_wow);
  sw.style.marginTop = "16px";
  const swSt = el("div", "status");
  sw.onclick = async () => {
    try {
      const on = await invoke("set_open_with_wow", { on: !sw.classList.contains("on") });
      sw.classList.toggle("on", on);
      swSt.className = "status";
      swSt.textContent = "";
    } catch (e) {
      swSt.className = "status err";
      swSt.textContent = String(e);
    }
  };
  body.append(sw, swSt);

  form.querySelector("#save").onclick = async () => {
    const st = form.querySelector("#st");
    st.className = "status";
    st.textContent = "Saving…";
    try {
      await invoke("set_settings", { wow: wowIn.value, data: dataIn.value });
      await render(body);
      const done = body.querySelector("#st");
      done.className = "status ok";
      done.textContent = "Saved.";
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };
}
