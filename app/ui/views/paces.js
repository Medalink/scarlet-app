// The pace tiles of one run: experience, gold, kills and gathers an hour against the character's own history. Drawn
// from the `paces` the backend sends for a session page and for the character's latest run.

import { num, dur, money, moneyHtml, tiles, fillRow, pace, state } from "../lib.js";

/** Makes `name` the selected character, so the run a record opens is drawn as that character's. */
export const openAs = (name) => () => {
  const select = document.querySelector("#char");
  if (select) select.value = name;
  state.ch = name;
};

const fixed = (v) => (Math.abs(v) >= 10 ? num(v) : String(Math.round(v * 10) / 10));

const PACES = [
  ["xp_h", "Experience", { format: num, text: num }, (n) => `${num(n)} XP`],
  ["gold_h", "Gold", { format: (v) => moneyHtml(v, true), text: money }, money],
  ["kills_h", "Kills", { format: fixed, text: fixed }, (n) => `${num(n)} kills`],
  ["gathers_h", "Gathers", { format: fixed, text: fixed }, (n) => `${num(n)} gathers`],
  ["quests_h", "Quests", { format: fixed, text: fixed }, (n) => `${num(n)} ${Math.round(n) === 1 ? "quest" : "quests"}`],
  ["deaths_h", "Deaths", { format: fixed, text: fixed }, (n) => `${num(n)} ${Math.round(n) === 1 ? "death" : "deaths"}`],
];

export const SESSION_PACES = ["xp_h", "gold_h", "kills_h", "gathers_h"];

export const rateText = (key, v) => PACES.find((p) => p[0] === key)?.[2].text(v) ?? String(v);

export const paceName = (key) => PACES.find((p) => p[0] === key)?.[1] ?? key;

export const levelsOf = (band) => (band ? ` for levels ${band[0]}–${band[1]}` : "");

/**
 * The tiles for the rates `paces` holds, with each total over `played` seconds under its rate; null when there are
 * none. `keys` picks the rates drawn, in the order of PACES, laid out `cols` across. `totals` replaces a key's total line. `onOpen` runs when a record's link is followed.
 */
export function paceTiles(paces, played, { totals = {}, onOpen = null, keys = SESSION_PACES, cols = 0 } = {}) {
  const list = [];
  for (const [key, label, how, total] of PACES) {
    const p = keys.includes(key) ? paces.find((x) => x.key === key) : null;
    if (!p) continue;
    const record = p.record ? { ...p.record, href: `#sessions/${p.record.from}-${p.record.to}` } : null;
    const line = totals[key] ?? `${total((p.rate * played) / 3600)} in ${dur(played)}`;
    list.push(pace(p.rate, p.spread, { label, total: line, best: p.isBest, newBest: p.newBest, peak: p.best, peers: p.peers, qualifies: p.qualifies, band: p.band, record, onOpen, ...how }));
  }
  const across = cols || list.length;
  return list.length ? tiles(fillRow(list, across), across) : null;
}
