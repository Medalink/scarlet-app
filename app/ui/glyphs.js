// The line glyphs, the eight family plates and the tier metals every seal is drawn from (FEATS.md).

const NS = "http://www.w3.org/2000/svg";

export const svg = (tag, attrs = {}, kids = []) => {
  const e = document.createElementNS(NS, tag);
  for (const k in attrs) if (attrs[k] != null) e.setAttribute(k, attrs[k]);
  for (const c of [].concat(kids)) if (c) e.append(c);
  return e;
};

export const GLYPHS = {
  characters: '<circle cx="12" cy="8" r="3.6"/><path d="M4.8 20c.7-4 3.5-6.1 7.2-6.1s6.5 2.1 7.2 6.1"/>',
  level: '<path d="M2.5 20 9 9.5l3.6 5.2 2.4-3.4L21.5 20z"/><path d="M9 9.5V3.6l4.2 1.6L9 6.9"/>',
  hours: '<path d="M7 3.5h10M7 20.5h10M8.2 3.5v3.6c0 2.2 3.8 3.1 3.8 4.9s-3.8 2.7-3.8 4.9v3.6M15.8 3.5v3.6c0 2.2-3.8 3.1-3.8 4.9s3.8 2.7 3.8 4.9v3.6"/>',
  kills: '<path d="m4 4 10 10m-2.2 2.2 4.4-4.4M15 15l4.5 4.5M20 4 10 14m2.2 2.2-4.4-4.4M9 15l-4.5 4.5"/>',
  deaths: '<path d="M12 3.2a7.3 7.3 0 0 0-7.3 7.3c0 2.5 1.2 4.4 2.8 5.6v2.7a1 1 0 0 0 1 1h7a1 1 0 0 0 1-1v-2.7c1.6-1.2 2.8-3.1 2.8-5.6A7.3 7.3 0 0 0 12 3.2z"/><circle cx="9.2" cy="11.2" r="1.5"/><circle cx="14.8" cy="11.2" r="1.5"/>',
  quests: '<path d="M6 3.8h11.2v13.6a2.8 2.8 0 0 0 2.8 2.8H8.8A2.8 2.8 0 0 1 6 17.4z"/><path d="M9.6 8.4h4.2M9.6 12h4.2"/>',
  dungeons: '<path d="M4.8 20.4v-9.2a7.2 7.2 0 0 1 14.4 0v9.2M3.2 20.4h17.6M9.6 20.4v-8.6m4.8 8.6v-8.6M12 20.4V8.2"/>',
  gold: '<circle cx="12" cy="12" r="8.4"/><circle cx="12" cy="12" r="4.4"/>',
  zones: '<circle cx="12" cy="12" r="8.6"/><path d="m15.6 8.4-2.1 5.1-5.1 2.1 2.1-5.1z"/>',
  distance: '<circle cx="6" cy="18" r="2.1"/><circle cx="18" cy="6" r="2.1"/><path d="M8.1 18H15a3 3 0 0 0 0-6H9a3 3 0 0 1 0-6h6.9"/>',
  skills: '<path d="m14.2 3.8 6 6-2.6 2.6-6-6zM12.7 8.7 4 17.4l2.6 2.6 8.7-8.7"/>',
  fish: '<path d="M3 12c3.1-4.6 9.2-5.6 13.2-2.3L20.4 7v10l-4.2-2.7C12.2 17.6 6.1 16.6 3 12z"/><path d="M7.8 11.2h.01"/>',
  gather: '<path d="M5 19c0-8.2 5.2-13.2 14.2-14.2C19.2 14 14 19 5 19zm0 0 8.4-8.4"/>',
  jumps: '<path d="M3.5 19c.5-7.2 5-11.6 8.5-11.6s8 4.4 8.5 11.6"/><path d="M12 7.4V3.2m0 0 2.7 2.7M12 3.2 9.3 5.9"/>',
  swim: '<path d="M3.5 12.8c1.7-1.4 3.3-1.4 5 0s3.3 1.4 5 0 3.3-1.4 5 0"/><path d="M3.5 17.8c1.7-1.4 3.3-1.4 5 0s3.3 1.4 5 0 3.3-1.4 5 0"/><circle cx="16.2" cy="6.2" r="2.4"/>',
  emote: '<path d="M4 5h16v10.4h-8.6L7 19.6v-4.2H4z"/><path d="M8.2 10.2h.01M12 10.2h.01M15.8 10.2h.01"/>',
  crown: '<path d="M4 18.6h16M4.6 18.6 3.4 8l5 4.2L12 5l3.6 7.2 5-4.2-1.2 10.6"/>',
  shield: '<path d="m12 3 7.2 3v5.6c0 4.3-3 7.6-7.2 9.4-4.2-1.8-7.2-5.1-7.2-9.4V6z"/>',
  clock: '<circle cx="12" cy="12" r="8.6"/><path d="M12 7v5.2l3.4 2"/>',
  pin: '<path d="M12 21s6.6-6.3 6.6-11.1a6.6 6.6 0 0 0-13.2 0C5.4 14.7 12 21 12 21z"/><circle cx="12" cy="9.9" r="2.3"/>',
  creature: '<path d="M6.5 13.2a5.5 5.5 0 0 1 11 0c0 3.7-2.4 6.6-5.5 6.6s-5.5-2.9-5.5-6.6z"/><path d="M7.6 10.2 4.8 4.6m11.6 5.6 2.8-5.6M10 13.6h.01m3.98 0H14"/>',
  item: '<path d="M8 8.6h8l2.2 9.1a2 2 0 0 1-2 2.5H7.8a2 2 0 0 1-2-2.5z"/><path d="M9 8.6 7.9 4.8h8.2L15 8.6M9.6 12.4h4.8"/>',
  talents: '<circle cx="12" cy="5" r="2.2"/><circle cx="6" cy="18.6" r="2.2"/><circle cx="18" cy="18.6" r="2.2"/><path d="M12 7.2v4.4m0 0-4.9 5.2M12 11.6l4.9 5.2"/>',
  spell: '<path d="m12 3.2 2.1 6.7 6.7 2.1-6.7 2.1-2.1 6.7-2.1-6.7L3.2 12l6.7-2.1z"/>',
  pet: '<circle cx="6.6" cy="10" r="1.9"/><circle cx="12" cy="6.6" r="1.9"/><circle cx="17.4" cy="10" r="1.9"/><path d="M12 12c-2.9 0-5.2 2.4-5.2 4.8 0 1.6 1.2 2.6 2.6 2.6 1 0 1.6-.5 2.6-.5s1.6.5 2.6.5c1.4 0 2.6-1 2.6-2.6 0-2.4-2.3-4.8-5.2-4.8z"/>',
  link: '<path d="M10 14a4.2 4.2 0 0 0 6 0l3-3a4.2 4.2 0 0 0-6-6l-1 1M14 10a4.2 4.2 0 0 0-6 0l-3 3a4.2 4.2 0 0 0 6 6l1-1"/>',
  search: '<circle cx="10.5" cy="10.5" r="6.2"/><path d="m15.2 15.2 5 5"/>',
  arrow: '<path d="M5 12h14m-5-5 5 5-5 5"/>',
  tack: '<path d="M9 3.5h6l-1 5.2 3 3.3H7l3-3.3zM12 12v8.5"/>',
  lock: '<rect x="5.5" y="10.5" width="13" height="9.5" rx="1"/><path d="M8.5 10.5V7.8a3.5 3.5 0 0 1 7 0v2.7"/>',
  check: '<path d="m5 12.5 4.6 4.6L19 7.4"/>',
  fire: '<path d="M12 3.4c.6 3.4 4.8 5.2 4.8 9.8a4.8 4.8 0 0 1-9.6 0c0-1.9.9-3.2 2-4.4.3 1.6 1 2.4 1.8 2.6-.4-2.9.2-5.6 1-8z"/><path d="M5 20.6h14"/>',
  wing: '<path d="M3 15.2c5.6.4 9.6-1.6 12.4-6.2 1.6-2.6 3.4-4 5.6-4.4-.4 3.8-1.8 6.6-4 8.6l-3.4.6 2 1.2c-1.6 1.6-3.8 2.6-6.6 2.8L7 20l-.4-2.4c-1.4-.4-2.6-1.2-3.6-2.4z"/>',
  heal: '<path d="M9.4 4h5.2v5.4H20v5.2h-5.4V20H9.4v-5.4H4V9.4h5.4z"/>',
  "m-bolt": '<path d="M13.6 4.6 7.4 13h4l-1 6.4 6.2-8.4h-4z"/>',
  "m-drop": '<path d="M12 5c2.6 3.2 4.4 5.6 4.4 8.2a4.4 4.4 0 0 1-8.8 0C7.6 10.6 9.4 8.2 12 5z"/><path d="M10.2 13.6a1.9 1.9 0 0 0 1.8 1.8"/>',
  "m-tap": '<path d="M12 18.6c-3.6-2.4-6-4.6-6-7.4a3.2 3.2 0 0 1 6-1.6 3.2 3.2 0 0 1 6 1.6c0 2.8-2.4 5-6 7.4z"/><path d="M12 9.6v4.6m0 0-1.8-1.8m1.8 1.8 1.8-1.8"/>',
  "m-fear": '<path d="M12 5.2a5.6 5.6 0 0 0-5.6 5.6c0 1.9.9 3.3 2.1 4.2v2.4h7v-2.4c1.2-.9 2.1-2.3 2.1-4.2A5.6 5.6 0 0 0 12 5.2z"/><path d="M9.9 11h.01M14.1 11h.01M12 17.4v-1.6"/>',
  "m-soul": '<path d="m12 4.6 4.6 5.6L12 19.4 7.4 10.2z"/><path d="M7.4 10.2h9.2M12 4.6v14.8"/>',
  "m-ritual": '<circle cx="12" cy="12" r="6.4"/><path d="M12 8.2v7.6M8.6 10l6.8 4M15.4 10l-6.8 4"/>',
  "m-agony": '<path d="M6 17.6 17.6 6M14 6h3.6v3.6M6.4 10.4l2.4 2.4M10.4 6.4l2.4 2.4"/>',
  "m-drain": '<path d="M5.4 12h4.2c1.6 0 1.6-4 3.2-4s1.6 8 3.2 8 1.2-4 2.6-4"/>',
  "m-demon": '<path d="M7 6.4c-.6 3 .4 5 2.2 5.8M17 6.4c.6 3-.4 5-2.2 5.8"/><path d="M8.2 12.6a3.8 3.8 0 0 1 7.6 0c0 2.8-1.6 5.4-3.8 5.4s-3.8-2.6-3.8-5.4z"/>',
  "m-funnel": '<path d="M6 6.6h12l-4.4 5.8v5l-3.2-1.6v-3.4z"/>',
  "m-pact": '<path d="M7 5.6h10v9.2L12 18.6 7 14.8z"/><path d="m9.8 10.8 1.6 1.6 3-3.2"/>',
  "m-flame": '<path d="M12 4.8c.5 2.9 4 4.4 4 8.2a4 4 0 0 1-8 0c0-1.6.8-2.7 1.7-3.7.2 1.4.8 2 1.5 2.2-.3-2.4.2-4.7.8-6.7z"/>',
  "m-sear": '<path d="M6.2 15.6c2-1 2-3.2 4-4.2s2 1.6 4 .6 2-3.2 3.6-4.2M6.2 18.4h11.6"/>',
  "m-tree": '<path d="M12 4.6 6.4 12h3L6.8 16.4h10.4L14.6 12h3z"/><path d="M12 16.4v3"/>',
  };

