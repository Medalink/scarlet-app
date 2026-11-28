// The tier names and colours of the app come from crest-kit.js. The views read `TIER_NAMES` (a crest without a tier is
// "Not earned", never "Poor"); the stylesheet reads the `--tier-1` to `--tier-5` and `--tier-N-on` tokens this file sets
// on the document root.

import { QUALITY, TIER_NAMES } from "./crest-kit.js";

export { QUALITY, TIER_NAMES };

QUALITY.forEach((q, tier) => {
  if (!tier) return;
  document.documentElement.style.setProperty(`--tier-${tier}`, q.color);
  document.documentElement.style.setProperty(`--tier-${tier}-on`, q.on);
});

export const CLASS_COLOR = {
  DEATHKNIGHT: "#E04A64",
  DEMONHUNTER: "#C46CE6",
  DRUID: "#FF8A2B",
  EVOKER: "#4AB8A2",
  HUNTER: "#AAD372",
  MAGE: "#5CC8EE",
  MONK: "#00FF98",
  PALADIN: "#F48CBA",
  PRIEST: "#F0F1F3",
  ROGUE: "#FFF468",
  SHAMAN: "#4A94FF",
  WARLOCK: "#A79BF5",
  WARRIOR: "#C69B6D",
};
