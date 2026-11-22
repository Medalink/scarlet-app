// Settings: one page per topic, chosen by the tabs at the top and by the hash (#settings/updates).

import { invoke, el, esc, openmapMissing } from "../lib.js";
import { renderUpload } from "./upload.js";
import { renderUpdates, renderAddon, watchProblems, refreshProblems } from "./updates.js";

const PAGES = [
  ["general", "General", general],
  ["map", "Map", mapArt],
  ["updates", "Updates", renderUpdates],
  ["addon", "Addon", renderAddon],
  ["website", "Website", renderUpload],
];

export async function render(body, sub) {
  const key = PAGES.some(([k]) => k === sub) ? sub : "general";
  body.innerHTML = "";
  const tabs = el("div", "subnav");
  tabs.setAttribute("role", "tablist");
  for (const [k, name] of PAGES) {
    const a = el("a", "", name);
    a.href = k === "general" ? "#settings" : `#settings/${k}`;
    a.setAttribute("role", "tab");
    a.setAttribute("aria-selected", String(k === key));
    tabs.appendChild(a);
  }
  // What is missing on this PC shows on every page, so a player who lands on one still sees it.
  const missing = el("div", "status err");
  missing.id = "missing";
  missing.hidden = true;
  const page = el("div");
  body.append(tabs, missing, page);
  await refreshProblems();
  await PAGES.find(([k]) => k === key)[2](page);
}

