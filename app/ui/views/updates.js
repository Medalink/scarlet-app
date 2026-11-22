// Settings > Updates and Settings > Addon: the version, the Release or Beta choice, the offered update
// with its notes and the recent releases; and the state of the Scarlet addon in the WoW folder.

import { invoke, el, bytes, makeSwitch } from "../lib.js";

const stamp = (ms) => new Date(ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
const day = (iso) => (iso ? new Date(iso).toLocaleDateString([], { dateStyle: "medium" }) : "");

export const label = (info) => (info.dev ? "dev" : info.version);

// The newest draw function; a push from the app redraws whichever Settings page is open.
let redraw = null;
let progress = null;
let statusEl = null;
let listening = false;

const downloading = () =>
  progress ? `Downloading… ${bytes(progress[0])}${progress[1] ? ` of ${bytes(progress[1])}` : ""}` : "Downloading…";

async function listenOnce() {
  if (listening) return;
  listening = true;
  const { listen } = window.__TAURI__.event;
  await listen("update-changed", () => redraw?.());
  await listen("addon-progress", (e) => {
    const busy = installing.get(e.payload.folder);
    if (!busy) return;
    busy.label = progressLabel(e.payload);
    if (busy.button?.isConnected) busy.button.textContent = busy.label;
  });
  await listen("update-progress", (e) => {
    progress = e.payload;
    if (statusEl?.isConnected) statusEl.textContent = downloading();
  });
}

function text(tag, cls, value) {
  const e = el(tag, cls);
  e.textContent = value;
  return e;
}

function line(parent, name, value) {
  const row = el("div", "kv");
  row.append(text("span", "k", name), text("span", "v", value));
  parent.appendChild(row);
}

function status(info) {
  const s = el("div", "status");
  if (info.installing) {
    s.textContent = progress
      ? `Downloading… ${bytes(progress[0])}${progress[1] ? ` of ${bytes(progress[1])}` : ""}`
      : "Downloading…";
  } else if (info.checking) {
    s.textContent = "Checking…";
  } else if (info.error) {
    s.className = "status err";
    s.textContent = info.error;
  } else if (info.offered) {
    s.className = "status ok";
    s.textContent = `Version ${info.offered.version} is available.`;
  } else if (!info.enabled) {
    s.textContent = `This is a development build. It does not update itself.${info.latest ? ` The latest ${info.track} is ${info.latest}.` : ""}`;
  } else if (info.latest && info.ahead_of_track) {
    s.textContent = `This build is newer than the latest ${info.track} (${info.latest}). Updates resume with the next ${info.track}.`;
  } else if (info.latest) {
    s.textContent = "This is the newest version.";
  } else if (info.checked_ms) {
    s.textContent = `No ${info.track} has been published yet.`;
  }
  if (info.checked_ms && !info.checking && !info.installing) {
    s.append(text("span", "when", ` Checked ${stamp(info.checked_ms)}.`));
  }
  return s;
}

function notesList(parent, info) {
  const d = el("details", "notes-all");
  d.appendChild(text("summary", "", "Recent releases"));
  const list = el("div");
  d.appendChild(list);
  let loaded = false;
  d.ontoggle = async () => {
    if (!d.open || loaded) return;
    loaded = true;
    list.textContent = "Loading…";
    try {
      const notes = await invoke("release_notes");
      list.textContent = "";
      if (!notes.length) list.textContent = "No releases are published yet.";
      for (const n of notes) {
        const h = el("div", "note-head");
        h.append(text("b", "", n.version), text("span", "when", ` ${n.prerelease ? "beta, " : ""}${day(n.date)}`));
        list.append(h, text("pre", "notes", n.body || "No notes."));
      }
    } catch (e) {
      loaded = false;
      list.textContent = String(e);
    }
  };
  parent.appendChild(d);
}

// Folders with an install running: the label on the button, and the button so progress can change it.
const installing = new Map();
// The last failure of a folder, kept until its next try, so a redraw does not lose it.
const failures = new Map();
// The version a folder was at after the player's last install there.
const installed = new Map();

const kb = (n) => bytes(n);
const progressLabel = (p) =>
  p.stage === "write"
    ? "Installing…"
    : p.done
      ? `Downloading… ${kb(p.done)}${p.total ? ` of ${kb(p.total)}` : ""}`
      : "Downloading…";

function folderRow(f, track, onChange) {
  const trackName = track === "beta" ? "Beta" : "Release";
  const row = el("div", "addon-row");
  const head = el("div", "addon-head");
  head.appendChild(text("b", "", f.name));
  const actions = el("div", "row");
  head.appendChild(actions);
  row.appendChild(head);
  line(row, "Installed", f.installed ? f.installed : f.manager === "missing" ? "Not installed" : "Unknown");
  line(row, `Latest ${trackName}`, f.latest || "Unknown");
  if (f.kept_by) line(row, "Managed by", f.kept_by);
  const err = el("div", "status err");
  err.textContent = failures.get(f.name) || "";
  row.appendChild(err);
  // Shown while the folder still holds the version the player installed here.
  if (installed.get(f.name) === f.installed && f.manager === "ours") {
    row.appendChild(text("div", "status ok", "Type /reload in WoW or restart the game to load it."));
  }

  const run = async (takeover) => {
    failures.delete(f.name);
    installed.delete(f.name);
    err.textContent = "";
    installing.set(f.name, { label: "Downloading…", button: null });
    drawActions();
    try {
      const done = await invoke("addon_install", { folder: f.name, takeover });
      installed.set(f.name, done.folders.find((x) => x.name === f.name)?.installed);
    } catch (e) {
      failures.set(f.name, String(e));
    }
    installing.delete(f.name);
    onChange();
  };

  const button = (name, primary, onclick) => {
    const b = el("button", primary ? "btn primary" : "btn", name);
    b.type = "button";
    b.onclick = onclick;
    return b;
  };

  const drawActions = (confirming = false) => {
    actions.textContent = "";
    const busy = installing.get(f.name);
    if (busy) {
      const b = button(busy.label, true, null);
      b.disabled = true;
      busy.button = b;
      actions.appendChild(b);
      return;
    }
    if (confirming) {
      actions.append(
        text("span", "when", "Scarlet will manage this copy from now on."),
        button("Continue", true, () => run(true)),
        button("Cancel", false, () => drawActions()),
      );
      return;
    }
    const go = f.takeover ? () => drawActions(true) : () => run(false);
    if (f.can_install) actions.appendChild(button("Install", true, go));
    if (f.can_update) actions.appendChild(button(`Update to ${f.latest}`, true, go));
    if (f.can_reinstall) actions.appendChild(button("Reinstall", !f.can_update, go));
  };
  drawActions();
  return row;
}

function addonBlock(parent, a, onChange) {
  const sw = makeSwitch("Keep the addon up to date", a.keep);
  sw.onclick = async () => {
    await invoke("set_keep_addon", { on: !sw.classList.contains("on") });
    onChange();
  };
  parent.appendChild(sw);
  const list = el("div", "addon-list");
  if (a.problem) {
    list.appendChild(text("div", "status", a.problem));
  } else {
    for (const f of a.folders) list.appendChild(folderRow(f, a.track, onChange));
  }
  parent.appendChild(list);
  if (a.last) {
    const s = text("div", a.last_error ? "status err" : "status ok", a.last);
    parent.appendChild(s);
  }
}

// The red lines at the top of General were read when the page opened; the app may have installed the addon since.
export async function refreshProblems() {
  const missing = document.querySelector("#missing");
  if (!missing) return;
  const s = await invoke("get_settings");
  missing.textContent = "";
  for (const p of s.problems) missing.appendChild(text("div", "", p));
  missing.hidden = s.problems.length === 0;
}

export async function watchProblems() {
  await listenOnce();
  redraw = refreshProblems;
}

export async function renderUpdates(body) {
  const box = el("div");
  body.appendChild(box);
  await listenOnce();

  const draw = async () => {
    const info = await invoke("update_info");
    if (!box.isConnected) {
      if (redraw === draw) redraw = null;
      return;
    }
    refreshProblems();
    box.textContent = "";
    line(box, "Version", label(info));

    const track = el("div", "field");
    track.appendChild(text("label", "", "Updates from"));
    const seg = el("div", "seg");
    seg.setAttribute("role", "group");
    seg.setAttribute("aria-label", "Updates from");
    for (const [key, name] of [["release", "Release"], ["beta", "Beta"]]) {
      const b = el("button", "", name);
      b.type = "button";
      b.setAttribute("aria-pressed", String(info.track === key));
      b.onclick = async () => {
        if (info.track === key) return;
        seg.querySelectorAll("button").forEach((x) => (x.disabled = true));
        await invoke("set_track", { track: key });
        draw();
      };
      seg.appendChild(b);
    }
    track.appendChild(seg);
    box.appendChild(track);

    const actions = el("div", "row");
    actions.style.marginTop = "14px";
    const check = el("button", "btn", "Check for updates");
    check.type = "button";
    check.disabled = info.checking || info.installing;
    check.onclick = async () => {
      check.disabled = true;
      await invoke("update_check");
      draw();
    };
    actions.appendChild(check);
    if (info.offered) {
      const install = el("button", "btn primary", "Install and restart");
      install.type = "button";
      install.disabled = info.installing;
      install.onclick = async () => {
        install.disabled = true;
        progress = null;
        try {
          await invoke("update_install");
        } catch (e) {
          await draw();
          if (statusEl) {
            statusEl.className = "status err";
            statusEl.textContent = String(e);
          }
        }
      };
      actions.appendChild(install);
    }
    box.appendChild(actions);
    statusEl = status(info);
    box.appendChild(statusEl);

    if (info.offered) {
      box.appendChild(text("div", "note-head", `Changes in ${info.offered.version}`));
      box.appendChild(text("pre", "notes", info.offered.notes || "No notes."));
    }
    notesList(box, info);
  };
  redraw = draw;
  await draw();
}

export async function renderAddon(body) {
  const box = el("div");
  body.appendChild(box);
  await listenOnce();

  const draw = async () => {
    const addon = await invoke("addon_status");
    if (!box.isConnected) {
      if (redraw === draw) redraw = null;
      return;
    }
    refreshProblems();
    box.textContent = "";
    addonBlock(box, addon, draw);
  };
  redraw = draw;
  await draw();
}
