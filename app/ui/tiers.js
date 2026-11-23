// Every tier name and colour of the app, in one table. The crest drawer reads `LOOKS`, the stylesheet reads the
// `--tier-1` to `--tier-5` tokens this file sets on the document root, and the views read `TIER_NAMES`.

export const INK = "#141416";

const NONE = { name: "Not earned", lead: "rgba(255,255,255,0.22)", glyph: "#8a8a93", field: null, fieldOpacity: 0, side: null, rays: null, gem: null };
const plain = (name, lead, fieldOpacity) => ({ name, lead, glyph: lead, field: lead, fieldOpacity, side: null, rays: null, gem: null });

/**
 * One row for each tier, 0 to 5. `lead` is the colour of the rim, crown, banner, chip and next-piece ghost; `field` and
 * `fieldOpacity` fill the shield; `glyph` is the glyph's ink; `side`, `rays` and `gem` are the pieces only the upper
 * tiers wear.
 */
export const LOOKS = [
  NONE,
  plain("Copper", "#c58a63", 0.16),
  plain("Silver", "#c3c7cf", 0.2),
  plain("Gold", "#d9b25f", 0.24),
  { name: "Scarlet", lead: "#ff2d55", glyph: INK, field: "#ff2d55", fieldOpacity: 1, side: "#c0102e", rays: null, gem: null },
  { name: "Legend", lead: "#d9b25f", glyph: INK, field: "#ff2d55", fieldOpacity: 1, side: "#d9b25f", rays: "#ff2d55", gem: "#ff2d55" },
];

export const TIER_NAMES = LOOKS.map((look) => look.name);
export const METAL = LOOKS.map((look, tier) => (tier ? look.lead : null));

LOOKS.forEach((look, tier) => {
  if (tier) document.documentElement.style.setProperty(`--tier-${tier}`, look.lead);
});
