// Shell: sidebar nav, character filter, store-changed refresh, hash routing.

import { invoke, listen, el, $, num, esc, displayName, state } from "./lib.js";
import * as roster from "./views/roster.js";
import * as feats from "./views/feats.js";
import { stopToasts } from "./feat-toast.js";
import * as stats from "./views/stats.js";
import * as activity from "./views/activity.js";
import * as log from "./views/log.js";
import * as sessions from "./views/sessions.js";
import * as map from "./views/map.js";
import * as loot from "./views/loot.js";
import * as character from "./views/character.js";
import * as legacy from "./views/legacy.js";
import * as items from "./views/items.js";
import * as data from "./views/data.js";
import * as settings from "./views/settings.js";
import * as system from "./views/system.js";
import { openSetup } from "./setup.js";
import { webState, footLine } from "./link.js";

const VIEWS = {
  sessions: ["Sessions", sessions],
  feats: ["Feats", feats],
  character: ["Character", character],
  stats: ["Stats", stats],
  map: ["Map", map],
  loot: ["Loot", loot],
  legacy: ["Legacy", legacy],
  characters: ["Characters", roster],
  items: ["Items", items],
  activity: ["Activity", activity],
  log: ["Log", log],
  data: ["Data", data],
  settings: ["Settings", settings],
  system: ["Components", system],
};

const NAV_GROUPS = [
  ["Play", ["sessions", "feats"]],
  ["Character", ["character", "legacy", "stats", "map", "loot"]],
  ["Account", ["characters", "items", "activity", "log"]],
  ["System", ["data", "settings"]],
];

const NAV_ICONS = {
  character: '<circle cx="12" cy="8" r="3.6"/><path d="M4.8 20c.7-4 3.5-6.1 7.2-6.1s6.5 2.1 7.2 6.1"/>',
  characters: '<circle cx="9" cy="8.4" r="3"/><circle cx="17" cy="9.4" r="2.4"/><path d="M3.4 19c.6-3.4 2.9-5.2 5.6-5.2s5 1.8 5.6 5.2M15.4 14.2c2.6-.2 4.5 1.3 5.2 4.2"/>',
  feats: '<path d="m12 3 7.2 3v5.6c0 4.3-3 7.6-7.2 9.4-4.2-1.8-7.2-5.1-7.2-9.4V6z"/>',
  stats: '<path d="M4 19.5h16M7 16v-5m5 5V6m5 10v-8"/>',
  activity: '<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><path d="M3.5 9.5h17M9 9.5v10M15 9.5v10"/>',
  log: '<path d="M5 7h14M5 12h10M5 17h14"/>',
  legacy: '<path d="M4.5 17.5 3.5 7.5l4.8 3.6L12 4.5l3.7 6.6 4.8-3.6-1 10z"/><path d="M5 20h14"/>',
  sessions: '<circle cx="12" cy="12" r="8.6"/><path d="M12 7v5.2l3.4 2"/>',
  map: '<circle cx="12" cy="12" r="8.6"/><path d="m15.6 8.4-2.1 5.1-5.1 2.1 2.1-5.1z"/>',
  loot: '<path d="M8 8.6h8l2.2 9.1a2 2 0 0 1-2 2.5H7.8a2 2 0 0 1-2-2.5z"/><path d="M9 8.6 7.9 4.8h8.2L15 8.6"/>',
  items: '<path d="M4.5 7.5 12 3.5l7.5 4v9L12 20.5l-7.5-4z"/><path d="M12 11.5v9M4.5 7.5 12 11.5l7.5-4"/>',
  data: '<path d="M12 4v11m0 0-3.5-3.5M12 15l3.5-3.5M4.5 19.5h15"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M12 3.5v2.2m0 12.6v2.2M3.5 12h2.2m12.6 0h2.2m-14-6 1.6 1.6m9.8 9.8 1.6 1.6m0-13-1.6 1.6m-9.8 9.8-1.6 1.6"/>',
};

let current = "sessions";
let activeMod = null;

// A route is a view key with an optional page after a slash: "settings/updates".
const routeKey = (route) => route.split("/")[0];

const REDIRECTS = {
  overview: () => (state.ch ? "character" : "characters"),
  classpage: () => "character/class",
  talents: () => "character/talents",
  charts: () => "stats",
  timeline: () => "log",
  heatmap: () => "activity",
  compress: () => "data",
  addons: () => "data",
};

function resolveRoute(route) {
  const [key, ...rest] = route.split("/");
  const to = REDIRECTS[key]?.();
  return to ? [to, ...rest].join("/") : route;
}

const labelOf = (key) => (typeof VIEWS[key][0] === "function" ? VIEWS[key][0]() : VIEWS[key][0]);

