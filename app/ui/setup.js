// First-run setup: find WoW Forever, install the Scarlet addon, link the website. Three steps
// in order, once for a new install, and again from Settings > General.

import { invoke, el, elText, listen, bytes } from "./lib.js";

const STEPS = ["wow", "addon", "link"];

let panel = null;
let step = 0;
let token = 0;
let redraw = null;
let listening = false;
let uploadWasOn = null;

const installing = new Map();
const failures = new Map();
const installedHere = new Set();

const progressLabel = (p) =>
  p.stage === "write"
    ? "Installing…"
    : p.done
      ? `Downloading… ${bytes(p.done)}${p.total ? ` of ${bytes(p.total)}` : ""}`
      : "Downloading…";

async function listenOnce() {
  if (listening) return;
  listening = true;
  await listen("update-changed", () => redraw?.());
  await listen("addon-progress", (e) => {
    const busy = installing.get(e.payload.folder);
    if (!busy) return;
    busy.label = progressLabel(e.payload);
    if (busy.button?.isConnected) busy.button.textContent = busy.label;
  });
}

export function openSetup() {
  if (panel) return;
  step = 0;
  uploadWasOn = null;
  failures.clear();
  installing.clear();
  installedHere.clear();
  panel = el("div", "setup");
  panel.id = "setup";
  const shell = document.querySelector("#shell");
  shell.hidden = true;
  shell.after(panel);
  listenOnce();
  draw();
}

function close() {
  token++;
  redraw = null;
  panel?.remove();
  panel = null;
  document.querySelector("#shell").hidden = false;
  window.dispatchEvent(new Event("hashchange"));
}

async function finish() {
  await invoke("set_setup_done", { done: true });
  close();
}

function back() {
  step--;
  draw();
}

function skip() {
  step++;
  draw();
}

function button(name, cls, onclick) {
  const b = el("button", cls, name);
  b.type = "button";
  b.onclick = onclick;
  return b;
}

function draw() {
  const mine = ++token;
  panel.textContent = "";
  const inner = el("div", "setup-inner");
  const heading = el("h2", "setup-title");
  const body = el("div", "setup-body");
  const foot = el("div", "setup-foot");
  inner.append(heading, body, foot);
  panel.appendChild(inner);
  const run = [stepWow, stepAddon, stepLink][step];
  const title = (text) => (heading.textContent = `${step + 1}. ${text}`);
  run({ mine, title, body, foot });
}

async function stepWow({ mine, title, body, foot }) {
  title("Find WoW Forever");
  const s = await invoke("get_settings");
  if (mine !== token) return;
  const folders = await invoke("wow_folders");
  if (mine !== token) return;
  const primary = folders.find((f) => f.primary) ?? null;
  const problem = s.wow
    ? (primary?.problem ?? null)
    : "Scarlet found no World of Warcraft on this PC. Choose the folder that contains _forever_.";
  if (s.wow) body.appendChild(el("div", "note", "")).textContent = `Scarlet will read the game in ${s.wow}.`;
  const status = el("div", problem ? "status err" : "status");
  if (problem) status.textContent = problem;
  body.appendChild(status);
  const browse = button("Browse…", "btn", async () => {
    try {
      const picked = await invoke("browse_wow");
      if (picked === null) return;
      await invoke("set_settings", { wow: picked, data: s.data });
      draw();
    } catch (e) {
      status.className = "status err";
      status.textContent = String(e);
    }
  });
  const next = button("Continue", "btn primary", async () => {
    try {
      if (!s.watch) await invoke("toggle_watch", { on: true });
      skip();
    } catch (e) {
      status.className = "status err";
      status.textContent = String(e);
    }
  });
  next.disabled = !s.wow || Boolean(problem);
  foot.append(browse, button("Skip", "btn", skip), next);
}

