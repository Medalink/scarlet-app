// The toasts for feats a character earned or upgraded since the player last saw them: one at a time, bottom right.
// A feat is reported as seen when its toast shows, never before; with `local` there is nothing to report to.

import { invoke, el } from "./lib.js";
import { BY_KEY } from "./feats-catalog.js";
import { crest, tierChip } from "./featcard.js";

const MAX_TOASTS = 5;
const STAY_MS = 4800;
const GAP_MS = 300;
const LEAVE_MS = 400;
const ACCOUNT = "\u0000account";

const shown = new Map();
let stopCurrent = null;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const seenKey = (owner, key) => `${owner}\n${key}`;
const ownerOf = (character, n) => (n.account ? ACCOUNT : character);

/** Ends the toasts now on screen and the ones waiting; what already showed stays seen. */
export function stopToasts() {
  stopCurrent?.();
  stopCurrent = null;
}

const markSeen = (items, character, local) => {
  const groups = new Map();
  for (const n of items) {
    const owner = ownerOf(character, n);
    shown.set(seenKey(owner, n.key), n.tier);
    groups.set(owner, { ...groups.get(owner), [n.key]: n.tier });
  }
  if (local) return;
  for (const [owner, tiers] of groups) {
    const args = owner === ACCOUNT ? { character: "", account: true, shown: tiers } : { character: owner, shown: tiers };
    invoke("feats_seen", args).catch(() => {});
  }
};

function featToast(n) {
  const upgraded = n.from > 0;
  const scope = n.account ? "account" : (n.crest ?? BY_KEY.get(n.key)?.crest ?? "character");
  const lead = el("span", "toast-crest");
  const mark = crest(n.icon, n.tier, { size: 64, progress: n.progress ?? 0, scope, state: "earned", label: `${n.name}, ${n.tier_name}` });
  lead.append(el("span", "toast-glow"), mark);
  const subject = n.account ? "Account feat" : "Feat";
  const label = el("p", "toast-l");
  label.textContent = upgraded ? `${subject} upgraded from ${n.from_name}` : `${subject} earned`;
  const name = el("p", "toast-n");
  name.append(document.createTextNode(n.name), tierChip(n.tier, n.tier_name));
  const text = el("div", "toast-text");
  text.append(label, name);
  if (n.counted) {
    const count = el("p", "toast-c");
    count.textContent = n.counted;
    text.append(count);
  }
  return { lead, text, tier: n.tier };
}

function moreToast(rest, viewAll) {
  const lead = el("span", "toast-more");
  lead.textContent = `+${rest.length}`;
  const name = el("p", "toast-n");
  name.textContent = `${rest.length} more ${rest.length === 1 ? "feat" : "feats"}`;
  const button = el("button", "sc-btn quiet small", "View all");
  button.type = "button";
  button.onclick = viewAll;
  const text = el("div");
  text.append(name, button);
  return { lead, text, tier: 2, button };
}

/**
 * Plays the toasts for `news` (the website's `new` list of one character, with the account's entries flagged
 * `account`) and calls `viewAll` when the player picks "View all" on the last one. A feat shown in this run of the
 * app is not shown again, whatever the website says.
 */
export function playToasts(character, news, viewAll, { local = false } = {}) {
  stopToasts();
  const fresh = news.filter((n) => (shown.get(seenKey(ownerOf(character, n), n.key)) ?? -1) < n.tier);
  if (!fresh.length) return;
  const queue = fresh.slice(0, MAX_TOASTS).map((n) => ({ feat: n }));
  const rest = fresh.slice(MAX_TOASTS);
  if (rest.length) queue.push({ rest });

  const host = el("section", "toasts");
  host.setAttribute("role", "region");
  host.setAttribute("aria-label", "New feats");
  document.body.append(host);
  let alive = true;
  const mine = () => {
    alive = false;
    host.remove();
  };
  stopCurrent = mine;

  const show = (item) =>
    new Promise((done) => {
      const parts = item.feat ? featToast(item.feat) : moreToast(item.rest, () => {
        leave();
        viewAll();
      });
      const toast = el("div", `toast t${parts.tier}`);
      toast.setAttribute("role", "status");
      host.append(toast);

      const close = el("button", "toast-x", "×");
      close.type = "button";
      close.setAttribute("aria-label", "Dismiss");
      close.onclick = () => leave();

      let left = STAY_MS;
      let began = 0;
      let timer = 0;
      let hovered = false;
      let focused = false;
      let gone = false;
      const begin = () => {
        if (gone || timer) return;
        began = performance.now();
        timer = setTimeout(leave, left);
      };
      const halt = () => {
        if (!timer) return;
        clearTimeout(timer);
        timer = 0;
        left -= performance.now() - began;
      };
      function leave() {
        if (gone) return;
        gone = true;
        clearTimeout(timer);
        toast.classList.add("out");
        setTimeout(() => {
          toast.remove();
          done();
        }, LEAVE_MS);
      }
      toast.addEventListener("mouseenter", () => {
        hovered = true;
        halt();
      });
      toast.addEventListener("mouseleave", () => {
        hovered = false;
        if (!focused) begin();
      });
      toast.addEventListener("focusin", () => {
        focused = true;
        halt();
      });
      toast.addEventListener("focusout", (e) => {
        if (toast.contains(e.relatedTarget)) return;
        focused = false;
        if (!hovered) begin();
      });

      requestAnimationFrame(() => {
        if (!alive || gone) return;
        toast.append(parts.lead, parts.text, close);
      });
      begin();
      markSeen(item.feat ? [item.feat] : item.rest, character, local);
    });

  (async () => {
    for (const item of queue) {
      if (!alive) return;
      await show(item);
      if (!alive) return;
      await sleep(GAP_MS);
    }
    if (stopCurrent === mine) stopCurrent = null;
    mine();
  })();
}
