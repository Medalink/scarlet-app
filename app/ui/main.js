// Shell: sidebar nav, character filter, store-changed refresh, hash routing.

import { invoke, el, $, num, esc, state } from "./lib.js";
import * as overview from "./views/overview.js";
import * as stats from "./views/stats.js";
import * as heatmap from "./views/heatmap.js";
import * as timeline from "./views/timeline.js";
import * as sessions from "./views/sessions.js";
import * as map from "./views/map.js";
import * as charts from "./views/charts.js";
import * as loot from "./views/loot.js";
import * as compress from "./views/compress.js";
import * as settings from "./views/settings.js";

const VIEWS = {
  overview: ["Overview", overview],
  stats: ["Stats", stats],
  heatmap: ["Activity", heatmap],
  timeline: ["Timeline", timeline],
  sessions: ["Sessions", sessions],
  map: ["Map", map],
  charts: ["Charts", charts],
  loot: ["Loot", loot],
  compress: ["Compress", compress],
  settings: ["Settings", settings],
};

let current = "overview";

function buildNav() {
  const nav = $("#nav");
  for (const [key, [label]] of Object.entries(VIEWS)) {
    const item = el("div", "item");
    item.dataset.key = key;
    item.innerHTML = `<span>${label}</span>`;
    item.onclick = () => {
      location.hash = key;
    };
    nav.appendChild(item);
  }
  const foot = el("div", "foot");
  foot.id = "foot";
  nav.appendChild(foot);
}

function updateFoot() {
  const st = state.store;
  if (!st) return;
  $("#foot").innerHTML = `${num(st.events)} events<br>${st.chars.length} character(s)`;
}

window.__nav = (key) => {
  location.hash = key;
};

async function setView() {
  const key = (location.hash || "#overview").slice(1);
  current = VIEWS[key] ? key : "overview";
  document.querySelectorAll("#nav .item").forEach((i) => i.classList.toggle("active", i.dataset.key === current));
  $("#pagetitle").textContent = VIEWS[current][0];
  await VIEWS[current][1].render($("#body"));
  invoke("save_view", { view: current });
}

async function refreshCharSelector() {
  const sel = $("#char");
  const prev = state.ch;
  sel.innerHTML = `<option value="">All characters</option>`;
  for (const c of state.store?.chars ?? []) {
    sel.insertAdjacentHTML("beforeend", `<option value="${esc(c)}">${esc(c)}</option>`);
  }
  sel.value = prev ?? "";
}

async function loadState() {
  state.store = await invoke("app_state");
  updateFoot();
}

function headStats() {
  const st = state.store;
  if (!st || !st.events) {
    $("#head-stats").textContent = "";
    return;
  }
  const a = new Date(st.first_ms).toLocaleDateString([], { month: "short", day: "numeric" });
  const b = new Date(st.last_ms).toLocaleDateString([], { month: "short", day: "numeric" });
  $("#head-stats").textContent = `${num(st.events)} events · ${a} – ${b}`;
}

async function boot() {
  buildNav();
  await loadState();
  await refreshCharSelector();
  headStats();
  $("#char").onchange = async (e) => {
    state.ch = e.target.value || null;
    await setView();
  };

  // Restore the last view before the first render, so save_view in setView
  // does not overwrite it with "overview".
  if (!location.hash) {
    const s = await invoke("get_settings");
    if (s.last_view && VIEWS[s.last_view]) location.hash = s.last_view;
  }
  await setView();

  window.addEventListener("hashchange", setView);

  const { listen } = window.__TAURI__.event;
  await listen("store-changed", async () => {
    await loadState();
    headStats();
    await refreshCharSelector();
    await setView();
  });
}

boot().catch((e) => {
  document.getElementById("pagetitle").textContent = "boot: " + (e?.message ?? e);
  document.getElementById("pagetitle").style.color = "#ff2d55";
});
