// #system: every component of DESIGN.md drawn from invented data. Reachable by its hash only.

import { el, elText, num, money, moneyHtml, tile, tiles, progressRing, spark, stack, bars, compare, goal, pace, record, strip, routeCard, sessionCard, collectionRow, linkRow, wireTips } from "../lib.js";
import { crest, card, tierChip } from "../featcard.js";
import { TIER_NAMES } from "../glyphs.js";

const SWATCHES = [
  ["bg", "#141416", "page"], ["win", "#101012", "app chrome"], ["panel", "#1b1b1f", ""], ["card", "#232328", "tooltips, menus"],
  ["fg", "#f0f1f3", "headings, numbers"], ["text", "#cdced4", "body"], ["muted", "#8a8a93", "labels, 5.4:1"], ["dim", "#6e6e7a", "lone grey marks"],
  ["accent", "#ff2d55", "you, recorded data"], ["soft", "#ff6b8e", "emphasis in text"], ["deep", "#c0102e", "combat marks"], ["ink", "#141416", "text on scarlet"],
  ["copper", "#c58a63", "tier 1"], ["silver", "#c3c7cf", "tier 2"], ["gold", "#d9b25f", "tier 3"], ["line", "rgba(255,255,255,0.13)", "white 7% and 13%"],
];

const SERIES_NAMES = [
  ["s1", "#ff2d55", "Kills · On foot"], ["s2", "#2f9ec4", "Exploring · Swimming"], ["s3", "#98721c", "Quests · Mounted"],
  ["s4", "#9a7df2", "Dungeons · Flight paths"], ["rest", "#54545d", "Other · Standing still"],
];

const swatches = (rows, cols) => {
  const g = el("div", "sw");
  g.style.gridTemplateColumns = `repeat(${cols}, minmax(0, 1fr))`;
  for (const [name, value, note] of rows) {
    const cell = el("div");
    const i = el("i");
    i.style.setProperty("--c", value);
    cell.append(i, elText("b", "", `${name} ${value.startsWith("#") ? value : ""}`.trim()), elText("span", "", note));
    g.append(cell);
  }
  return g;
};

const section = (title, ...kids) => {
  const s = el("section");
  s.append(elText("h2", "sc-h", title), ...kids);
  return s;
};

const caption = (text) => elText("p", "cap", text);

const demoRecap = () => {
  const start = Math.floor(new Date(2026, 10, 22, 19, 42).getTime() / 1000);
  const xp = [0, 200, 100, 1800, 3100, 3600, 4000, 2200, 4400, 5200, 3800, 4100, 900, 2600, 4700, 4500, 6100, 3900, 3300, 4800, 4300, 4800, 4400, 1200, 3000, 4100, 800, 300, 200];
  const combat = xp.map((v) => Math.min(300, Math.round(v / 20)));
  return {
    v: 1, character: "Sable-Varrow", class: "MAGE", start, end: start + 144 * 60, played: 144 * 60, level: [41, 42], zone: "Tanaris",
    title: { rule: "level", text: "Level 41 to 42 in Tanaris" },
    xp: { kills: 41200, dungeons: 0, quests: 38900, explore: 2100, other: 4200 }, runs: 0,
    counters: { kills: 312, quests: 14 },
    buckets: { secs: 300, xp, combat },
    zones: [{ name: "Gadgetzan", at: 0, secs: 720 }, { name: "Tanaris", at: 720, secs: 6720 }, { name: "Gadgetzan", at: 7440, secs: 1200 }],
    levels: [{ level: 42, at: 130 * 60 }],
    moments: [
      { at: 49 * 60, kind: "death", text: "Died to a Wastewander Assassin" },
      { at: 126 * 60, kind: "death", text: "Died to a Dunemaul Brute" },
      { at: 119 * 60, kind: "feat", text: "Rampage, Rare: 300 kills in one session", feat: "rampage", tier: 2 },
    ],
    route: null,
    feats: { earned: [{ key: "rampage", from: 1, tier: 2, at: 119 * 60, value: 312, name: "Rampage", family: "combat" }], near: [] },
  };
};

const demoRoute = () => {
  const points = [];
  let x = 220;
  let y = 330;
  let a = 0.4;
  for (let i = 0; i < 160; i++) {
    a += Math.sin(i * 12.9898) * 0.5;
    x = Math.max(60, Math.min(940, x + Math.cos(a) * 14 + (i / 160) * 3));
    y = Math.max(60, Math.min(940, y + Math.sin(a) * 14));
    points.push(Math.round(x), Math.round(y));
  }
  return { sheet: { aspect: 1, points, breaks: [], air: [] } };
};

