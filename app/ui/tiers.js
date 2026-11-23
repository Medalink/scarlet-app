// Every tier name and colour of the app, in one table (CRESTS.md, Tiers). The crest drawer reads `QUALITY`, the
// stylesheet reads the `--tier-1` to `--tier-5` and `--tier-N-on` tokens this file sets on the document root, and the
// views read `TIER_NAMES`.

export const INK = "#141416";
export const LIGHT = "#f0f1f3";

/**
 * One row for each tier, 0 to 5 (0 is not earned). `color` is the field, crown and banner, and the chip, bar and
 * toast accent; `rim` the shield's rim and the account ring; `field` the field's opacity (1 is the solid field);
 * `plain` the shield's opacity under 22 px; `glyph` the glyph's stroke; `side`, `rays` and `gem` the pieces only the
 * upper tiers wear; `ghost` and `near` the opacity of a ghost in this colour, at rest and from 90%; `on` the text on a
 * solid chip of this colour.
 */
export const QUALITY = [
  { name: "Poor", color: "#9d9d9d", rim: "#9d9d9d", field: 0, plain: 0, glyph: "#9d9d9d", side: null, rays: null, gem: null, ghost: 0, near: 0, on: INK },
  { name: "Uncommon", color: "#1eff00", rim: "#1eff00", field: 0.12, plain: 0.55, glyph: LIGHT, side: null, rays: null, gem: null, ghost: 0.12, near: 0.3, on: INK },
  { name: "Rare", color: "#0070dd", rim: "#0070dd", field: 0.3, plain: 1, glyph: LIGHT, side: null, rays: null, gem: null, ghost: 0.3, near: 0.55, on: LIGHT },
  { name: "Epic", color: "#a335ee", rim: "#a335ee", field: 0.3, plain: 1, glyph: LIGHT, side: null, rays: null, gem: null, ghost: 0.3, near: 0.55, on: LIGHT },
  { name: "Legendary", color: "#ff8000", rim: "#ff8000", field: 1, plain: 1, glyph: INK, side: "#b85c00", rays: null, gem: null, ghost: 0.16, near: 0.36, on: INK },
  { name: "Scarlet", color: "#ff2d55", rim: LIGHT, field: 1, plain: 1, glyph: INK, side: "#c0102e", rays: LIGHT, gem: LIGHT, ghost: 0.14, near: 0.34, on: INK },
];

/** The name a surface prints for a tier. A crest without a tier is "Not earned", never "Poor". */
export const TIER_NAMES = QUALITY.map((q, tier) => (tier ? q.name : "Not earned"));

QUALITY.forEach((q, tier) => {
  if (!tier) return;
  document.documentElement.style.setProperty(`--tier-${tier}`, q.color);
  document.documentElement.style.setProperty(`--tier-${tier}-on`, q.on);
});