export const METAL = [null, "#c58a63", "#c3c7cf", "#d9b25f", "#ff2d55", "#ff2d55"];
export const NEXT = ["#c58a63", "#c3c7cf", "#d9b25f", "#ff2d55", "#d9b25f", "#d9b25f"];
export const TIER_NAMES = ["Not earned", "Copper", "Silver", "Gold", "Scarlet", "Legend"];

const poly = (pts) => "M" + pts.map((p) => p.map((n) => +n.toFixed(2)).join(" ")).join("L") + "Z";
const ringPts = (n, r, start, inner) => { const out = []; for (let i = 0; i < n; i++) { const a = ((start + (360 / n) * i) * Math.PI) / 180; out.push([32 + Math.sin(a) * r, 32 - Math.cos(a) * r]); if (inner) { const b = ((start + (360 / n) * (i + 0.5)) * Math.PI) / 180; out.push([32 + Math.sin(b) * inner, 32 - Math.cos(b) * inner]); } } return out; };
export const PLATES = {
  journey: "M32 10a22 22 0 1 0 0 44a22 22 0 1 0 0-44Z",
  combat: "M12 11.5H52V31C52 42.5 43.5 50 32 54.5C20.5 50 12 42.5 12 31Z",
  quests: "M13.5 10.5H50.5V54L32 45.5L13.5 54Z",
  dungeons: "M12 53.5V30.5A20 20 0 0 1 52 30.5V53.5Z",
  world: poly(ringPts(4, 24, 0)),
  trade: poly(ringPts(6, 23.5, 30)),
  class: poly(ringPts(8, 23.5, 0, 18)),
  role: poly(ringPts(5, 24, 180)),
};
export const PLATE_SHIFT = { combat: -2, quests: -4, dungeons: 2.5, role: -1.5 };
export const FAMILY_OF = { level: "journey", hours: "journey", clock: "journey", crown: "journey", characters: "journey", emote: "journey", fire: "journey", kills: "combat", deaths: "combat", shield: "combat", quests: "quests", dungeons: "dungeons", zones: "world", distance: "world", jumps: "world", swim: "world", pin: "world", creature: "world", wing: "world", gold: "trade", skills: "trade", fish: "trade", gather: "trade", item: "trade", spell: "class", pet: "class", talents: "class", heal: "role" };
