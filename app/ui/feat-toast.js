// The toasts for feats a character earned or upgraded since the player last saw them: one at a time, bottom right.
// A feat is reported as seen when its toast shows, never before; with `local` there is nothing to report to.

import { invoke, el } from "./lib.js";
import { seal, tierChip, dial } from "./featcard.js";

const MAX_TOASTS = 5;
const STAY_MS = 4800;
const GAP_MS = 300;
const LEAVE_MS = 400;
const TIER_SWAP_MS = 280;
const DIAL_FULL_MS = 900;
const DIAL_SETTLE_MS = 1200;

const shown = new Map();
let stopCurrent = null;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const seenKey = (character, key) => `${character}\n${key}`;

/** Ends the toasts now on screen and the ones waiting; what already showed stays seen. */
export function stopToasts() {
  stopCurrent?.();
  stopCurrent = null;
}

const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

const markSeen = (character, entries, local) => {
  for (const [key, tier] of entries) shown.set(seenKey(character, key), tier);
  if (!local) invoke("feats_seen", { character, shown: Object.fromEntries(entries) }).catch(() => {});
};

function featToast(n) {
  const upgraded = n.from > 0;
  const progress = n.progress ?? 0;
  const plate = (tier) => seal(n.icon, tier, { size: 64, progress: 0, family: n.family });
  const lead = el("span", "toast-seal");
  const mark = plate(upgraded ? n.from : n.tier);
  lead.append(el("span", "toast-glow"), mark);
  if (upgraded) mark.querySelectorAll(".pip").forEach((p, i) => i < n.from && p.classList.add("was"));
  const label = el("p", "toast-l");
  label.textContent = upgraded ? `Upgraded from ${n.from_name}` : "Feat earned";
  const name = el("p", "toast-n");
  name.append(document.createTextNode(n.name), tierChip(n.tier, n.tier_name));
  const text = el("div", "toast-text");
  text.append(label, name);
  if (n.counted) {
    const count = el("p", "toast-c");
    count.textContent = n.counted;
    text.append(count);
  }
  const swap = () => {
    if (reduced()) {
      mark.replaceChildren(plate(n.tier).firstChild);
      mark.className = `sc-seal t${n.tier}`;
      dial(mark, n.tier, progress);
      return;
    }
    if (upgraded) {
      setTimeout(() => {
        mark.replaceChildren(plate(n.tier).firstChild);
        mark.className = `sc-seal t${n.tier}`;
      }, TIER_SWAP_MS);
    }
    setTimeout(() => dial(mark, n.tier, 1), DIAL_FULL_MS);
    setTimeout(() => dial(mark, n.tier, progress), DIAL_SETTLE_MS);
  };
  return { lead, text, tier: n.tier, swap };
}

function moreToast(rest, viewAll) {
  const lead = el("span", "sc-seal t2");
  lead.textContent = `+${rest.length}`;
  const name = el("p", "toast-n");
  name.textContent = `${rest.length} more ${rest.length === 1 ? "feat" : "feats"}`;
  const button = el("button", "sc-btn quiet small", "View all");
  button.type = "button";
  button.onclick = viewAll;
  const text = el("div");
  text.append(name, button);
  return { lead, text, tier: 2, swap: null, button };
}

/**
 * Plays the toasts for `news` (the website's `new` list of one character) and calls `viewAll` when the player picks
 * "View all" on the last one. A feat shown in this run of the app is not shown again, whatever the website says.
 */
export function playToasts(character, news, viewAll, { local = false } = {}) {
  stopToasts();
  const fresh = news.filter((n) => (shown.get(seenKey(character, n.key)) ?? -1) < n.tier);
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
        parts.swap?.();
      });
      begin();
      markSeen(
        character,
        item.feat ? [[item.feat.key, item.feat.tier]] : item.rest.map((n) => [n.key, n.tier]),
        local,
      );
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
