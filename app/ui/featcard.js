// A feat's crest, tier chip and card (CRESTS.md). Everything from the website is set as text.

import { el, num } from "./lib.js";
import { BY_KEY } from "./feats-catalog.js";
import { METAL, TIER_NAMES, svg } from "./glyphs.js";
import { drawCrest } from "./crests.js";
import { hydrate } from "./views/classblocks.js";

/**
 * The crest of a feat: the shield says the scope, the glyph what it counts, the pieces the tier held and the dim next
 * piece, filled by `progress`, how far the next tier is. `state: "earned"` plays the attach of the held tier's piece.
 */
export function crest(name, tier, { size = 64, progress = 0, scope = "character", state = "", icon = "", label = "" } = {}) {
  const host = el("span");
  Object.assign(host.dataset, { crest: name, scope, tier, progress, size, label });
  if (state) host.dataset.state = state;
  if (icon) host.dataset.icon = icon;
  drawCrest(host);
  if (icon) hydrate(host);
  return host;
}

/** The five tier pips alone, for a collection's tier. */
export function pips(tier) {
  const host = el("span", "sc-pips");
  const root = svg("svg", {
    viewBox: "0 0 46 10", width: 46, height: 10, role: "img",
    "aria-label": tier ? `${TIER_NAMES[tier]} collection` : "No tier yet",
  });
  for (let i = 0; i < 5; i++) {
    root.append(
      svg("rect", {
        x: 2.5 + i * 9, y: 2, width: 6, height: 6, transform: `rotate(45 ${5.5 + i * 9} 5)`,
        fill: i < tier ? METAL[tier] : "none", stroke: i < tier ? "none" : "rgba(255,255,255,0.22)", "stroke-width": 1,
      }),
    );
  }
  host.append(root);
  return host;
}

/** The small tier name beside a feat's name. */
export function tierChip(tier, text) {
  const c = el("small", `sc-chip t${tier}`);
  c.textContent = text;
  return c;
}

const unitValue = (cat, n) => (cat.unit === "mi" ? n / 1760 : n);

/** A count in the catalog's unit: yards of a mile feat become miles. */
export const amount = (cat, n) => {
  const v = unitValue(cat, n);
  return Number.isInteger(v) || v >= 10 ? num(v) : v.toFixed(1);
};

/** "9,912 kills", from the catalog's own words for one and many. */
export const countText = (cat, value) => `${amount(cat, value)} ${unitValue(cat, value) === 1 ? cat.one : cat.many}`;

/** A website card with the catalog's glyph, family, collection and crest scope filled in where the answer has none. */
export function withCatalog(feat) {
  const c = BY_KEY.get(feat.key);
  if (!c) return feat;
  return { ...feat, glyph: feat.glyph ?? c.glyph, family: feat.family ?? c.family, collection: feat.collection ?? c.collection, thresholds: feat.thresholds ?? c.thresholds, crest: feat.crest ?? c.crest };
}

const cardProgress = (feat) => {
  if (feat.progress != null) return feat.progress;
  if (Array.isArray(feat.track)) return Number(feat.track[feat.tier] ?? 0) / 100;
  if (feat.next != null && feat.value != null) return (feat.value - (feat.prev ?? 0)) / (feat.next - (feat.prev ?? 0));
  return 0;
};

/** The crest of a card: its glyph, scope and progress, whichever shape the feat arrives in. */
export const cardCrest = (feat, size = 64) =>
  crest(feat.glyph ?? feat.icon, feat.tier, {
    size, progress: cardProgress(feat), scope: feat.crest ?? "character", icon: feat.art ?? "", label: `${feat.name}, ${feat.tier_name || TIER_NAMES[feat.tier]}`,
  });

/**
 * One feat card: crest 64, the name with its tier chip, the count in the feat's words and what the next tier takes.
 * `share` shows at the right when the website sends it.
 */
export function card(raw, { share = true } = {}) {
  const feat = withCatalog(raw);
  const li = el("li", `sc-feat t${feat.tier}`);
  const name = el("p", "sc-feat-n");
  name.append(document.createTextNode(feat.name));
  if (feat.tier && (feat.tier_name || TIER_NAMES[feat.tier])) name.append(tierChip(feat.tier, feat.tier_name || TIER_NAMES[feat.tier]));
  if (share && feat.share != null) {
    const r = el("span", "sc-feat-r");
    const at = TIER_NAMES[Math.max(1, feat.tier || 1)];
    r.textContent = `${feat.share < 1 ? "<1" : num(feat.share)}% hold ${at} or higher`;
    name.append(r);
  }
  const text = el("div");
  text.append(name);
  const counted = feat.counted ?? feat.detail;
  if (counted) {
    const d = el("p", "sc-feat-d");
    d.textContent = counted;
    text.append(d);
  }
  if (feat.to_next) {
    const t = el("p", "sc-feat-to");
    t.textContent = feat.to_next;
    text.append(t);
  }
  li.append(cardCrest(feat), text);
  return li;
}

/** The cards as a grid list; `top` gives the three-column grid of the closest. */
export function cardList(feats, top = false) {
  const ul = el("ul", top ? "feats feats-top" : "feats");
  for (const feat of feats) ul.append(card(feat));
  return ul;
}

/** "54 of 70 feats" and the count at each tier. */
export function summary(entry) {
  const p = el("p", "feats-sum");
  const part = (count, label) => {
    const span = el("span");
    const b = el("b");
    b.textContent = String(count);
    span.append(b, document.createTextNode(` ${label}`));
    return span;
  };
  p.append(part(entry.held, `of ${entry.total} feats`));
  for (const t of entry.tiers) p.append(part(t.count, t.name));
  return p;
}