// WoW folder and data folder, what is missing on this PC, where they are stored, and starting with WoW.
async function general(body) {
  body.innerHTML = "";
  const s = await invoke("get_settings");
  watchProblems();

  const form = el("div");
  form.innerHTML = `
    <div class="field"><label>WoW folder</label></div>
    <div class="field"><label>Data folder</label></div>
    <button class="btn primary" id="save">Save</button>
    <div class="status" id="st"></div>
    <div class="note" id="where"></div>`;
  form.querySelector("#where").textContent = s.keep_off_c
    ? `Settings are in ${s.file}. The data folder must not be on C:.`
    : `Settings are in ${s.file}.`;
  const [wow, data] = form.querySelectorAll(".field");
  const wowIn = el("input");
  wowIn.value = s.wow;
  const dataIn = el("input");
  dataIn.value = s.data;
  const wowRow = el("div", "row");
  const browse = el("button", "btn", "Browse…");
  browse.type = "button";
  wowRow.append(wowIn, browse);
  wow.appendChild(wowRow);
  data.appendChild(dataIn);
  body.appendChild(form);

  // The picker blocks until it closes. A folder without _forever_ is not saved; the line says why.
  browse.onclick = async () => {
    const st = form.querySelector("#st");
    st.className = "status";
    st.textContent = "";
    try {
      const picked = await invoke("browse_wow");
      if (picked === null) return;
      await invoke("set_settings", { wow: picked, data: dataIn.value });
      await general(body);
      await refreshProblems();
      const done = body.querySelector("#st");
      done.className = "status ok";
      done.textContent = "Saved.";
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };

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
  const grp = el("span", "switch");
  grp.innerHTML = `<span class="track"><span class="knob"></span></span><span class="lab">Group characters by game account</span>`;
  grp.classList.toggle("on", !!s.group_by_account);
  grp.style.marginTop = "16px";
  grp.onclick = async () => {
    const on = !grp.classList.contains("on");
    grp.classList.toggle("on", on);
    swSt.className = "status";
    swSt.textContent = "";
    try {
      await invoke("set_group_by_account", { on });
    } catch (e) {
      grp.classList.toggle("on", !on);
      swSt.className = "status err";
      swSt.textContent = String(e);
    }
  };
  for (const x of [sw, grp]) {
    x.style.display = "flex";
    x.style.width = "fit-content";
  }
  body.append(sw, grp, swSt);
  const boxes = el("div");
  boxes.style.marginTop = "22px";
  body.appendChild(boxes);
  await drawBoxes(body, boxes, s, dataIn);

  form.querySelector("#save").onclick = async () => {
    const st = form.querySelector("#st");
    st.className = "status";
    st.textContent = "Saving…";
    try {
      await invoke("set_settings", { wow: wowIn.value, data: dataIn.value });
      await general(body);
      await refreshProblems();
      const done = body.querySelector("#st");
      done.className = "status ok";
      done.textContent = "Saved.";
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };
}

let boxesRedraw = null;
let boxesListening = false;

function addonText(a) {
  const ver = a.installed ? `${a.name} ${a.installed}` : a.manager === "missing" ? `${a.name} not installed` : `${a.name} version unknown`;
  if (a.manager === "ours") return a.can_update ? `${ver}, ${a.latest} on the next sync` : ver;
  if (a.manager === "missing") return a.latest ? `${ver}, ${a.latest} on the next sync` : ver;
  return `${ver}, kept by ${a.kept_by.toLowerCase()}`;
}

async function drawBoxes(body, box, s, dataIn) {
  boxesRedraw = () => (box.isConnected ? drawBoxes(body, box, s, dataIn) : (boxesRedraw = null));
  if (!boxesListening) {
    boxesListening = true;
    window.__TAURI__.event.listen("update-changed", () => boxesRedraw?.());
  }
  const list = await invoke("wow_folders");
  const extra = list.filter((f) => !f.primary).map((f) => f.path);
  const save = async (more) => {
    await invoke("set_settings", { wow: s.wow, data: dataIn.value, moreWow: more });
    await general(body);
  };
  box.innerHTML = "";
  const st = el("div", "status");
  const addFolder = async () => {
    st.className = "status";
    st.textContent = "";
    try {
      const picked = await invoke("browse_wow");
      if (picked === null) return;
      if (list.some((f) => f.path.toLowerCase() === picked.toLowerCase())) return;
      await save([...extra, picked]);
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };
  const head = el("div", "split-head");
  head.innerHTML = `<span>WoW folders</span><span>${list.reduce((n, f) => n + f.running, 0)} running</span>`;
  box.appendChild(head);
  for (const f of list) {
    const card = el("div", "wowbox");
    const top = el("div", "wb-head");
    const title = el("div", "wb-path");
    title.textContent = f.path;
    const state = el("div", "wb-state");
    state.textContent = f.problem ? f.problem : f.running ? `${f.running} running` : "not running";
    state.classList.toggle("on", f.running > 0 && !f.problem);
    top.append(title, state);
    const ctl = el("button", "btn", f.primary ? "+" : "×");
    ctl.type = "button";
    ctl.setAttribute("aria-label", f.primary ? "Add a WoW folder" : "Remove this folder");
    if (f.primary) ctl.onclick = addFolder;
    else ctl.onclick = () => save(extra.filter((p) => p !== f.path)).catch(() => {});
    top.appendChild(ctl);
    card.appendChild(top);
    for (const a of f.addon) {
      const line = el("div", "dim");
      line.textContent = addonText(a);
      card.appendChild(line);
    }
    if (!f.problem && !f.accounts.length) {
      const none = el("div", "dim");
      none.textContent = "No game accounts yet.";
      card.appendChild(none);
    }
    const t = el("table");
    for (const a of f.accounts) {
      const tr = el("tr");
      const when = a.saved ? new Date(a.saved * 1000).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : "no recording yet";
      tr.innerHTML = `<td>${esc(a.name)}</td><td>${a.characters.length ? esc(a.characters.join(", ")) : ""}</td><td class="dim">${esc(when)}</td>`;
      t.appendChild(tr);
    }
    if (f.accounts.length) card.appendChild(t);
    box.appendChild(card);
  }
  box.appendChild(st);
}

// Map art: Scarlet's own dark sheets, or the real map art read from the player's OpenMap addon.
async function mapArt(body) {
  const s = await invoke("get_settings");
  const artField = el("div", "field");
  artField.innerHTML = `<label>Map art</label>`;
  const artSeg = el("div", "seg");
  artSeg.setAttribute("role", "group");
  artSeg.setAttribute("aria-label", "Map art");
  const artNote = el("div", "status");
  // Only the latest choice writes the note: an earlier one still waiting for the status would add
  // its own note, twice or for the art no longer chosen.
  let artSeq = 0;
  const showArt = async (art) => {
    const mine = ++artSeq;
    artSeg.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x.dataset.art === art)));
    let st = null, failed = null;
    try { st = await invoke("map_art_status"); } catch (e) { failed = e; }
    if (mine !== artSeq) return;
    artNote.className = "status";
    artNote.textContent = "";
    if (failed) {
      artNote.className = "status err";
      artNote.textContent = String(failed);
    } else if (art === "openmap" && !st.installed) openmapMissing(artNote, st);
  };
  for (const [art, label] of [["dark", "Scarlet dark"], ["openmap", "OpenMap"]]) {
    const b = el("button", "", label);
    b.type = "button";
    b.dataset.art = art;
    b.onclick = async () => {
      try {
        await showArt(await invoke("set_map_art", { art }));
      } catch (e) {
        artSeq++; // a choice still waiting for the status must not wipe this
        artNote.className = "status err";
        artNote.textContent = String(e);
      }
    };
    artSeg.appendChild(b);
  }
  artField.appendChild(artSeg);
  body.append(artField, artNote);
  showArt(s.map_art || "dark");
}
