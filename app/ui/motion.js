// The entrance of a page: numbers count up from 0 and the CSS under `.play` draws bars and routes in. Same timing as the website.

const DURATION = 900;
const MIN_COUNT = 10;
const formats = new WeakMap();

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
 * and each node marked with `counted` counts up. Nothing changes under reduced motion.
 */
export function play(root) {
  root.classList.remove("play");
  if (reducedMotion()) return;
  requestAnimationFrame((now) => {
    root.classList.add("play");
    for (const node of root.querySelectorAll("[data-count]")) countUp(node, now);
  });
}
