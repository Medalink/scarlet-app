// Every tier name, colour and material of the app, in one place (CRESTS.md, Tiers). The crest drawer reads `QUALITY` and
// `MATERIAL`, the stylesheet reads the `--tier-1` to `--tier-5` and `--tier-N-on` tokens this file sets on the document
// root, and the views read `TIER_NAMES`.

export const INK = "#141416";
export const LIGHT = "#f0f1f3";

/**
 * The neutral materials a crest is made of, each lit from above: `top` and `bottom` are the ends of a vertical gradient
 * across a piece's box, and `flat` is the one tint of a surface that draws no gradient and of a ghost.
 */
export const MATERIAL = {
  steel: { top: "#e6e9ef", bottom: "#767c88", flat: "#aeb4bf" },
  pewter: { top: "#a9aeb9", bottom: "#4b4f58", flat: "#787d89" },
  parchment: { top: "#f3ead6", bottom: "#c2b391", flat: "#ddd0b3" },
  gold: { top: "#ffeaa6", bottom: "#b8872a", flat: "#e8c060" },
};

/**
 * One row for each tier, 0 to 5 (0 is not earned). `color` is the accent: the enamel field's top, the inlay, the
 * banner's band, and the chip, bar and toast accent; `deep` the foot of the field; `frame`, `crown`, `banner`, `side`
 * and `rays` the `MATERIAL` each piece is made of; `field` the field's opacity (1 is the solid field); `plain` the
 * shield's opacity under 22 px; `glyph` the glyph's stroke; `gem` the gem on the crown; `ghost` and `near` the opacity
 * of a ghost in this colour, at rest and from 90%; `on` the text on a solid chip of this colour.
 */
export const QUALITY = [
  { name: "Poor", color: "#9d9d9d", deep: null, frame: null, field: 0, plain: 0, glyph: "#9d9d9d", crown: null, banner: null, side: null, rays: null, gem: null, ghost: 0, near: 0, on: INK },
  { name: "Uncommon", color: "#1eff00", deep: "#14b800", frame: "steel", field: 0.22, plain: 0.55, glyph: LIGHT, crown: null, banner: null, side: null, rays: null, gem: null, ghost: 0.12, near: 0.3, on: INK },
  { name: "Rare", color: "#0070dd", deep: "#004f9e", frame: "steel", field: 0.45, plain: 1, glyph: LIGHT, crown: "steel", banner: null, side: null, rays: null, gem: "#0070dd", ghost: 0.3, near: 0.55, on: LIGHT },
  { name: "Epic", color: "#a335ee", deep: "#7220b0", frame: "steel", field: 0.45, plain: 1, glyph: LIGHT, crown: "steel", banner: "parchment", side: null, rays: null, gem: "#a335ee", ghost: 0.3, near: 0.55, on: LIGHT },
  { name: "Legendary", color: "#ff8000", deep: "#c75a00", frame: "steel", field: 1, plain: 1, glyph: INK, crown: "steel", banner: "parchment", side: "pewter", rays: null, gem: "#ff8000", ghost: 0.16, near: 0.36, on: INK },
  { name: "Scarlet", color: "#ff2d55", deep: "#c4123a", frame: "gold", field: 1, plain: 1, glyph: INK, crown: "gold", banner: "parchment", side: "pewter", rays: "gold", gem: "#ff2d55", ghost: 0.14, near: 0.34, on: INK },
];

/** The name a surface prints for a tier. A crest without a tier is "Not earned", never "Poor". */
export const TIER_NAMES = QUALITY.map((q, tier) => (tier ? q.name : "Not earned"));

QUALITY.forEach((q, tier) => {
  if (!tier) return;
  document.documentElement.style.setProperty(`--tier-${tier}`, q.color);
  document.documentElement.style.setProperty(`--tier-${tier}-on`, q.on);
});
