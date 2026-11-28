// The invented recording every e2e lane starts from: two game accounts, two characters, three sessions, written as the
// SavedVariables files the Scarlet addon would leave in a World of Warcraft folder (Scarlet's docs/FORMAT.md).
// Nothing here is a real player's: the names, the times and every number are made up, and the same call writes the
// same bytes.
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

export const ELWYNN = 1429;
export const WESTFALL = 1436;
export const GUILD = "Ember Guard";
const TEN_YEARS = 315360000;

const session = (start, lines) => ({ start, lines });

const hit = (pos, h) => [
  "DR", h.spell ?? "", h.amount, h.source ?? "", h.env ?? 0, pos, h.over ?? "", h.hp, 1800, h.before ?? 0, h.school ?? 1,
  h.kind ?? "", h.level ?? "", h.rank ?? "", h.race ?? "", h.guild ?? "", h.hits ?? "",
  h.event, h.type ?? "", h.absorbed ?? 0, h.resisted ?? 0, h.blocked ?? 0, h.flags ?? "", h.npc ?? "",
];

const walk = (from, map, points) => points.map(([x, y, facing], i) => [from + i * 5, "P", map, x, y, facing]);

export const CHARACTERS = [
  {
    name: "Testa-Fixture",
    account: "E2EONE",
    sessions: [
      session(1795000000, [
        [0, "S", "1.15.8", "WARRIOR", "Human", 5, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 1200, 2800, 3500, 60],
        [0, "GR", "normal", 0],
        [0, "LM", "solo", "", "", 1, "s"],
        [0, "ST", 5, 3500, 26, 21, 24, 20, 21, 180, 220, 100, 8],
        [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
        [1, "SZ", ELWYNN, "Goldshire", 4210, 6580],
        [2, "GU", "s", GUILD, "Initiate", 0, 0],
        [3, "TP", 5400, 900],
        [10, "QN", 783, "A Threat Within"],
        [10, "Q", 1, 783],
        ...walk(12, ELWYNN, [[4210, 6580, 0], [4260, 6620, 14], [4320, 6660, 13], [4390, 6700, 13], [4450, 6760, 12], [4510, 6830, 12]]),
        [42, "W", "f", 310, 30, 2, "4,19"],
        [43, "N", 113, "Stonetusk Boar"],
        [43, "NS", 113, 5, "normal", "Beast", 1, "Boar", 5, 2, ELWYNN, 45, 68],
        [44, "F", 1],
        [46, "SN", 78, "Heroic Strike"],
        [46, "C", 78, ""],
        [52, "K", 113, 1],
        [52, "F", 0, 8],
        [52, "X", 95, 1295, 2800, "k", 113],
        [53, "LO", "c", 113, 3, "769x1,2589x2,2592x1", 0],
        [53, "IN", 769, "Chunk of Boar Meat"],
        [53, "O", 769, 1, 0, "", ""],
        [53, "I", 769, 1, "l", 113, 4],
        [53, "IN", 2589, "Linen Cloth"],
        [53, "O", 2589, 2, 0, "", ""],
        [53, "I", 2589, 2, "l", 113, 13],
        [53, "M", 37, 0, 3537],
        [53, "IQ", 769, 1],
        [53, "IQ", 2589, 1],
        [53, "RL", 1, 769],
        [53, "RW", "w", 769],
        [54, "IQ", 2592, 2],
        [54, "IQ", 4306, 3],
        [54, "LT", "x", "Fixture Stranger", 2592, 1],
        [54, "LT", "x", "Fixture Stranger", 4306, 1],
        [60, "N", 38, "Defias Thug"],
        [60, "NS", 38, 6, "normal", "Humanoid", 7, "", 0, 2, ELWYNN, 46, 69],
        [61, "F", 1],
        [70, "K", 38, 1],
        [70, "F", 0, 9],
        [70, "X", 110, 1405, 2800, "k", 38],
        [71, "LO", "c", 38, 1, "2589x1", 0],
        [71, "O", 2589, 1, 0, "", ""],
        [71, "I", 2589, 1, "l", 38, 13],
        [71, "M", 52, 0, 3589],
        ...walk(80, ELWYNN, [[4510, 6830, 4], [4450, 6760, 4], [4390, 6700, 5], [4320, 6660, 5], [4260, 6620, 6], [4210, 6580, 6]]),
        [110, "W", "f", 310, 30, 0, ""],
        [112, "QD", "c", 783, 197],
        [113, "Q", 2, 783, 1400, 250],
        [113, "X", 1400, 5, 3600, "q", 783],
        [113, "M", 250, 0, 3839],
        [113, "L", 6],
        [114, "ST", 6, 3839, 28, 22, 26, 20, 22, 190, 250, 100, 9],
        [120, "N", 54, "Corina Steele"],
        [120, "NI", "v", 54, "Corina Steele", "Stormwind", 1],
        [121, "I", 769, -1, "v", 54, 4],
        [121, "M", 4, 0, 3843],
        [129, "IN", 6948, "Hearthstone"],
        [129, "IN", 2592, "Wool Cloth"],
        [129, "QB", 1234, "783,100,200"],
        [129, "KT", 3, "1,2,3"],
        [129, "EB", "1429:5,1436:2,1411:1"],
        [129, "IS", "Blackfathom Deeps", 48, 1, TEN_YEARS, 1, 0, 0, 5, 1, 4, "s"],
        [129, "IS", "Gnomeregan", 90, 1, 60, 1, 0, 0, 5, 4, 4, "s"],
        [129, "IS", "Lord Kazzak", 12, "", TEN_YEARS, 1, 0, 2, "", "", "", "s"],
        [129, "AQ", "c", 2589, 125, 5400, 12, "", ""],
        [129, "AQ", "c", 2589, 150, 5000, 11, "", ""],
        [129, "SN", 2963, "Bolt of Linen Cloth"],
        [129, "IN", 2996, "Bolt of Linen Cloth"],
        [129, "RQ", 2963, 2996, 1, 1, "2589:2", 0, 1],
        [130, "BC", "b", "2589:3,6948:1"],
        [130, "BC", "w", "2589:20"],
        [130, "BC", "g", "2592:4"],
        [130, "BK", "a", 2, 123456],
        [130, "BK", "a", 1, 250000],
        [130, "RO", 0, 3600, 1],
        [130, "ES"],
      ]),
      session(1795090000, [
        [0, "S", "1.15.8", "WARRIOR", "Human", 6, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
        [0, "ST", 6, 3843, 28, 22, 26, 20, 22, 190, 250, 100, 9],
        [1, "Z", WESTFALL, 0, "none", 0, "Westfall", "Sentinel Hill"],
        [1, "SZ", WESTFALL, "Sentinel Hill", 5650, 4760],
        [3, "TP", 5540, 20],
        [4, "TY", 161, "Arms"],
        [4, "TY", 162, "Fury"],
        [4, "SN", 7001, "Improved Heroic Strike"],
        [4, "SN", 7002, "Deflection"],
        [4, "SN", 7003, "Overpower"],
        [4, "SN", 7004, "Slam"],
        [4, "SN", 7005, "Whirlwind"],
        [4, "SN", 7010, "Rampage"],
        [4, "SN", 7011, "Bloodthirst"],
        [4, "TV", 1, 1, 6, "a1b2c3d4"],
        [4, "TD", 1, 9001, 1020, 2130, 1, 3, 161, "9001:7001:3", "9002:0"],
        [4, "TD", 1, 9002, 1020, 2730, 0, 2, 161, "9002:7002:2", ""],
        [4, "TD", 1, 9003, 1620, 2730, 0, 1, 161, "9003:7003:1", ""],
        [4, "TD", 1, 9004, 2820, 33300, 2, 1, 161, "9004:7004:1,9005:7005:1", ""],
        [4, "TD", 1, 9010, 5020, 2130, 0, 1, 162, "9010:7010:1", "9011:0"],
        [4, "TD", 1, 9011, 5620, 3330, 0, 2, 162, "9011:7011:2", ""],
        [5, "TG", "s", 1, 1, "", "", 1, "161:3:0,162:0:3", 0, 1, 1, ""],
        [5, "TA", 1, 9001, 9001, 1, "", 3, 7001, 161, "s"],
        [5, "TA", 1, 9002, 9002, 2, "", 2, 7002, 161, "s"],
        [6, "TA", 1, 9001, 9001, 2, 1, 3, 7001, 161, "c"],
        [6, "SN", 1225499, "Gourmand"],
        [6, "SN", 1249519, "Well Fed"],
        [6, "LB", 2, 2, 0, 2, 65, 0, 0, 0, "s"],
        [6, "LE", 1189, 110313, 136867, 1225499, 1, 3, "Increases the duration of beneficial effects from eating food by 33%."],
        [6, "LE", 1189, 110313, 136867, 1225499, 2, 3, "Increases the duration of beneficial effects from eating food by 67%."],
        [6, "LE", 1189, 110313, 136867, 1225499, 3, 3, "Increases the duration of beneficial effects from eating food by 100%."],
        [7, "A", 1, 1249519, 1, 900, 1, ""],
        [8, "LN", 1189, 110313, 136867, 1, 0, 1, 3, 1225499, "c", ""],
        [9, "A", 1, 1249519, 1, 1200, 1, ""],
        ...walk(8, WESTFALL, [[5650, 4760, 8], [5600, 4820, 8], [5540, 4890, 9], [5480, 4960, 9], [5420, 5040, 9]]),
        [30, "W", "f", 260, 25, 1, "11"],
        [31, "N", 115, "Harvest Reaper"],
        [31, "NS", 115, 12, "normal", "Mechanical", 9, "", 0, 2, WESTFALL, 54, 50],
        [32, "F", 1],
        [38, "SN", 5101, "Reaper Slam"],
        [38, "SN", 5102, "Rust Bleed"],
        [39, "D"],
        [39, "DL", WESTFALL, 5420, 5040],
        [39, ...hit(1, { spell: 5101, amount: 412, source: "Harvest Reaper", over: 37, hp: 0, kind: "c", level: 12, rank: "elite", hits: 7, event: "SPELL_DAMAGE", flags: "ad", npc: 115 })],
        [39, ...hit(2, { spell: "?", amount: 88, source: "Harvest Reaper", hp: 412, before: 1, event: "SWING_DAMAGE" })],
        [39, ...hit(3, { spell: 5101, amount: 150, source: "Harvest Reaper", hp: 500, before: 3, event: "SPELL_DAMAGE", absorbed: 55 })],
        [39, ...hit(4, { amount: 61, source: "Defias Thug", hp: 650, before: 4, event: "RANGE_DAMAGE" })],
        [39, ...hit(5, { spell: 5102, amount: 20, source: "Harvest Reaper", hp: 711, before: 5, event: "SPELL_PERIODIC_DAMAGE", resisted: 7, blocked: 3 })],
        [39, ...hit(6, { amount: 74, source: "Harvest Reaper", hp: 731, before: 6, event: "SWING_DAMAGE" })],
        [39, ...hit(7, { amount: 70, source: "Harvest Reaper", hp: 805, before: 8, event: "SWING_DAMAGE" })],
        [39, "F", 0, 7],
        [45, "R"],
        [90, "U"],
        ...walk(95, WESTFALL, [[5420, 5040, 0], [5480, 4960, 1], [5540, 4890, 1], [5600, 4820, 0], [5650, 4760, 0]]),
        [118, "D"],
        [118, "DL", WESTFALL, 5650, 4760],
        [118, ...hit(1, { amount: 640, env: 1, over: 12, hp: 0, event: "ENVIRONMENTAL_DAMAGE", type: "Falling", hits: 2, flags: "a" })],
        [118, ...hit(2, { amount: 35, source: "Harvest Reaper", hp: 640, before: 2, event: "SWING_DAMAGE" })],
        [119, "R"],
        [120, "W", "f", 260, 25, 0, ""],
        [124, "U"],
        [125, "BC", "b", "2589:3,6948:1"],
        [125, "RO", 0, 3600, 0],
        [125, "ES"],
      ]),
    ],
  },
  {
    name: "Probe-Lane",
    account: "E2ETWO",
    sessions: [
      session(1795180000, [
        [0, "S", "1.15.8", "MAGE", "Gnome", 3, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 300, 1400, 120, 60],
        [0, "ST", 3, 120, 15, 18, 17, 26, 24, 40, 90, 180, 4],
        [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Northshire Valley"],
        [1, "SZ", ELWYNN, "Northshire Valley", 4840, 4180],
        [3, "TP", 1300, 400],
        ...walk(5, ELWYNN, [[4840, 4180, 0], [4880, 4130, 15], [4930, 4090, 15], [4990, 4060, 14]]),
        [25, "W", "f", 190, 20, 0, ""],
        [26, "N", 6, "Kobold Vermin"],
        [26, "NS", 6, 2, "normal", "Humanoid", 7, "", 0, 2, ELWYNN, 49, 40],
        [27, "F", 1],
        [28, "SN", 133, "Fireball"],
        [28, "C", 133, 1.5],
        [31, "K", 6, 1],
        [31, "F", 0, 4],
        [31, "X", 60, 360, 1400, "k", 6],
        [32, "M", 9, 0, 129],
        ...[34, 35].flatMap((at) => [
          [at, "D"],
          [at, "DL", ELWYNN, 4990, 4060],
        ]),
        ...[35, 36].flatMap((at) => [
          [at, "DR", 133, 15, "Kobold Vermin", 0, 1, 3, 0, 120, 0, 4],
          [at, "DR", "", 9, "Kobold Vermin", 0, 2, "", 15, 120, 3, 4],
          [at, "DR", "", 7, "Kobold Vermin", 0, 3, "", 24, 120, 5, 4],
        ]),
        [39, "IN", 6948, "Hearthstone"],
        [40, "BC", "b", "6948:1"],
        [40, "IN", 2592, "Wool Cloth"],
        [40, "BC", "w", "2589:5"],
        [40, "BC", "g", "2592:7"],
        [40, "BK", "a", 2, 50000],
        [40, "BK", "a", 1, 987654],
        [40, "GU", "s", GUILD, "Initiate", 0, 0],
        [40, "RO", 0, 1400, 0],
        [40, "ES"],
      ]),
    ],
  },
];

const GRIND_START = new Date(2026, 10, 13, 12).getTime() / 1000;
const GRIND_DAY = 86400;
const GRIND_RUN = 1500;
const ALT_RUN = 1200;
const ALT_OFFSET = 600;

const grindSession = (n, { offset = 0, run = GRIND_RUN } = {}) => {
  const kills = Array.from({ length: 10 }, (_, k) => [100 * (k + 1), [["K", 6, 1], ["X", 50 * (n + 1), 360, 1400, "k", 6]]]).flatMap(([at, rows]) => rows.map((row) => [at, ...row]));
  const death = n % 2 ? [[200, "D"], [200, "DL", ELWYNN, 4990, 4060], [210, "R"]] : [];
  return session(GRIND_START + offset + n * GRIND_DAY, [
    [0, "S", "1.15.8", "WARRIOR", "Human", 20, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    ...kills,
    ...death,
    [run, "ES"],
  ]);
};

/** A character that played six runs of 25 minutes on six days, each earning more experience an hour than the last. */
export const GRIND = { name: "Grind-Lane", account: "E2EGRIND", sessions: Array.from({ length: 6 }, (_, n) => grindSession(n)) };

/** A second character that played 20 minutes on each of the same six days, starting 10 minutes into the run of `GRIND`. */
export const ALT = { name: "Alt-Lane", account: "E2EALT", sessions: Array.from({ length: 6 }, (_, n) => grindSession(n, { offset: ALT_OFFSET, run: ALT_RUN })) };

const noonDaysAgo = (now, days) => {
  const d = new Date(now);
  return Math.floor(new Date(d.getFullYear(), d.getMonth(), d.getDate() - days, 12).getTime() / 1000);
};

const FULL_TALENTS = [
  [9001, 7001, "Improved Heroic Strike", 3, 161, 1020, 2130, "9002:0"],
  [9002, 7002, "Deflection", 2, 161, 1020, 2730, ""],
  [9003, 7003, "Overpower", 1, 161, 1620, 2730, ""],
  [9010, 7010, "Rampage", 1, 162, 5020, 2130, "9011:0"],
  [9011, 7011, "Bloodthirst", 2, 162, 5620, 3330, ""],
];

const FULL_LEGACY = [
  [1187, 110289, 1225457, "Master Chef", 5],
  [1187, 110290, 1225451, "Working Overtime", 5],
  [1187, 110286, 1225459, "Bartering", 2],
  [1188, 110305, 1225477, "High Alert", 2],
  [1188, 110301, 1225478, "Well Rested", 5],
  [1188, 110298, 1225474, "Talented", 1],
  [1189, 110313, 1225499, "Gourmand", 3],
  [1189, 110316, 1225502, "Permanence", 2],
  [1189, 110319, 1225501, "For Great Honor", 5],
];

const fullSession = (start) =>
  session(start, [
    [0, "S", "1.15.8", "WARRIOR", "Human", 20, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    [4, "TY", 161, "Arms"],
    [4, "TY", 162, "Fury"],
    ...FULL_TALENTS.map(([, spell, name]) => [4, "SN", spell, name]),
    [4, "TV", 1, 1, 6, "a1b2c3d4"],
    ...FULL_TALENTS.map(([node, spell, , max, tab, x, y, edges]) => [4, "TD", 1, node, x, y, 1, max, tab, `${node}:${spell}:${max}`, edges]),
    [5, "TG", "s", 1, 1, "", "Arms", 1, "161:6:0,162:3:0", 0, 0, 1, ""],
    ...FULL_TALENTS.map(([node, spell, , max, tab]) => [5, "TA", 1, node, node, max, "", max, spell, tab, "s"]),
    ...FULL_LEGACY.map(([, , spell, name]) => [6, "SN", spell, name]),
    [6.1, "LB", 30, 0, 30, 30, 30, 12, 8, 10, "s"],
    ...[1187, 1188, 1189].flatMap((tree, i) =>
      FULL_LEGACY.filter(([t]) => t === tree).map(([, node, spell, , max]) => [6.4 + i * 0.7, "LN", tree, node, node + 1000, max, "", max, max, spell, "s", ""]),
    ),
    [60, "ES"],
  ]);

/** A character first read with every talent and Legacy rank already taken, in two sessions that read the same build; its times are counted back from `now`. */
export const fullBuild = (now = Date.now()) => ({
  name: "Full-Build",
  account: "E2EFULL",
  sessions: [fullSession(noonDaysAgo(now, 4)), fullSession(noonDaysAgo(now, 2))],
});

const TRADES = [
  [171, "Alchemy", 11],
  [182, "Herbalism", 11],
  [186, "Mining", 11],
  [43, "Swords", 6],
];

const tradesSession = (start, held) =>
  session(start, [
    [0, "S", "1.15.8", "WARRIOR", "Human", 20, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    ...held.flatMap(([id, rank, max]) => {
      const [, name, category] = TRADES.find(([n]) => n === id);
      return [[5, "KN", id, name, category], [6, "SL", id, rank, max, 0, "s"]];
    }),
    [60, "ES"],
  ]);

/**
 * A character that listed Alchemy, Herbalism, Mining and Swords, then Herbalism, Mining and Swords without Alchemy, then
 * only Swords: the header of the professions was collapsed, so the third read says nothing about them. Its times are
 * counted back from `now`.
 */
export const trades = (now = Date.now()) => ({
  name: "Trades-Fixture",
  account: "E2ETRADES",
  sessions: [
    tradesSession(noonDaysAgo(now, 5), [[171, 225, 225], [182, 100, 300], [186, 150, 225], [43, 90, 200]]),
    tradesSession(noonDaysAgo(now, 3), [[182, 120, 300], [186, 160, 225], [43, 92, 200]]),
    tradesSession(noonDaysAgo(now, 1), [[43, 95, 200]]),
  ],
});

const HEADED = [
  [43, "Swords", 6],
  [171, "Alchemy", 11],
  [2937, "Alchemy", 11],
  [185, "Cooking", 9],
  [2939, "Cooking", 9],
];

const headedSession = (start, headers) =>
  session(start, [
    [0, "S", "1.15.8", "WARRIOR", "Human", 20, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    ...headers.flatMap(([title, expanded, held]) => [
      ...held.flatMap(([id, rank, max]) => {
        const [, name, category] = HEADED.find(([n]) => n === id);
        return [[5, "KN", id, name, category], [6, "SL", id, rank, max, 0, "s"]];
      }),
      [6, "SV", title, expanded ? 0 : 1, [...new Set(held.map(([id]) => HEADED.find(([n]) => n === id)[2]))].sort((a, b) => a - b).join(",")],
    ]),
    [60, "ES"],
  ]);

const ALCHEMY = [[171, 225, 225], [2937, 225, 225]];
const COOKING = [[185, 120, 225], [2939, 120, 225]];
const SWORDS = [[43, 90, 200]];

/**
 * A character that listed Swords, Alchemy and Cooking under the headers of the skills list with every header expanded,
 * then listed only Swords with the Professions header expanded and empty and the Secondary Skills header collapsed.
 * Its times are counted back from `now`.
 */
export const dropped = (now = Date.now()) => ({
  name: "Dropped-Fixture",
  account: "E2EDROPPED",
  sessions: [
    headedSession(noonDaysAgo(now, 4), [["Weapon Skills", true, SWORDS], ["Secondary Skills", true, COOKING], ["Professions", true, ALCHEMY]]),
    headedSession(noonDaysAgo(now, 2), [["Weapon Skills", true, SWORDS], ["Secondary Skills", false, []], ["Professions", true, []]]),
  ],
});

/** The same first session as `dropped`, then a read with the Professions header collapsed. */
export const collapsed = (now = Date.now()) => ({
  name: "Collapsed-Fixture",
  account: "E2ECOLLAPSED",
  sessions: [
    headedSession(noonDaysAgo(now, 4), [["Weapon Skills", true, SWORDS], ["Secondary Skills", true, COOKING], ["Professions", true, ALCHEMY]]),
    headedSession(noonDaysAgo(now, 2), [["Weapon Skills", true, SWORDS], ["Secondary Skills", true, COOKING], ["Professions", false, []]]),
  ],
});

const GERMAN = [
  [43, "Schwerter", 6],
  [171, "Alchimie", 11],
  [2937, "Alchimie", 11],
  [182, "Kräuterkunde", 11],
  [2944, "Kräuterkunde", 11],
];

const germanSession = (start, held) =>
  session(start, [
    [0, "S", "1.15.8", "WARRIOR", "Human", 20, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    ...held.flatMap(([id, rank, max]) => {
      const [, name, category] = GERMAN.find(([n]) => n === id);
      return [[5, "KN", id, name, category], [6, "SL", id, rank, max, 0, "s"]];
    }),
    [60, "ES"],
  ]);

/**
 * A character on a German client, with no header lines (an older addon): Schwerter, Alchimie and Kräuterkunde, then
 * Schwerter and Kräuterkunde. The names its skills carry differ from the ones other characters wrote for the same ids.
 */
export const german = (now = Date.now()) => ({
  name: "Deutsch-Fixture",
  account: "E2EGERMAN",
  sessions: [
    germanSession(noonDaysAgo(now, 4), [[43, 90, 200], ...[171, 2937].map((id) => [id, 225, 225]), ...[182, 2944].map((id) => [id, 100, 300])]),
    germanSession(noonDaysAgo(now, 2), [[43, 92, 200], ...[182, 2944].map((id) => [id, 110, 300])]),
  ],
});

const ADDON_LINES = [[20, "gm", "N", ELWYNN, 4390, 6700, 2447]];

const text = (lines) => {
  let last = 0;
  return lines.map(([at, ...fields]) => {
    const dt = Math.round((at - last) * 100);
    last = at;
    return [dt, ...fields].join("\t");
  });
};

const lua = (value, depth = 0) => {
  if (typeof value === "number") return String(value);
  if (typeof value === "string") return '"' + value.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n") + '"';
  const pad = "\t".repeat(depth + 1);
  const body = Array.isArray(value)
    ? value.map((v) => pad + lua(v, depth + 1) + ",")
    : Object.entries(value).map(([k, v]) => `${pad}["${k}"] = ${lua(v, depth + 1)},`);
  return "{\n" + body.join("\n") + "\n" + "\t".repeat(depth) + "}";
};

const chunk = (q, name, start, lines, encoding) => {
  const joined = lines.join("\n");
  const z = encoding === "d64" ? zlib.deflateRawSync(Buffer.from(joined)).toString("base64") : joined;
  return { q, c: name, s: start, t: start, n: lines.length, e: encoding, z };
};

/** A session recorded after the first export: what a later export has to pick up. */
export const LATER = { character: "Testa-Fixture", start: 1795270000, lines: [
  [0, "S", "1.15.8", "WARRIOR", "Human", 6, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
  [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
  [20, "ES"],
] };

const savedVariables = (character, extra) => {
  const chunks = character.sessions.map((s, i) => chunk(i + 1, character.name, s.start, text(s.lines), i % 2 ? "txt" : "d64"));
  if (extra) chunks.push(chunk(chunks.length + 1, character.name, extra.start, text(extra.lines), "d64"));
  const tables = [`ScarletDB = ${lua({ v: 1, seq: chunks.length, chunks, buf: [], chars: { [character.name]: { v: 1 } } })}`];
  if (character.name === CHARACTERS[0].name) {
    const start = character.sessions[0].start;
    const addons = { v: 1, seq: 1, chunks: [chunk(1, character.name, start, text(ADDON_LINES), "txt")], buf: [] };
    tables.push(`ScarletAddonsDB = ${lua(addons)}`);
  }
  return ["", ...tables, ""].join("\n");
};

/** The SavedVariables file of a character's account under a World of Warcraft folder. */
export const savedVariablesPath = (wow, character) => path.join(wow, "_forever_", "WTF", "Account", character.account, "SavedVariables", "Scarlet.lua");

/** The Fishing Log's saved variables for one character and its account (Scarlet's FORMAT.md, "The Fishing Log"). */
export const FISHING = {
  character: "Testa-Fixture",
  rival: "Rival-Angler",
  account: `TheFishingLogDB = {
["lastSeen"] = 1795300000,
["fishNames"] = {
[6362] = "Raw Rockscale Cod",
},
["spotNames"] = {
["a977"] = "Wavestrider Beach",
},
["spotWhere"] = {
["a977"] = {
["map"] = 1446,
["x"] = 0.5,
["y"] = 0.25,
["mine"] = true,
},
},
["spots"] = {
["a977"] = {
["seasonID"] = 24322,
["fish"] = {
[6362] = {
["h"] = "Rival Angler-SomeRealm",
["t"] = 1795100000,
["w"] = 15.5,
},
},
["season"] = {
[6362] = {
["h"] = "Testa-Fixture",
["t"] = 1795200000,
["w"] = 13.3,
},
},
},
},
["factions"] = {
["rivalangler"] = "A",
},
}
`,
  char: `TheFishingLogChar = {
["records"] = {
["a977"] = {
[6362] = {
["when"] = 1795200000,
["weight"] = 13.3,
["name"] = "Raw Rockscale Cod",
["specimen"] = true,
},
},
},
["history"] = {
{
["when"] = 1795200500,
["zone"] = "Tanaris",
["sub"] = "Wavestrider Beach",
["duration"] = 600,
["count"] = 4,
["total"] = 30.25,
["biggest"] = 13.3,
["biggestName"] = "Raw Rockscale Cod",
["solo"] = true,
["outcomes"] = {
["casts"] = 5,
["landed"] = 4,
},
},
},
["profile"] = {
["updated"] = 1795200600,
["xp"] = 120,
["journal"] = {
[6362] = {
["c"] = 4,
["b"] = 13.3,
["s"] = 13.3,
},
},
},
}
`,
};

/** Writes the World of Warcraft folder; `later` adds the chunk a second export has to find. */
export function writeWow(wow, { later = false } = {}) {
  for (const character of CHARACTERS) {
    const file = savedVariablesPath(wow, character);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, savedVariables(character, later && character.name === LATER.character ? LATER : null));
  }
  const angler = CHARACTERS.find((c) => c.name === FISHING.character);
  const account = path.join(wow, "_forever_", "WTF", "Account", angler.account);
  const charFile = path.join(account, "91", angler.name, "SavedVariables", "TheFishingLog.lua");
  fs.mkdirSync(path.dirname(charFile), { recursive: true });
  fs.writeFileSync(charFile, FISHING.char);
  fs.writeFileSync(path.join(account, "SavedVariables", "TheFishingLog.lua"), FISHING.account);
  fs.mkdirSync(path.join(wow, "_forever_", "Interface", "AddOns", "Scarlet"), { recursive: true });
  return wow;
}

/** Writes a World of Warcraft folder that holds `GRIND`, and `ALT` with `alt`. */
export function writeGrind(wow, { alt = false } = {}) {
  for (const character of alt ? [GRIND, ALT] : [GRIND]) {
    const file = savedVariablesPath(wow, character);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, savedVariables(character, null));
  }
  fs.mkdirSync(path.join(wow, "_forever_", "Interface", "AddOns", "Scarlet"), { recursive: true });
  return wow;
}

const resetSession = (start, level) =>
  session(start, [
    [0, "S", "1.15.8", "WARRIOR", "Human", level, "Alliance", "0.9.0", 1, "Forever", 1, 11508, 5, 3600, 3843, 60],
    [1, "Z", ELWYNN, 0, "none", 0, "Elwynn Forest", "Goldshire"],
    [20, "ES"],
  ]);

/**
 * A character whose saved store started over: three sessions at levels 2 to 4 numbered q 1 to 3, then a new store
 * that numbers three later sessions at levels 5 to 7 from q 1 again, so both runs are named 1-3 and 1-1 (addons).
 */
export const RESET = {
  name: "Reset-Fixture",
  account: "E2ERESET",
  before: [resetSession(1795400000, 2), resetSession(1795407200, 3), resetSession(1795414400, 4)],
  after: [resetSession(1795500000, 5), resetSession(1795507200, 6), resetSession(1795514400, 7)],
  addon: { before: { start: 1795400000, lines: [[20, "gm", "N", ELWYNN, 4390, 6700, 2447]] }, after: { start: 1795500000, lines: [[20, "gm", "N", ELWYNN, 4400, 6710, 2447]] } },
};

/** Writes the World of Warcraft folder of `RESET` as the first store (`stage` "before") or the one that began again ("after"); `unreadable` adds a chunk in an encoding no reader knows. */
export function writeReset(wow, stage, { unreadable = false } = {}) {
  const sessions = RESET[stage];
  const chunks = sessions.map((s, i) => chunk(i + 1, RESET.name, s.start, text(s.lines), i % 2 ? "txt" : "d64"));
  if (unreadable) chunks.push({ ...chunk(chunks.length + 1, RESET.name, sessions[0].start + 86400, ["0\tES"], "txt"), e: "zz" });
  const addons = { v: 1, seq: 1, chunks: [chunk(1, RESET.name, RESET.addon[stage].start, text(RESET.addon[stage].lines), "txt")], buf: [] };
  const tables = [
    `ScarletDB = ${lua({ v: 1, seq: chunks.length, chunks, buf: [], chars: { [RESET.name]: { v: 1 } } })}`,
    `ScarletAddonsDB = ${lua(addons)}`,
  ];
  const file = savedVariablesPath(wow, RESET);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, ["", ...tables, ""].join("\n"));
  fs.mkdirSync(path.join(wow, "_forever_", "Interface", "AddOns", "Scarlet"), { recursive: true });
  return wow;
}

/** Writes a World of Warcraft folder that holds `fullBuild(now)` alone. */
export function writeFullBuild(wow, now = Date.now()) {
  const character = fullBuild(now);
  const file = savedVariablesPath(wow, character);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, savedVariables(character, null));
  fs.mkdirSync(path.join(wow, "_forever_", "Interface", "AddOns", "Scarlet"), { recursive: true });
  return wow;
}

/** Writes a World of Warcraft folder that holds `trades(now)`, `dropped(now)`, `collapsed(now)` and `german(now)`. */
export function writeTrades(wow, now = Date.now()) {
  for (const character of [trades(now), dropped(now), collapsed(now), german(now)]) {
    const file = savedVariablesPath(wow, character);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, savedVariables(character, null));
  }
  fs.mkdirSync(path.join(wow, "_forever_", "Interface", "AddOns", "Scarlet"), { recursive: true });
  return wow;
}

/** How many lines the recording holds for one character, or for all of them, as the export counts them. */
export const lineCount = ({ later = false, character = null } = {}) =>
  CHARACTERS.filter((c) => !character || c.name === character).reduce(
    (n, c) => n + c.sessions.reduce((m, s) => m + s.lines.length, 0) + (later && c.name === LATER.character ? LATER.lines.length : 0),
    0,
  );

if (process.argv[1] && path.resolve(process.argv[1]) === import.meta.filename) {
  const wow = process.argv[2];
  if (!wow) {
    console.error("usage: node e2e/scenario.mjs <folder to write the World of Warcraft folder into> [--later]");
    process.exit(2);
  }
  writeWow(wow, { later: process.argv.includes("--later") });
  console.log(`${wow}: ${CHARACTERS.length} characters, ${lineCount({ later: process.argv.includes("--later") })} lines`);
}
