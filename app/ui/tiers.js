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
