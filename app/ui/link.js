// Where the app stands with the website: one line in the sidebar foot and one hideable row at the end of a few views.

import { invoke, linkRow } from "./lib.js";

const HIDDEN_KEY = "scarlet.linkRowHidden";
const ASK_MS = 30_000;
let asked = null;

const hostOf = (server) => String(server).replace(/^https?:\/\//, "").replace(/\/+$/, "");

export async function webState() {
  let view;
  try {
    view = await invoke("upload_get");
  } catch {
    return { state: "off", host: "" };
  }
  const host = hostOf(view.server);
  if (!view.enabled || view.problem) return { state: "off", host };
  if (!asked || asked.server !== view.server || Date.now() - asked.at > ASK_MS) {
    const linked = await invoke("upload_linked").catch(() => false);
    asked = { server: view.server, at: Date.now(), linked };
  }
  return { state: asked.linked ? "linked" : "unlinked", host };
}

export const forgetLink = () => {
  asked = null;
};

export function footLine({ state, host }) {
  const text = state === "linked" ? `Linked to ${host}` : state === "unlinked" ? "Not linked" : "Uploading is off";
  return { text, linked: state === "linked" };
}

function hiddenViews() {
  try {
    return JSON.parse(localStorage.getItem(HIDDEN_KEY)) ?? [];
  } catch {
    return [];
  }
}

function hide(view) {
  try {
    localStorage.setItem(HIDDEN_KEY, JSON.stringify([...new Set([...hiddenViews(), view])]));
  } catch {}
}

export async function linkPrompt(view) {
  if (hiddenViews().includes(view)) return null;
  if ((await webState()).state !== "unlinked") return null;
  const row = linkRow(
    "Link this app to your account to pin feats and choose what is public.",
    "Link",
    async () => {
      await invoke("upload_link").catch(() => {});
      forgetLink();
    },
    {
      hide: () => {
        hide(view);
        row.remove();
      },
    },
  );
  return row;
}
