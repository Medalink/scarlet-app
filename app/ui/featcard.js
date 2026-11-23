// A feat's seal, tier chip and card (FEATS.md). Everything from the website is set as text.

import { el, num } from "./lib.js";
import { BY_KEY } from "./feats-catalog.js";
import { GLYPHS, METAL, NEXT, PLATES, PLATE_SHIFT, FAMILY_OF, TIER_NAMES, svg } from "./glyphs.js";

const GOLD = "#d9b25f";
const SCARLET = "#ff2d55";

export const familyOf = (name, family) => family || FAMILY_OF[name] || (String(name).startsWith("m-") ? "class" : "journey");

/**
 * The seal of a feat: the plate's shape is the family, its metal the tier, the pips count the tier and the dial of
 * 20 ticks lights toward the next tier. `ghost` draws the plate as a dashed outline in that tier's metal.
 */
export function seal(name, tier, { size = 64, progress = 0, family, ghost = 0, label = "" } = {}) {
  const host = el("span", `sc-seal t${tier}`);
  const family_ = familyOf(name, family);
  const root = svg("svg", { viewBox: "0 0 64 64", width: size, height: size, role: "img", "aria-label": label || null });
  const metal = METAL[ghost || tier] || "rgba(255,255,255,0.22)";
  const full = size >= 44;
  const tiny = size < 22;
  const k = full ? 1 : 1.36;
  const plate = (scale, attrs) => svg("path", { d: PLATES[family_], transform: `translate(32 32) scale(${scale * k}) translate(-32 -32)`, ...attrs });
  if (full) {
    const lit = tier === 5 ? 20 : Math.round(progress * 20);
    for (let i = 0; i < 20; i++) {
      const a = ((222 + (276 / 19) * i) * Math.PI) / 180;
      const on = i < lit;
      const sx = Math.sin(a);
      const cy = -Math.cos(a);
      root.append(
        svg("line", {
          x1: 32 + sx * 27.4, y1: 32 + cy * 27.4, x2: 32 + sx * 31, y2: 32 + cy * 31,
          stroke: on ? NEXT[tier] : "rgba(255,255,255,0.16)", "stroke-width": on ? 2.3 : 1.4, class: "tick", style: `--i:${i}`,
        }),
      );
    }
    for (let i = 0; i < 5; i++) {
      const a = ((201 - 10.5 * i) * Math.PI) / 180;
      const x = 32 + Math.sin(a) * 29.4;
      const y = 32 - Math.cos(a) * 29.4;
      const on = i < tier;
      root.append(
        svg("rect", {
          x: x - 1.8, y: y - 1.8, width: 3.6, height: 3.6, transform: `rotate(45 ${x} ${y})`,
          fill: on ? (tier === 5 ? GOLD : METAL[tier]) : "none", stroke: on ? "none" : "rgba(255,255,255,0.22)", "stroke-width": 0.8, class: on ? "pip on" : "pip", style: `--i:${i}`,
        }),
      );
    }
  }
  const body = svg("g", { class: "plate" });
  const s = full ? 0.9 : 1;
  if (tiny) body.append(plate(1, { fill: tier ? (tier >= 4 ? SCARLET : metal) : "none", stroke: tier === 5 ? GOLD : tier ? "none" : "rgba(255,255,255,0.3)", "stroke-width": 3 }));
  else if (ghost) body.append(plate(s, { fill: "none", stroke: metal, "stroke-width": full ? 1.6 : 2.2, "stroke-dasharray": "3.5 3" }));
  else if (tier === 0) body.append(plate(s, { fill: "none", stroke: "rgba(255,255,255,0.22)", "stroke-width": full ? 1.4 : 2 }));
  else if (tier < 4) {
    body.append(plate(s, { fill: metal, "fill-opacity": [0, 0.14, 0.18, 0.22][tier], stroke: metal, "stroke-width": full ? 1.6 : 2.2 }));
    if (tier === 3) body.append(plate(s * 0.85, { fill: "none", stroke: metal, "stroke-width": full ? 0.9 : 1.2, opacity: 0.75 }));
  } else {
    body.append(plate(s, { fill: "none", stroke: tier === 5 ? GOLD : SCARLET, "stroke-width": full ? (tier === 5 ? 2.1 : 1.6) : 2.6 }));
    body.append(plate(s * 0.84, { fill: SCARLET }));
  }
  if (!tiny) {
    const ink = ghost ? metal : tier >= 4 ? "#141416" : tier ? metal : "#8a8a93";
    const gs = (full ? 0.9 : 1.2) * (String(name).startsWith("m-") ? 0.92 : 1);
    const dy = (PLATE_SHIFT[family_] || 0) * (full ? 0.9 : 1.2);
    const g = svg("g", {
      transform: `translate(${32 - 12 * gs} ${32 - 12 * gs + dy}) scale(${gs})`, fill: "none", stroke: ink,
      "stroke-width": tier >= 4 ? 2 : 1.75, "stroke-linecap": "round", "stroke-linejoin": "round", opacity: tier || ghost ? 1 : 0.75,
    });
    g.innerHTML = (String(name).startsWith("m-") ? '<rect x="1.2" y="1.2" width="21.6" height="21.6" rx="2.6" stroke-width="1" opacity="0.5"/>' : "") + (GLYPHS[name] || "");
    body.append(g);
  }
  root.append(body);
  host.append(root);
  return host;
}

/** Lights the dial of a seal for `progress` (0 to 1) toward the next tier, ticks left to right. */
export function dial(host, tier, progress) {
  const lit = tier === 5 ? 20 : Math.round(progress * 20);
  host.querySelectorAll(".tick").forEach((tick, i) => {
    const on = i < lit;
    tick.setAttribute("stroke", on ? NEXT[tier] : "rgba(255,255,255,0.16)");
    tick.setAttribute("stroke-width", on ? 2.3 : 1.4);
  });
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
        fill: i < tier ? (tier === 5 ? GOLD : METAL[tier]) : "none", stroke: i < tier ? "none" : "rgba(255,255,255,0.22)", "stroke-width": 1,
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

/** A website card with the catalog's glyph, family and collection filled in where the answer has none. */
export function withCatalog(feat) {
  const c = BY_KEY.get(feat.key);
  if (!c) return feat;
  return { ...feat, glyph: feat.glyph ?? c.glyph, family: feat.family ?? c.family, collection: feat.collection ?? c.collection, thresholds: feat.thresholds ?? c.thresholds };
}

const cardProgress = (feat) => feat.progress ?? (Array.isArray(feat.track) ? Number(feat.track[feat.tier] ?? 0) / 100 : 0);

/** The seal of a card: its glyph, family and progress, whichever shape the feat arrives in. */
export const cardSeal = (feat, size = 64) =>
  seal(feat.glyph ?? feat.icon, feat.tier, { size, progress: cardProgress(feat), family: feat.family, label: `${feat.name}, ${feat.tier_name || TIER_NAMES[feat.tier]}` });

/**
 * One feat card: seal 64, the name with its tier chip, the count in the feat's words and what the next tier takes.
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
  const round = cardSeal(feat);
  if (feat.art) {
    const art = el("img", "seal-art");
    art.alt = "";
    art.dataset.icon = feat.art;
    round.append(art);
  }
  li.append(round, text);
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
