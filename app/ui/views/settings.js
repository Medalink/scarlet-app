// Settings: WoW folder and data folder, stored next to the exe.

import { invoke, el } from "../lib.js";

export async function render(body) {
  body.innerHTML = "";
  const s = await invoke("get_settings");

  const form = el("div");
  form.innerHTML = `
    <div class="field"><label>WoW folder</label></div>
    <div class="field"><label>Data folder</label></div>
    <button class="btn primary" id="save">Save</button>
    <div class="status" id="st"></div>
    <div class="note">Settings live in settings.json beside the exe. The data
    folder holds the exported text, payloads and compressed combat logs; it
    must not be on C:. The WebView2 profile lives beside the exe too, so
    nothing lands in %LOCALAPPDATA%.</div>`;
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
      const ns = await invoke("set_settings", { wow: wowIn.value, data: dataIn.value });
      st.className = "status ok";
      st.textContent = `Saved. Data folder: ${ns.data}`;
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };
}
