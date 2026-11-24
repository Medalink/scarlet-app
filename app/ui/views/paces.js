// The pace tiles of one run: experience, gold, kills and gathers an hour against the character's own history. Drawn
// from the `paces` the backend sends for a session page and for the character's latest run.

import { num, dur, money, moneyHtml, tiles, pace } from "../lib.js";

const fixed = (v) => (Math.abs(v) >= 10 ? num(v) : String(Math.round(v * 10) / 10));

const PACES = [
  ["xp_h", "Experience", { format: num, text: num }, (n) => `${num(n)} XP`],
  ["gold_h", "Gold", { format: (v) => moneyHtml(v, true), text: money }, money],
  ["kills_h", "Kills", { format: fixed, text: fixed }, (n) => `${num(n)} kills`],
  ["gathers_h", "Gathers", { format: fixed, text: fixed }, (n) => `${num(n)} gathers`],
];

export const paceName = (key) => PACES.find((p) => p[0] === key)?.[1] ?? key;

/** The tiles for the rates `paces` holds, with each total over `played` seconds under its rate; null when there are none. */
export function paceTiles(paces, played) {
  const list = [];
  for (const [key, label, how, total] of PACES) {
    const p = paces.find((x) => x.key === key);
    if (!p) continue;
    list.push(pace(p.rate, p.spread, { label, total: `${total((p.rate * played) / 3600)} in ${dur(played)}`, best: p.isBest, newBest: p.newBest, peak: p.best, peers: p.peers, qualifies: p.qualifies, ...how }));
  }
  return list.length ? tiles(list, list.length) : null;
}