function buildNav() {
  const nav = $("#nav");
  for (const [groupName, keys] of NAV_GROUPS) {
    const heading = el("div", "nav-group");
    heading.textContent = groupName;
    nav.appendChild(heading);
    for (const key of keys) {
      const item = el("button", "item");
      item.type = "button";
      item.dataset.key = key;
      const ico = NAV_ICONS[key] ? `<svg class="nav-ico" viewBox="0 0 24 24" aria-hidden="true">${NAV_ICONS[key]}</svg>` : "";
      item.innerHTML = `${ico}<span class="nav-label">${labelOf(key)}</span>`;
      item.onclick = () => {
        location.hash = key;
      };
      nav.appendChild(item);
    }
  }
  const foot = el("div", "foot");
  foot.id = "foot";
  foot.onclick = (e) => {
    if (e.target.closest(".weblink")) location.hash = "settings/website";
  };
  nav.appendChild(foot);
}

let wowClients = 0;
let rebuilt = false;

function updateFoot() {
  const st = state.store;
  if (!st) return;
  const ev = `${num(st.events)} ${st.events === 1 ? "event" : "events"}`;
  const ch = `${st.chars.length} ${st.chars.length === 1 ? "character" : "characters"}`;
  const running = wowClients > 0 ? `<br><span class="foot-live" aria-hidden="true"></span>${wowClients} WoW ${wowClients === 1 ? "client" : "clients"} running` : "";
  const web = webLine ? `<button type="button" class="weblink" title="Settings, Website"><span class="sc-dot${webLine.linked ? "" : " off"}"></span> ${esc(webLine.text)}</button>` : "";
  $("#foot").innerHTML = `${web}${ev}<br>${ch}${running}`;
}

let webLine = null;

async function refreshWebLine() {
  webLine = footLine(await webState());
  updateFoot();
}

window.addEventListener("scarlet:web", refreshWebLine);

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
    const asked = (location.hash || "#sessions").slice(1);
    const route = resolveRoute(asked);
    if (route !== asked) history.replaceState(null, "", `#${route}`);
    const [key, sub] = route.split("/");
    const next = VIEWS[key] ? key : "sessions";
    if (activeMod?.cleanup) activeMod.cleanup();
    current = next;
    activeMod = VIEWS[current][1];
    if (current !== "feats") stopToasts();
    document.querySelectorAll("#nav .item").forEach((i) => {
      const isActive = i.dataset.key === current;
      i.classList.toggle("active", isActive);
      if (isActive) i.setAttribute("aria-current", "page");
      else i.removeAttribute("aria-current");
      i.querySelector(".nav-label").textContent = labelOf(i.dataset.key);
    });
    $("#pagetitle").textContent = labelOf(current);
    refreshWebLine();
    let body = $("#body");
    if (body.dataset.route !== route) {
      const fresh = body.cloneNode(false);
      fresh.classList.remove("play");
      fresh.dataset.route = route;
      body.replaceWith(fresh);
      body = fresh;
    }
    delete body.dataset.ready;
    await activeMod.render(body, VIEWS[key] ? sub : undefined);
    body.dataset.ready = "1";
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
    sel.insertAdjacentHTML("beforeend", `<option value="${esc(c)}">${esc(displayName(c))}</option>`);
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
  $("#head-stats").textContent = `${num(st.events)} ${st.events === 1 ? "event" : "events"} · ${a} – ${b}`;
}

function showVersion(info) {
  $("#ver").textContent = info.dev ? "dev" : `v${info.version}`;
  const upd = $("#upd");
  rebuilt = info.rebuilt;
  upd.hidden = !info.offered && !rebuilt;
  upd.textContent = rebuilt ? "Restart to update" : info.offered ? `Update ${info.offered.version}` : "";
}

async function boot() {
  buildNav();
  const [info, , s] = await Promise.all([invoke("update_info"), loadState(), invoke("get_settings")]);
  showVersion(info);
  const resumed = s?.last_view ? resolveRoute(s.last_view) : "";
  if (!location.hash && VIEWS[routeKey(resumed)]) history.replaceState(null, "", `#${resumed}`);
  if (s && !s.setup_done) openSetup();
  await refreshCharSelector();
  headStats();
  $("#char").onchange = async (e) => {
    state.ch = e.target.value || null;
    await setView();
  };
  window.addEventListener("hashchange", setView);
  await setView();
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
      const search = document.querySelector("#body input[placeholder]");
      if (search) {
        e.preventDefault();
        search.focus();
        search.select();
      }
    } else if (e.altKey && e.key === "ArrowLeft") {
      e.preventDefault();
      history.back();
    } else if (e.altKey && e.key === "ArrowRight") {
      e.preventDefault();
      history.forward();
    }
  });

  const openRun = async () => {
    const run = await invoke("take_wow_run").catch(() => null);
    if (run) location.hash = `sessions/${run.from}-${run.to}`;
  };
  await listen("wow-run", openRun);
  await openRun();

  // How many clients run: two for a player who dual boxes. Cheap, so it is asked every 5 s.
  const askClients = async () => {
    const n = await invoke("wow_clients").catch(() => wowClients);
    if (n !== wowClients) {
      wowClients = n;
      updateFoot();
    }
  };
  askClients();
  setInterval(askClients, 5000);

  $("#upd").onclick = () => (rebuilt ? invoke("restart_rebuilt") : (location.hash = "settings/updates"));
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
