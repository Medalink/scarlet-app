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
import * as addons from "./views/addons.js";
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
  addons: ["Addons", addons],
  compress: ["Compress", compress],
  settings: ["Settings", settings],
};

let current = "overview";

// A route is a view key with an optional page after a slash: "settings/updates".
const routeKey = (route) => route.split("/")[0];

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

// Renders run one at a time, so a view that is still waiting on the app when the user moves on has
// finished drawing before the next one clears the page (else its page lands under the next one's).
// Of the navigations that queue up behind a render, only the last one runs.
let navSeq = 0;
let navTail = Promise.resolve();
function setView() {
  const mine = ++navSeq;
  const run = navTail.then(async () => {
    if (mine !== navSeq) return;
    const route = (location.hash || "#overview").slice(1);
    const [key, sub] = route.split("/");
    current = VIEWS[key] ? key : "overview";
    document.querySelectorAll("#nav .item").forEach((i) => i.classList.toggle("active", i.dataset.key === current));
    $("#pagetitle").textContent = VIEWS[current][0];
    await VIEWS[current][1].render($("#body"), VIEWS[key] ? sub : undefined);
    if (mine === navSeq) invoke("save_view", { view: VIEWS[key] ? route : current });
  });
  navTail = run.catch(() => {});
  return run;
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

// The build's version, and a link to Settings while an update is waiting.
function showVersion(info) {
  $("#ver").textContent = info.dev ? "dev" : `v${info.version}`;
  const upd = $("#upd");
  upd.hidden = !info.offered;
  upd.textContent = info.offered ? `Update ${info.offered.version}` : "";
}

async function boot() {
  buildNav();
  showVersion(await invoke("update_info"));
  // Restore the last view before the first render, so save_view in setView
  // does not overwrite it with "overview". Asked together with the app state.
  const [, s] = await Promise.all([loadState(), location.hash ? null : invoke("get_settings")]);
  if (s?.last_view && VIEWS[routeKey(s.last_view)]) location.hash = s.last_view;
  await refreshCharSelector();
  headStats();
  $("#char").onchange = async (e) => {
    state.ch = e.target.value || null;
    await setView();
  };
  await setView();

  window.addEventListener("hashchange", setView);

  const { listen } = window.__TAURI__.event;
  $("#upd").onclick = () => (location.hash = "settings/updates");
  await listen("update-changed", (e) => showVersion(e.payload));
  await listen("store-changed", async () => {
    await loadState();
    headStats();
    await refreshCharSelector();
    // Settings draws itself after a save; redrawing it here would wipe "Saved." and whatever
    // the player is typing when an export lands.
    if (!location.hash.startsWith("#settings")) await setView();
  });
}

boot().catch((e) => {
  document.getElementById("pagetitle").textContent = "boot: " + (e?.message ?? e);
  document.getElementById("pagetitle").style.color = "#ff2d55";
});