async function stepAddon({ mine, title, body, foot }) {
  title("Install the Scarlet addon");
  const info = await invoke("update_info");
  if (mine !== token) return;
  const dev = info.dev && !info.enabled;
  const list = el("div");
  body.appendChild(list);
  const status = el("div", "status");
  body.appendChild(status);
  let checked = false;

  const run = async (f, takeover) => {
    failures.delete(f.name);
    installing.set(f.name, { label: "Downloading…", button: null });
    draw();
    try {
      await invoke("addon_install", { folder: f.name, takeover });
      installedHere.add(f.name);
    } catch (e) {
      failures.set(f.name, String(e));
    }
    installing.delete(f.name);
    draw();
  };

  const drawRow = (f) => {
    const row = el("div", "addon-row");
    const head = el("div", "addon-head");
    head.appendChild(el("b", "", f.name));
    const actions = el("div", "row");
    head.appendChild(actions);
    row.appendChild(head);
    const state = el("div", "status");
    if (f.installed) {
      state.className = "status ok";
      state.textContent = `${f.name} ${f.installed} is installed${
        f.kept_by && f.kept_by !== "Scarlet" ? `, kept by ${f.kept_by.toLowerCase()}` : ""
      }.`;
    } else if (f.manager !== "missing") {
      state.textContent = `${f.name} is installed, kept by ${(f.kept_by ?? "someone else").toLowerCase()}.`;
    } else {
      state.textContent = `${f.name} is not installed.`;
    }
    row.appendChild(state);
    if (installedHere.has(f.name)) {
      row.appendChild(el("div", "status ok", "It loads the next time WoW starts or you type /reload."));
    }
    const fail = el("div", "status err");
    fail.textContent = failures.get(f.name) ?? "";
    row.appendChild(fail);
    const busy = installing.get(f.name);
    if (busy) {
      const b = button(busy.label, "btn primary", null);
      b.disabled = true;
      busy.button = b;
      actions.appendChild(b);
    } else if (f.can_install) {
      actions.appendChild(button("Install", "btn primary", () => run(f, false)));
    }
    return row;
  };

  const next = button("Continue", "btn primary", skip);
  const draw = async () => {
    const [a, update] = await Promise.all([invoke("addon_status"), invoke("update_info")]);
    if (mine !== token) return;
    list.textContent = "";
    if (a.problem) {
      status.className = "status err";
      status.textContent = a.problem;
      next.disabled = true;
      return;
    }
    const isInstalled = (f) => f.manager !== "missing";
    const allInstalled = a.folders.every(isInstalled);
    const noOffer = a.folders.some((f) => !isInstalled(f) && !f.latest);
    status.className = "status";
    status.textContent = "";
    if (!allInstalled && noOffer) {
      if (dev) {
        status.textContent = "This development build does not install the addon.";
      } else if (update.error && checked) {
        status.className = "status err";
        status.textContent = update.error;
      } else {
        if (!checked) {
          checked = true;
          invoke("update_check").catch(() => {});
        }
        status.textContent = "Checking for the addon…";
      }
    }
    for (const f of a.folders) list.appendChild(drawRow(f));
    next.disabled = !allInstalled;
  };
  redraw = draw;
  await draw();
  foot.append(button("Back", "btn", back), button("Skip", "btn", skip), next);
}

async function stepLink({ mine, title, body, foot }) {
  title("Link this app to scarletweb.io");
  body.appendChild(
    el(
      "div",
      "note",
      "Linking turns on uploading of your finished play sessions to the website under your account. You can switch groups off in Settings > Website.",
    ),
  );
  const status = el("div", "status");
  body.appendChild(status);
  const card = el("div", "linkcode");
  card.hidden = true;
  body.appendChild(card);
  const linkedLine = el("div", "status ok", "This app is linked to your account.");
  linkedLine.hidden = true;
  body.appendChild(linkedLine);
  if (uploadWasOn === null) {
    uploadWasOn = await invoke("upload_get")
      .then((v) => v.enabled)
      .catch(() => false);
  }
  if (mine !== token) return;
  const backBtn = button("Back", "btn", back);

  const showLinked = () => {
    card.hidden = true;
    status.textContent = "";
    linkedLine.hidden = false;
    foot.textContent = "";
    foot.append(backBtn, button("Finish", "btn primary", finish));
  };

  const showCode = (code) => {
    linkBtn.remove();
    card.textContent = "";
    card.hidden = false;
    const copy = button("Copy", "btn", async () => {
      try {
        await navigator.clipboard.writeText(code);
        copy.textContent = "Copied";
      } catch {
        copy.textContent = "Select and copy";
      }
    });
    const row = el("div", "row");
    row.append(elText("div", "code", code), copy);
    card.append(
      el("div", "lab", "Code"),
      row,
      el("div", "note", "Sign in on the page that opened in your browser and confirm this code."),
    );
    const again = button("Open the page again", "btn", async () => {
      try {
        const r = await invoke("upload_link");
        if (mine !== token) return;
        if (r.linked) showLinked();
        else await showPending();
      } catch (e) {
        fail(e);
      }
    });
    again.style.marginTop = "10px";
    card.appendChild(again);
  };

  const fail = (e) => {
    status.className = "status err";
    status.textContent = "";
    status.append(`${String(e)} `, button("Try again", "linkbtn", linkNow));
  };

  const showPending = async () => {
    const v = await invoke("upload_get");
    if (mine !== token) return;
    showCode(v.link_code ?? "");
  };

  const poll = () => {
    const timer = setInterval(async () => {
      if (mine !== token || !panel) {
        clearInterval(timer);
        return;
      }
      try {
        if (await invoke("upload_linked")) {
          clearInterval(timer);
          showLinked();
        }
      } catch {
        clearInterval(timer);
        if (mine === token) fail("The link state could not be read.");
      }
    }, 3000);
  };

  const linkNow = async () => {
    status.className = "status";
    status.textContent = "Linking…";
    try {
      await invoke("upload_set_enabled", { on: true });
      const r = await invoke("upload_link");
      if (mine !== token) return;
      if (r.linked) {
        showLinked();
        return;
      }
      status.textContent = "";
      await showPending();
      poll();
    } catch (e) {
      if (mine !== token) return;
      fail(e);
    }
  };

  const notNow = button("Not now", "btn", async () => {
    if (!uploadWasOn) await invoke("upload_set_enabled", { on: false }).catch(() => {});
    finish();
  });

  const linkBtn = button("Link my account", "btn primary", linkNow);
  foot.append(backBtn, linkBtn, notNow);
}
