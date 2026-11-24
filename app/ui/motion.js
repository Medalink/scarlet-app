// The entrance of a page: numbers count up from 0 and the CSS under `.play` draws bars and routes in. Same timing as the website.

import { state } from "./lib.js";

const DURATION = 900;
const MIN_COUNT = 10;
const formats = new WeakMap();
const queued = new WeakSet();
const timers = new WeakMap();
const played = new Set();
const ENTRANCE_MS = 2000;

window.addEventListener("hashchange", () => played.clear());

export const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * Marks `node` to count up to `value` when `play` runs. `format(n)` returns the node's HTML for the whole number `n`,
 * so units and money are right at every frame. A value under 10 is left as it is.
 */
export function counted(node, value, format) {
  if (Number.isFinite(value) && Math.abs(value) >= MIN_COUNT) {
    node.dataset.count = String(value);
    formats.set(node, format);
  }
  return node;
}

/** Marks every node under `root` carrying `data-to` (the number) and `data-as` (a key of `byName`) with `counted`. */
export function countMarked(root, byName) {
  for (const node of root.querySelectorAll("[data-to]")) counted(node, Number(node.dataset.to), byName[node.dataset.as]);
}

function countUp(node, now) {
  const target = Number(node.dataset.count);
  const format = formats.get(node);
  const final = node.innerHTML;
  const step = (frame) => {
    const t = Math.min(1, (frame - now) / DURATION);
    const eased = 1 - (1 - t) ** 4;
    node.innerHTML = t < 1 ? format(Math.round(target * eased)) : final;
    if (t < 1) requestAnimationFrame(step);
  };
  step(now);
}

/**
 * Plays the entrance of everything under `root` on the next frame, before it is painted: `root` gets the class `play`
 * and each node marked with `counted` counts up, and the class comes off when the entrance is over. A page plays once
 * each time it is opened: a second call with the same `key` and character, such as a redraw after `store-changed` or
 * a return to a character tab, leaves the page as drawn. `key` is the hash unless the page names another. Navigation
 * forgets every key. Nothing changes under reduced motion.
 */
export function play(root, key = location.hash) {
  clearTimeout(timers.get(root));
  root.classList.remove("play");
  const id = `${key}|${state.ch ?? ""}`;
  if (reducedMotion() || queued.has(root) || played.has(id)) return;
  played.add(id);
  queued.add(root);
  requestAnimationFrame((now) => {
    queued.delete(root);
    root.classList.add("play");
    for (const node of root.querySelectorAll("[data-count]")) countUp(node, now);
    timers.set(root, setTimeout(() => root.classList.remove("play"), ENTRANCE_MS));
  });
}