const crestCell = (scope, name, tier, progress, label) => {
  const c = el("div", "cell");
  c.append(crest(name, tier, { size: 104, progress, scope }));
  const t = el("p", "sc-h3");
  t.style.marginTop = "12px";
  if (tier) t.append(tierChip(tier, TIER_NAMES[tier]));
  else t.append("Not earned");
  c.append(t, elText("p", "sc-label", label));
  return c;
};

const stateCell = (...kids) => {
  const c = el("div");
  c.append(...kids);
  return c;
};

const para = (text, cls = "") => elText("p", cls, text);

export async function render(body) {
  wireTips();
  body.innerHTML = "";
  const root = el("div", "sheet-system");
  const lead = el("p", "recap-sub");
  lead.textContent = "Every component of the design drawn from invented data.";
  root.append(lead);

  root.append(section("Surfaces, text and accent", swatches(SWATCHES.slice(0, 8), 8), swatches(SWATCHES.slice(8), 8)));

  const xpStack = stack([["Kills", 41200, "--s-kill"], ["Exploring", 2100, "--s-explore"], ["Quests", 38900, "--s-quest"], ["Dungeons", 12400, "--s-dungeon"], ["Other", 4200, "--s-other"]]);
  const travel = stack([["Swimming", 9, "--m-swim", "9m"], ["On foot", 65, "--m-foot", "1h 05m"], ["Mounted", 18, "--m-mount", "18m"], ["Flight paths", 6, "--m-flight", "6m"], ["Standing still", 64, "--m-still", "1h 04m"]]);
  const two = el("div", "recap-grid two");
  two.append(stateCell(para("Experience by source", "sc-label"), xpStack), stateCell(para("Time by travel mode", "sc-label"), travel));
  root.append(
    section("Chart series, in the order they are drawn", swatches(SERIES_NAMES, 5), two, caption("Four hues and one grey. Each stack keeps its order, a 2 px gap and a key with values.")),
  );

  const grid = (cells) => {
    const g = el("div", "sc-grid");
    g.style.gridTemplateColumns = "repeat(6, minmax(0, 1fr))";
    g.append(...cells);
    return g;
  };
  const NEXT_PIECE = ["The shield fills in green", "The crown fills in blue", "The banner fills in purple", "The side piece fills in orange", "The rays fill in light", "Nothing left to fill"];
  const PROGRESS = [0.3, 0.9, 0.5, 0.7, 0.4, 0];
  const tiers = grid(NEXT_PIECE.map((label, tier) => crestCell("character", "kills", tier, PROGRESS[tier], label)));
  const scopes = el("div", "sys-row");
  for (const [scope, glyph, label] of [["character", "kills", "Character"], ["class", "m-bolt", "Class"], ["account", "hours", "Account"]]) {
    const c = el("div");
    c.append(crest(glyph, 2, { size: 64, progress: 0.5, scope }), elText("p", "sc-label", label));
    scopes.append(c);
  }
  for (const [size, label] of [[28, "28 · row"], [18, "18 · inline"]]) {
    const c = el("div");
    c.append(crest("level", 3, { size }), elText("p", "sc-label", label));
    scopes.append(c);
  }
  root.append(section("Feat crest: the shield is the scope, the pieces are the tier", tiers, scopes));

  const cards = el("ul", "feats");
  cards.append(
    card({ name: "Soloist", tier: 0, glyph: "dungeons", crest: "character", progress: 0.3, counted: "No dungeon run alone yet", to_next: "1 to Uncommon", share: 6 }),
    card({ name: "Slayer", tier: 2, tier_name: "Rare", glyph: "kills", crest: "character", progress: 0.99, counted: "9,912 kills", to_next: "88 to Epic", share: 64 }),
    card({ name: "Rampage", tier: 2, tier_name: "Rare", glyph: "kills", crest: "character", progress: 0.04, counted: "312 kills in one session", to_next: "288 to Epic" }),
    card({ name: "Flawless", tier: 5, tier_name: "Scarlet", glyph: "shield", crest: "character", progress: 1, counted: "100 dungeon runs with no deaths in the party", to_next: "Scarlet since Nov 4", share: 0.4 }),
    card({ name: "Veteran", tier: 3, tier_name: "Epic", glyph: "hours", crest: "account", progress: 0.4, counted: "1,840 hours on the account", to_next: "120 to Legendary" }),
  );
  root.append(section("Feat card", cards, caption("The card has no bar: the next piece filling in on the crest is the progress.")));

  const week = tiles([
    tile("Kills", "9,912", "63 an hour"),
    tile("Played since Monday", "16h 20m", "", { extra: spark([0, 40, 140, 150, 210, 180, 260], { w: 120, form: "cols" }) }),
    tile("Hours recorded", "1,284,000", "", { extra: spark([40, 44, 47, 52, 55, 61, 66, 70, 78], { w: 120 }) }),
    tile("Distance", "9.4<small>mi</small>", "", { extra: stack([["Swimming", 0.9, "--m-swim"], ["On foot", 6, "--m-foot"], ["Flight paths", 2.5, "--m-flight"]], { key: false, h: 8 }) }),
  ]);
  const ring = el("div", "sc-panel");
  ring.style.display = "flex";
  ring.style.alignItems = "center";
  ring.style.gap = "20px";
  const inner = el("span", "", `<b class="sc-num" style="font-size:40px">72</b><br><span class="sc-label">of 92</span>`);
  ring.append(progressRing(0.78, { size: 112, inner }), stateCell(para("Progress ring", "sc-h3"), para("One arc, filled clockwise from the top.", "sc-label")));
  const numbers = el("div");
  numbers.style.cssText = "display:grid;grid-template-columns:minmax(0,1fr) 330px;gap:24px;margin-top:12px";
  numbers.append(week, ring);
  root.append(section("Numbers: tiles, hero, tick ring", numbers, caption("Tiles share one hairline grid and are never separate cards. A tile whose value is zero is left out.")));

  const recap = demoRecap();
  const full = el("div", "sc-panel");
  full.append(strip(recap));
  const compact = el("div", "sc-panel");
  compact.style.width = "560px";
  compact.append(strip(recap, { compact: true }));
  root.append(section("Session strip, full and compact", full, compact, caption("Rows, top to bottom: zones in order, XP per five minutes, combat, moments, clock. Hover gives the readout.")));

  const cmp = el("div", "compare-grid");
  cmp.append(
    para("Kills an hour", "sc-sub"), compare({ p10: 38, p25: 61, p50: 84, p75: 108, p90: 124, you: 104 }, { hi: 160, name: "Kills an hour" }), elText("b", "", "104"),
    para("Deaths an hour", "sc-sub"), compare({ p10: 0.1, p25: 0.22, p50: 0.36, p75: 0.6, p90: 0.9, you: 0.21 }, { hi: 1.4, name: "Deaths an hour" }), elText("b", "", "0.21"),
    para("Jumps an hour", "sc-sub"), compare({ p10: 90, p25: 210, p50: 420, p75: 760, p90: 1300, you: 2770 }, { hi: 3000, name: "Jumps an hour", text: "2,770" }), elText("b", "", "2,770"),
  );
  const barsDemo = bars([["Wastewander Bandit", 58], ["Dunemaul Ogre", 44], ["Scorpid Hunter", 41], ["Wastewander Assassin", 37], ["Steeljaw Snapper", 29]]);
  const three = el("div", "recap-grid two");
  three.append(stateCell(cmp, caption("Thin band: 10th to 90th percentile. Thick band: the middle half. Tick: the median. Diamond: you.")), stateCell(barsDemo, caption("Label left, value right, a 4 px bar under both.")));
  root.append(section("Compare strip, bar list", three));

  const goals = el("div", "sc-panel");
  goals.style.cssText = "display:grid;gap:16px";
  goals.append(
    goal(42, 60, { label: "Level", size: "lg", eta: "Level 60 in 3d 4h" }),
    goal(131, 150, { label: "Mining", eta: "about 2h" }),
    goal(300, 300, { label: "Fishing" }),
    goal(17400, 42000, { label: "Orgrimmar", note: "Honored" }),
  );
  const paces = tiles(
    [
      pace(12404, { p10: 6200, p25: 8100, p50: 10400, p75: 14000, p90: 16800 }, { label: "Experience", best: true, newBest: true, peak: { value: 12404, from: 1795400000000 } }),
      pace(-3100, { p10: -9000, p25: -4200, p50: 1200, p75: 5400, p90: 11000 }, { label: "Gold", format: moneyHtml, text: money, peak: { value: 11000, from: 1795300000000 } }),
      pace(42, null, { label: "Kills", peers: 2 }),
      pace(9, null, { label: "Gathers", qualifies: false }),
    ],
    4,
  );
  const records = tiles(
    [
      record("Best XP an hour", num(69658), "Nov 21 · level 41", "#sessions", { count: [69658, num] }),
      record("Most kills", num(744), "Nov 21 · level 42", "#sessions", { count: [744, num] }),
      record("Highest balance", moneyHtml(724903), "Nov 20", ""),
    ],
    3,
  );
  root.append(
    section("Goal, pace and record", goals, paces, records, caption("A goal fills toward its ceiling and says how long it takes. A pace is the rate over the strip of the character's own history. A record links to the session that set it.")),
  );

  const card1 = sessionCard(recap, { onOpen: () => {} });
  const routePanel = el("div", "sc-panel recap-route");
  routePanel.style.height = "356px";
  routePanel.append(routeCard(demoRoute(), { levels: [{ frac: 0.9 }] }));
  const half = el("div", "recap-grid two");
  half.append(card1, routePanel);
  root.append(section("Session card and route card", half));

  const wall = collectionRow(
    "Journey",
    [
      { key: "summit", name: "Summit", tier: 3, glyph: "level", family: "journey", progress: 0.7, counted: "Level 42" },
      { key: "veteran", name: "Veteran", tier: 2, glyph: "hours", family: "journey", progress: 0.4 },
      { key: "steadfast", name: "Steadfast", tier: 0, glyph: "clock", family: "journey", progress: 0.2 },
    ],
    { draw: { crest: (f) => crest(f.glyph, f.tier, { size: 48, progress: f.progress }) } },
  );
  root.append(section("Collection row", wall));

  const controls = el("div");
  controls.style.cssText = "display:flex;flex-wrap:wrap;align-items:center;gap:20px 32px;margin-top:16px";
  const btns = el("span");
  btns.style.cssText = "display:flex;gap:8px";
  btns.append(elText("a", "sc-btn", "Get the app"), elText("a", "sc-btn quiet", "All sessions"), elText("a", "sc-btn quiet small", "Link"));
  const seg = el("span", "sc-seg");
  seg.append(elText("button", "", "Everyone"), elText("button", "on", "You vs everyone"), elText("button", "", "Only you"));
  const chips = el("span");
  chips.style.cssText = "display:flex;gap:8px";
  chips.append(elText("button", "sc-chipbtn on", "All tiers"), elText("button", "sc-chipbtn", "Rare"), elText("button", "sc-chipbtn", "Epic"));
  const tog = (on, text) => {
    const t = el("span");
    t.style.cssText = "display:flex;align-items:center;gap:8px;font-size:13.5px";
    t.append(el("span", `sc-toggle${on ? " on" : ""}`), text);
    return t;
  };
  controls.append(btns, seg, chips, tog(true, "Play a sound with a feat toast"), tog(false, "Animate feats"));
  root.append(section("Controls", controls, elText("p", "sc-empty", "No sessions recorded for Odile Fenn yet. They appear after the next export.")));

  const states = el("div", "states");
  const cell = (...kids) => {
    const c = el("div");
    c.append(...kids);
    return c;
  };
  const dot = (off, text) => {
    const p = el("p", "sc-note");
    p.append(el("span", `sc-dot${off ? " off" : ""}`), ` ${text}`);
    return p;
  };
  for (const text of ["State", "Addon", "App", "Website"]) states.append(elText("div", "hd", text));
  const row = (name, addon, app, web) => {
    states.append(elText("div", "st", name), cell(...addon), cell(...app), cell(...web));
  };
  const none = (text) => [elText("p", "none", text)];
  const addr = (text) => elText("span", "addr", text);
  row("No app", [para("Website: no app", "sc-note"), para("The Scarlet app adds your history, all 241 feats and your pages on the website."), addr("scarletweb.io/download")], none("Does not apply."), [linkRow("The desktop app uploads what the addon records.", "Get the app", () => {})]);
  row("App, uploading off", [para("Website: not linked", "sc-note"), para("Uploading is off in the Scarlet app.")], [dot(true, "Uploading is off"), linkRow("40 more feats are counted on the website. Uploading is off.", "Website settings", () => {})], none("Nothing. The site has no data for this app."));
  row("Uploading, not linked", [para("Website: not linked", "sc-note"), para("Your page:"), addr("scarletweb.io/c/Sable-Varrow"), para("Link in the Scarlet app: Settings, Website.", "sc-note")], [dot(true, "Not linked"), linkRow("Link this app to your account to pin feats and choose what is public.", "Link", () => {}, { hide: () => {} })], [para("On the character's page, one line in the head:"), para("Is this yours? Link the app to manage it", "sc-note")]);
  row("Linked", [para("Website: linked", "sc-note"), para("Your page:"), addr("scarletweb.io/c/Sable-Varrow"), para("Last upload 5 minutes ago.", "sc-note")], [dot(false, "Linked to scarletweb.io"), para("No rows anywhere.", "none")], [linkRow("Scarlet app linked on 1 computer · last upload 5 minutes ago", "Settings", () => {})]);
  root.append(section("Link entry points: four states on three surfaces", states, caption("One line, in one fixed place per surface. It never opens by itself.")));

  const toasts = el("div");
  toasts.style.marginTop = "12px";
  const play = elText("button", "btn", "Play an earned toast");
  play.type = "button";
  play.id = "system-toast";
  play.onclick = async () => {
    const { playToasts } = await import("../feat-toast.js");
    playToasts(
      "Sable-Varrow",
      [{ key: "rampage", name: "Rampage", glyph: "kills", icon: "kills", family: "combat", from: 1, from_name: "Uncommon", tier: 2, tier_name: "Rare", counted: "300 kills in one session", progress: 0.04 }],
      () => {},
      { local: true },
    );
  };
  toasts.append(play);
  root.append(section("Feat toast", toasts));

  body.append(root);
}
