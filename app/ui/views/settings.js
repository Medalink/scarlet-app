// Settings: one page per topic, chosen by the tabs at the top and by the hash (#settings/updates).

import { invoke, el, elText, esc, listen, displayName, openmapMissing, makeSwitch, setSwitch, stampHtml } from "../lib.js";
import { openSetup } from "../setup.js";
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

async function general(body) {
  body.innerHTML = "";
  const s = await invoke("get_settings");
  watchProblems();

  const boxes = el("div");
  body.appendChild(boxes);

  const data = el("div", "field");
  data.style.marginTop = "22px";
  data.innerHTML = `<label for="set-data">Data folder</label>`;
  const dataIn = el("input");
  dataIn.id = "set-data";
  dataIn.value = s.data;
  const dataRow = el("div", "row");
  const browseData = el("button", "btn", "Browse…");
  browseData.type = "button";
  const save = el("button", "btn primary", "Save");
  save.type = "button";
  save.id = "save";
  dataRow.append(dataIn, browseData, save);
  data.appendChild(dataRow);
  if (s.keep_off_c) data.appendChild(el("div", "dim", "The data folder must not be on C:."));
  const st = el("div", "status");
  st.id = "st";
  body.append(data, st);

  browseData.onclick = async () => {
    const picked = await invoke("browse_data");
    if (picked) dataIn.value = picked;
  };
  save.onclick = async () => {
    st.className = "status";
    st.textContent = "Saving…";
    try {
      await invoke("set_settings", { wow: s.wow, data: dataIn.value });
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

  const sw = makeSwitch("Open when WoW Forever starts", !!s.open_with_wow);
  sw.style.marginTop = "16px";
  const swSt = el("div", "status");
  sw.onclick = async () => {
    try {
      const on = await invoke("set_open_with_wow", { on: !sw.classList.contains("on") });
      setSwitch(sw, on);
      swSt.className = "status";
      swSt.textContent = "";
    } catch (e) {
      swSt.className = "status err";
      swSt.textContent = String(e);
    }
  };
  const grp = makeSwitch("Group characters by game account", !!s.group_by_account);
  grp.style.marginTop = "16px";
  grp.onclick = async () => {
    const on = !grp.classList.contains("on");
    setSwitch(grp, on);
    swSt.className = "status";
    swSt.textContent = "";
    try {
      await invoke("set_group_by_account", { on });
    } catch (e) {
      setSwitch(grp, !on);
      swSt.className = "status err";
      swSt.textContent = String(e);
    }
  };
  for (const x of [sw, grp]) {
    x.style.display = "flex";
    x.style.width = "fit-content";
  }
  body.append(sw, grp, swSt);
  const again = el("button", "btn", "Run setup again");
  again.type = "button";
  again.style.marginTop = "22px";
  again.onclick = () => openSetup();
  body.appendChild(again);

  const where = el("div", "settings-file");
  const open = el("button", "btn", "Open");
  open.type = "button";
  open.onclick = () => invoke("open_folder", { path: s.file }).catch((e) => {
    st.className = "status err";
    st.textContent = String(e);
  });
  where.append(elText("span", "dim", `Settings file ${s.file}`), open);
  body.appendChild(where);

  await drawBoxes(body, boxes, s, dataIn);
}

let boxesRedraw = null;
let boxesListening = false;

function addonText(a) {
  const ver = a.installed ? `${a.name} ${a.installed}` : a.manager === "missing" ? `${a.name} not installed` : `${a.name} version unknown`;
  if (a.manager === "ours") return a.can_update ? `${ver}, ${a.latest} on the next sync` : ver;
  if (a.manager === "missing") return a.latest ? `${ver}, ${a.latest} on the next sync` : ver;
  if (a.manager === "by-hand") return `${ver}, installed by hand`;
  if (a.manager === "git") return `${ver}, git checkout`;
  if (a.manager === "linked") return `${ver}, linked folder`;
  return `${ver}, managed by ${a.kept_by}`;
}

async function drawBoxes(body, box, s, dataIn) {
  boxesRedraw = () => (box.isConnected ? drawBoxes(body, box, s, dataIn) : (boxesRedraw = null));
  if (!boxesListening) {
    boxesListening = true;
    listen("update-changed", () => boxesRedraw?.());
  }
  const list = await invoke("wow_folders");
  const extra = list.filter((f) => !f.primary).map((f) => f.path);
  const save = async (wow, more) => {
    await invoke("set_settings", { wow, data: dataIn.value, moreWow: more });
    await general(body);
  };
  box.innerHTML = "";
  const st = el("div", "status");
  const pick = async (apply) => {
    st.className = "status";
    st.textContent = "";
    try {
      const picked = await invoke("browse_wow");
      if (picked === null) return;
      if (list.some((f) => f.path.toLowerCase() === picked.toLowerCase())) return;
      await apply(picked);
    } catch (e) {
      st.className = "status err";
      st.textContent = String(e);
    }
  };
  const head = el("div", "split-head");
  const add = el("button", "btn", "Add folder");
  add.type = "button";
  add.onclick = () => pick((picked) => save(s.wow, [...extra, picked]));
  head.append(el("span", "", "WoW folders"), add);
  box.appendChild(head);
  for (const f of list) {
    const card = el("div", "wowbox");
    const top = el("div", "wb-head");
    const title = el("div", "wb-path");
    title.textContent = f.path;
    const state = el("div", "wb-state");
    state.textContent = f.problem ? f.problem : f.running ? `${f.running} running` : "Not running";
    state.classList.toggle("on", f.running > 0 && !f.problem);
    top.append(title, state);
    const ctl = el("button", "btn", f.primary ? "Browse…" : "Remove");
    ctl.type = "button";
    ctl.setAttribute("aria-label", f.primary ? "Choose another WoW folder" : "Remove this folder");
    if (f.primary) ctl.onclick = () => pick((picked) => save(picked));
    else ctl.onclick = () => save(s.wow, extra.filter((p) => p !== f.path)).catch(() => {});
    top.appendChild(ctl);
    card.appendChild(top);
    for (const a of f.addon) {
      if (a.manager === "missing") {
        const line = el("div", "wb-missing");
        const install = el("button", "btn", "Install addon");
        install.type = "button";
        install.onclick = () => (location.hash = "settings/addon");
        line.append(elText("span", "", addonText(a)), install);
        card.appendChild(line);
        continue;
      }
      const line = el("div", "dim");
      line.textContent = addonText(a);
      card.appendChild(line);
    }
    if (!f.problem && !f.accounts.length) {
      const none = el("div", "dim");
      none.textContent = "No game accounts yet.";
      card.appendChild(none);
    }
    if (f.accounts.length) {
      const t = el("table");
      t.innerHTML = `<thead><tr><th>Account</th><th>Characters</th><th>Last read</th></tr></thead>`;
      const tb = el("tbody");
      for (const a of f.accounts) {
        const tr = el("tr");
        const when = a.saved ? stampHtml(a.saved * 1000) : "No recording yet";
        tr.innerHTML = `<td>${esc(a.name)}</td><td>${a.characters.length ? esc(a.characters.map(displayName).join(", ")) : ""}</td><td class="dim">${when}</td>`;
        tb.appendChild(tr);
      }
      t.appendChild(tb);
      card.appendChild(t);
    }
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
