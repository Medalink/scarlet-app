// The checks to run before a ship: the real `scarlet` command and, on Windows, the real app on a desktop of its own.
//   node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]
// Exit 0 when every check passes, 1 when one fails, 2 on a usage error, 3 when a window of the app was found on the
// desktop in use.
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { parseArgs } from "node:util";
import { launch } from "./cdp.mjs";
import zlib from "node:zlib";
import { ALT, CHARACTERS, ELWYNN, FISHING, GRIND, GUILD, LATER, RESET, WESTFALL, fullBuild, lineCount, savedVariablesPath, trades, writeFullBuild, writeGrind, writeReset, writeTrades, writeWow } from "./scenario.mjs";

const REPO = path.resolve(import.meta.dirname, "..");
const WINDOWS = process.platform === "win32";
const USAGE = "usage: node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]";
const TESTA = CHARACTERS[0].name;
const PROBE = CHARACTERS[1].name;
const SHOWN = { testa: TESTA.replace("-", " "), probe: PROBE.replace("-", " "), grind: GRIND.name.replace("-", " "), alt: ALT.name.replace("-", " ") };
const OTHER_GRIND = new RegExp(SHOWN.alt);
const OTHER_CHARACTER = new RegExp(`${SHOWN.probe}|Kobold Vermin`);
const OTHER_CHARACTER_ONLY = new Set(["character", "sessions", "log", "map", "loot", "data"]);

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const shortDay = (ms) => {
  const d = new Date(ms);
  return `${MONTHS[d.getMonth()]} ${d.getDate()}${d.getFullYear() === new Date().getFullYear() ? "" : ` ${d.getFullYear()}`}`;
};
const exactStamp = (ms) => {
  const d = new Date(ms);
  const two = (n) => String(n).padStart(2, "0");
  return `${WEEKDAYS[d.getDay()]} ${MONTHS[d.getMonth()]} ${d.getDate()} ${d.getFullYear()} ${d.getHours() % 12 || 12}:${two(d.getMinutes())}:${two(d.getSeconds())} ${d.getHours() < 12 ? "AM" : "PM"}`;
};
const startOfDay = (ms) => {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
};
const daysAgo = (ms) => Math.round((startOfDay(Date.now()) - startOfDay(ms)) / 86400000);
const MOMENT = String.raw`(?:just now|\d+ (?:seconds?|minutes?|hours?|days?) ago|yesterday|in \d+ (?:seconds?|minutes?|hours?|days?)|[A-Z][a-z]{2} \d{1,2}(?: \d{4})?)`;
const OLD_DATE_SHAPE = /\d{4}-\d\d-\d\d|\b\d{1,2}:\d\d(?::\d\d)?(?![:\d])(?! ?[AP]M)/;
const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const BODY = `document.querySelector("#body").innerText.replace(/\\s+/g, " ")`;

const ACCOUNT_ROW = /(?=.*Account \d+ of \d+)(?!.*Account holds)/.source;
const FEATS_LOCAL = new RegExp(String.raw`^${ACCOUNT_ROW}(?=.*Testa Fixture [1-9]\d* of \d+)(?=.*Closest to the next tier)(?=.*Every feat Testa can hold)(?=.*\d+ more feats are counted on the website Website settings)`);
const FEATS_LOCAL_ALL = new RegExp(String.raw`^${ACCOUNT_ROW}(?=.*Probe Lane [1-9]\d* of \d+)(?=.*Testa Fixture [1-9]\d* of \d+)(?=.*\d+ more feats are counted on the website)`);

const VIEWS = [
  {
    key: "character",
    shows: /^(?=.*Testa Fixture Level 6 Human Warrior Alliance in Westfall)(?=.*Whole life Quests completed 1,234 .* Titles known 3 .* Zone maps explored 3 .* Instance locks Blackfathom Deeps normal 1\/4 .* Lord Kazzak World boss)(?!.*Gnomeregan)/,
    never: /No characters recorded yet/,
  },
  { key: "characters", shows: /^(?=.*Last session)(?=.*Testa Fixture)(?=.*Probe Lane)/, never: /No characters recorded yet/ },
  { key: "feats", shows: FEATS_LOCAL, never: /No feats counted|Could not|refused|Copper|Silver|Legend\b|Poor/ },
  { key: "stats", shows: /^(?=.*Experience 1,605)(?=.*Kills 2)(?=.*Travel and time)(?=.*Experience over time)(?=.*Top spells cast Heroic Strike 1)(?=.*Moving and standing)(?=.*Time by zone)/, never: /No recorded time yet/ },
  { key: "activity", shows: /Busiest hour: (Mon|Tue|Wed|Thu|Fri|Sat|Sun) \d{1,2}:00 (AM|PM)/, never: /No play time recorded yet|events in an hour/ },
  { key: "log", shows: /kill Stonetusk Boar \(113\) by you/, never: /No events|Nothing recorded/ },
  { key: "sessions", shows: /^(?=.*2m in Westfall)(?=.*Level 5 to 6 in Elwynn Forest)(?=.*2m 10s · 2 kills · 1 quest · 1,605 XP)/, never: /No sessions yet\./ },
  { key: "map", shows: /Elwynn Forest 2m 9s Westfall 2m 4s Paths Flights and teleports Deaths Creatures met/, never: /No paths recorded yet/ },
  { key: "loot", shows: /^(?=.*Drop rates .* Stonetusk Boar 1 corpse Chunk of Boar Meat 1 of 1 Linen Cloth 1 of 1)(?=.*Items received Common 3 Uncommon 1 Rare 1 Received Rolls won Testa Fixture Solo 3 1 Group members \(1\) 1 player 2)/, never: /No creature loot with an item list|No loot recorded yet/ },
  { key: "character", hash: "character/class", shows: /Spells cast most Heroic Strike 1/, never: /Select a character|Loading the class section/ },
  { key: "character", hash: "character/talents", shows: /Points spent 3 \/ 4 .* Unspent 1 0 class · 1 spec Arms 3 Fury 0 Arms · 3 points I 2\/3 D 2\/2 .* Fury · 0 points .* Order points were taken Level 6 3 points already spent in 2 talents when Scarlet first read group 1 .* Improved Heroic Strike Picked · point 4 · Arms .* 1→2\/3/, never: /No talent lines recorded|Select a character/ },
  { key: "legacy", shows: /^(?=.*Earned 2 \/ 65)(?=.*Spent 0 To spend 2 Next rank 1 Can afford)(?!.*Next ranks)(?!.*Ranks bought)(?=.*Professions · 0 points .* Adventure · 0 points .* Progression · 0 points)(?=.*Progression G Gourmand 1 \/ 3 33% Next rank costs 1 · Can afford Rank 1)(?=.*Well Fed duration)(?=.*Rank 0 15\.0 min measured in 1 buffs)(?=.*Rank 1 20\.0 min measured in 1 buffs · \+33% against rank 0 · tooltip 33%)/, never: /No Legacy lines recorded|Select a character/ },
  {
    key: "items",
    shows: new RegExp(String.raw`^(?=.*Linen Cloth 3 looted Testa Fixture 3 Warband bank \(E2EONE\) 20 Warband bank \(E2ETWO\) 5 28 42s 1s ?50c each Wool Cloth Guild bank 7 7 –)(?=.*Recipes Makes Needs Known by 1× Bolt of Linen Cloth 2× Linen Cloth Testa Fixture)(?=.*Probe Lane 1 item Bags .* · Not read: bank, mailbox, auctions)(?=.*Warband and guild banks Warband bank \(E2EONE\) 12g ?34s 20 items · 1 kind · Last read ${MOMENT} by Testa Fixture Warband bank \(E2ETWO\) 5g 5 items · 1 kind · Last read .* by Probe Lane Guild bank 98g ?76s 7 items · 1 kind · Last read .* by Probe Lane)(?!.*1s ?25c)`),
    never: /No bags or bank recorded yet|No items match|Guild bank \(|account unknown/,
  },
  {
    key: "data",
    shows: /^(?=.*Export now .* 2 files .* Payloads .* [1-9]\d* events loaded)(?=.*Lines from other addons .* GatherMate2 1 .* GatherMate2 Testa Fixture \(2447\) map 1429 at 43\.9, 67\.0)/,
    never: /Data folder .* · 0 events loaded|No lines from other addons/,
  },
  { key: "settings", shows: /WoW folders .* E2EONE .* E2ETWO/, never: /No game accounts yet/ },
];

const OLD_ADDRESSES = [
  ["overview", "character"],
  ["classpage", "character/class"],
  ["talents", "character/talents"],
  ["charts", "stats"],
  ["timeline", "log"],
  ["heatmap", "activity"],
  ["compress", "data"],
  ["addons", "data"],
];

const mismatch = (what, want, saw) => new Error(`${what}\n  expected ${want}\n  saw      ${saw}`);
const excerpt = (text) => JSON.stringify(String(text).slice(0, 300));

function expectEqual(what, saw, want) {
  if (JSON.stringify(saw) !== JSON.stringify(want)) throw mismatch(what, JSON.stringify(want), JSON.stringify(saw));
}

function expectMatch(what, text, pattern) {
  if (!pattern.test(text)) throw mismatch(what, String(pattern), excerpt(text));
}

function expectNoMatch(what, text, pattern) {
  if (pattern.test(text)) throw mismatch(what, `nothing matching ${pattern}`, excerpt(text));
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const scarlet = (ctx, ...args) => {
  const { SCARLET_HOME, SCARLET_OUT, SCARLET_SETTINGS, ...env } = process.env;
  const run = spawnSync(ctx.cli, args, { encoding: "utf8", env, timeout: 60_000, windowsHide: true });
  return { code: run.status, out: run.stdout ?? "", err: run.stderr ?? "" };
};

const exportSite = (ctx, site) => scarlet(ctx, "export", "--wow", site.wow, "--out", site.out);

const site = (root) => ({ wow: path.join(root, "wow"), out: path.join(root, "out"), home: path.join(root, "home") });

const listenWebsite = () =>
  new Promise((resolve) => {
    const seen = [];
    const icons = [];
    const server = http.createServer((request, answer) => {
      (request.method === "GET" && /^\/spell-icons\/\d+\.png$/.test(request.url) ? icons : seen).push(`${request.method} ${request.url}`);
      answer.statusCode = 404;
      answer.end();
    });
    server.listen(0, "127.0.0.1", () => resolve({ server, seen, icons, url: `http://127.0.0.1:${server.address().port}` }));
  });

const read = (file) => fs.readFileSync(file, "utf8");

const hashOf = (dir) =>
  Object.fromEntries(
    fs
      .readdirSync(dir)
      .sort()
      .map((name) => [name, crypto.createHash("sha256").update(fs.readFileSync(path.join(dir, name))).digest("hex")]),
  );

const stateFile = (site) => path.join(site.out, "state.json");

const readState = (site) => JSON.parse(read(stateFile(site)));

const NEW_MARKS = /#(?:t|restart|since|bad)$/;

const dropNewMarks = (site) => {
  const old = Object.fromEntries(Object.entries(readState(site)).filter(([key]) => !NEW_MARKS.test(key)));
  fs.writeFileSync(stateFile(site), JSON.stringify(old, null, 2) + "\n");
};

const resetPayloads = (site) => ({
  main: hashOf(path.join(site.out, RESET.name, "payloads")),
  addons: hashOf(path.join(site.out, RESET.name, "addons", "payloads")),
});

const resetSessions = (site) => csvRows(path.join(site.out, RESET.name, "tables/sessions.csv"));

const resetLevel = (site) => JSON.parse(read(path.join(site.out, RESET.name, "overview.json"))).level;

const RESTARTED = /The game's Scarlet store started over on [A-Z][a-z]{2} \d{1,2}(?: \d{4})?; 3 recordings read since\./;
const ADDONS_RESTARTED = /The game's Scarlet addon store started over on [A-Z][a-z]{2} \d{1,2}(?: \d{4})?; 1 recording read since\./;
const UNREADABLE = /1 recording in SavedVariables could not be read\./;

function expectOldFilesKept(what, before, now) {
  for (const [name, hash] of Object.entries(before)) expectEqual(`${what}: ${name} is still there, unchanged`, now[name], hash);
}

const csvRows = (file) => read(file).trim().split(/\r?\n/).length - 1;

const names = (dir) => (fs.existsSync(dir) ? fs.readdirSync(dir).sort() : null);

const registry = (key, value) => spawnSync("reg", ["query", key, ...(value ? ["/v", value] : [])], { encoding: "utf8", windowsHide: true }).stdout ?? "";

const documents = () => {
  const personal = /Personal\s+REG_(?:EXPAND_)?SZ\s+(.+)/.exec(registry(String.raw`HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders`, "Personal"))?.[1].trim();
  return personal ? personal.replace(/%([^%]+)%/g, (whole, name) => process.env[name] ?? whole) : path.join(os.homedir(), "Documents");
};

const footprint = () => {
  const identifier = "app.scarlet.desktop";
  const watched = [
    path.join(process.env.APPDATA ?? "", identifier),
    path.join(process.env.LOCALAPPDATA ?? "", identifier),
    path.join(documents(), "Scarlet"),
    path.join(REPO, "app", "target", "debug"),
    path.join(REPO, "companion", "target", "debug"),
  ];
  return {
    entries: Object.fromEntries(watched.map((place) => [place, names(place)])),
    run: registry(String.raw`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`),
  };
};

const selectCharacter = (app, name) =>
  app.evaluate(`(() => {
    const select = document.querySelector("#char");
    if (select.value === ${JSON.stringify(name)}) return;
    select.value = ${JSON.stringify(name)};
    select.dispatchEvent(new Event("change"));
  })()`);

const patternExpression = (pattern) => `new RegExp(${JSON.stringify(pattern.source)}, ${JSON.stringify(pattern.flags)})`;

async function open(app, key, shows, gone = null, hash = key) {
  await app.evaluate(`location.hash = ${JSON.stringify(hash)}`);
  const without = gone ? ` && !${patternExpression(gone)}.test(${BODY})` : "";
  await app.until(
    `document.querySelector("#nav .item.active")?.dataset.key === ${JSON.stringify(key)} && document.querySelector("#body")?.dataset.ready === "1" && ${patternExpression(shows)}.test(${BODY})${without}`,
    `${hash} shows ${shows}${gone ? ` and no longer ${gone}` : ""}`,
  );
}

const running = (ctx) => {
  if (!ctx.app) throw new Error("the app is not running: the check that starts it failed");
  return ctx.app;
};

const appState = (app) => app.evaluate(`(async () => (await import("/bridge.js")).invoke("app_state"))()`);

const headEvents = (app) => app.evaluate(`Number(/([\\d,]+) events/.exec(document.querySelector("#head-stats").textContent)?.[1].replace(/,/g, "") ?? NaN)`);

function prepareSite(ctx, s) {
  fs.mkdirSync(s.home, { recursive: true });
  writeWow(s.wow);
  const exported = exportSite(ctx, s);
  expectEqual("exporting the fixture for the app", exported.code, 0);
  fs.writeFileSync(
    path.join(s.home, "settings.json"),
    JSON.stringify({
      wow: s.wow,
      data: s.out,
      watch: false,
      last_view: "overview",
      keep_addon: false,
      open_with_wow: false,
      upload: { enabled: false, server: ctx.website.url },
    }),
  );
}

const booted = (app) => app.until(`document.querySelectorAll("#nav .item").length > 0 && document.querySelector("#pagetitle").textContent !== ""`, "the page booted", 10000);

async function startApp(ctx) {
  prepareSite(ctx, ctx.site.app);
  ctx.footprint = footprint();
  ctx.app = await launch({ exe: ctx.exe, home: ctx.site.app.home, reducedMotion: true });
  await booted(ctx.app);
}

const ENTRANCE_PAGES = [
  ["characters", "characters", /Last session/],
  ["sessions", null, /\d[smhd] played/],
];


async function grindApp(ctx) {
  if (ctx.grind) return ctx.grind;
  const s = ctx.site.grind;
  fs.mkdirSync(s.home, { recursive: true });
  writeGrind(s.wow, { alt: true });
  expectEqual("exporting the grind fixture", exportSite(ctx, s).code, 0);
  fs.writeFileSync(path.join(s.home, "settings.json"), JSON.stringify({ wow: s.wow, data: s.out, watch: false, last_view: "overview", keep_addon: false, open_with_wow: false, upload: { enabled: false, server: ctx.website.url } }));
  ctx.grind = await launch({ exe: ctx.exe, home: s.home, reducedMotion: true });
  await booted(ctx.grind);
  await selectCharacter(ctx.grind, GRIND.name);
  return ctx.grind;
}

async function fullApp(ctx) {
  if (ctx.full) return ctx.full;
  const s = ctx.site.full;
  fs.mkdirSync(s.home, { recursive: true });
  writeFullBuild(s.wow, ctx.now);
  expectEqual("exporting the full build fixture", exportSite(ctx, s).code, 0);
  fs.writeFileSync(path.join(s.home, "settings.json"), JSON.stringify({ wow: s.wow, data: s.out, watch: false, last_view: "overview", keep_addon: false, open_with_wow: false, upload: { enabled: false, server: ctx.website.url } }));
  ctx.full = await launch({ exe: ctx.exe, home: s.home, reducedMotion: true });
  await booted(ctx.full);
  await selectCharacter(ctx.full, fullBuild(ctx.now).name);
  return ctx.full;
}

async function tradesApp(ctx) {
  if (ctx.trades) return ctx.trades;
  const s = ctx.site.trades;
  fs.mkdirSync(s.home, { recursive: true });
  writeTrades(s.wow, ctx.now);
  expectEqual("exporting the trades fixture", exportSite(ctx, s).code, 0);
  fs.writeFileSync(path.join(s.home, "settings.json"), JSON.stringify({ wow: s.wow, data: s.out, watch: false, last_view: "overview", keep_addon: false, open_with_wow: false, upload: { enabled: false, server: ctx.website.url } }));
  ctx.trades = await launch({ exe: ctx.exe, home: s.home, reducedMotion: true });
  await booted(ctx.trades);
  await selectCharacter(ctx.trades, trades(ctx.now).name);
  return ctx.trades;
}

const tradesSkills = async (app, pick) =>
  JSON.parse(await app.evaluate(`(async () => { const skills = (await (await import("/bridge.js")).invoke("goals", { ch: "Trades-Fixture" })).caps.skills; return JSON.stringify(${pick}); })()`));

const skillGroups = (app) =>
  app.evaluate(`(() => {
    const groups = {};
    let title = "";
    const mod = [...document.querySelectorAll("#body section.mod")].find((m) => m.querySelector("h2")?.textContent === "Skills");
    for (const node of mod.children) {
      if (node.classList.contains("sc-label")) groups[(title = node.textContent)] = [];
      else if (title && node.classList.contains("goal-list")) for (const row of node.children) groups[title].push(row.innerText.replace(/\\s+/g, " ").trim());
    }
    return groups;
  })()`);

const text = (app, selector) => app.evaluate(`[...document.querySelectorAll(${JSON.stringify(selector)})].map((n) => n.innerText.replace(/\\s+/g, " ").trim())`);

async function openGrind(ctx, key, shows) {
  const app = await grindApp(ctx);
  await open(app, key, shows);
  return app;
}

async function entrancePages(ctx, reducedMotion) {
  const app = await launch({ exe: ctx.exe, home: ctx.site.motion.home, reducedMotion });
  const pages = {};
  const played = {};
  try {
    await booted(app);
    const first = await app.evaluate(`(async () => (await (await import("/bridge.js")).invoke("session_list", { ch: null })).sessions[0])()`);
    await app.evaluate(`location.hash = "data"`);
    await app.until(`document.querySelector("#nav .item.active")?.dataset.key === "data" && document.querySelector("#body.play, #body .play") === null`, "the first page draw is over");
    await app.evaluate(`window.__sawPlay = false; new MutationObserver(() => { if (document.querySelector("#body.play, #body .play")) window.__sawPlay = true; }).observe(document.body, { subtree: true, attributes: true, attributeFilter: ["class"] })`);
    for (const [key, hash, shows] of ENTRANCE_PAGES) {
      await app.evaluate(`window.__sawPlay = false`);
      await open(app, key, shows, null, hash ?? `sessions/${first.from}-${first.to}`);
      if (!reducedMotion) await app.until(`window.__sawPlay`, `${key} starts its entrance`, 1500).catch(() => null);
      await app.until(`!document.querySelector("#body.play, #body .play")`, `${key} finishes its entrance`, 6000);
      pages[key] = await app.evaluate(BODY);
      played[key] = await app.evaluate(`window.__sawPlay`);
    }
  } finally {
    if ((await app.close()) === 3) ctx.breach = true;
  }
  return { pages, played };
}

const CHECKS = [
  {
    lane: "cli",
    name: "export reads the recording",
    run(ctx) {
      const s = ctx.site.cli;
      const run = exportSite(ctx, s);
      expectEqual("exit code", run.code, 0);
      for (const c of CHARACTERS) {
        expectMatch(`${c.name} in the output`, run.out, new RegExp(`${c.name}: ${c.sessions.length} chunks, ${lineCount({ character: c.name })} events`));
        const dir = path.join(s.out, c.name);
        for (const file of ["scarlet.txt", "overview.json", "inventory.json", "tables/sessions.csv", "tables/events.csv", "tables/drops.csv"]) {
          if (!fs.existsSync(path.join(dir, file))) throw mismatch(`${c.name}: a file the export writes`, file, "missing");
        }
        const payloads = fs.readdirSync(path.join(dir, "payloads")).filter((f) => f.endsWith(".scl"));
        if (!payloads.length) throw mismatch(`${c.name}: payloads`, "at least one .scl file", "none");
        expectEqual(`${c.name}: rows in tables/sessions.csv`, csvRows(path.join(dir, "tables/sessions.csv")), c.sessions.length);
      }
      for (const file of ["state.json", "accounts.json"]) {
        if (!fs.existsSync(path.join(s.out, file))) throw mismatch("a file the export writes", file, "missing");
      }
      expectMatch("a known line in Testa-Fixture/scarlet.txt", read(path.join(s.out, TESTA, "scarlet.txt")), /zone\s+Elwynn Forest - Goldshire/);
      const exported = read(path.join(s.out, TESTA, "scarlet.txt"));
      expectMatch("the killing blow of a creature in Testa-Fixture/scarlet.txt", exported, /deathhit\s+Reaper Slam \(5101\) for 412 from Harvest Reaper \(115\), the killing blow \(37 overkill\), avoidable, deadly, the killer was a creature \(level 12, elite\), 7 hits in the recap/);
      expectMatch("the killing blow of a fall in Testa-Fixture/scarlet.txt", exported, /deathhit\s+a fall for 640, the killing blow \(12 overkill\), avoidable, 2 hits in the recap/);
      expectMatch("a known line in Probe-Lane/scarlet.txt", read(path.join(s.out, PROBE, "scarlet.txt")), /kill\s+Kobold Vermin \(6\) by you/);
    },
  },
  {
    lane: "cli",
    name: "The Fishing Log's records and leaderboard are read",
    run(ctx) {
      const file = path.join(ctx.site.cli.out, FISHING.character, "fishing", "payloads", "fishing.scl");
      if (!fs.existsSync(file)) throw mismatch("the Fishing Log payload", file, "missing");
      const lines = zlib.zstdDecompressSync(fs.readFileSync(file)).toString("utf8").trimEnd().split("\n");
      const row = (...fields) => fields.join("\t");
      expectEqual("the payload header", lines[0], row("scarletfishing", 1, FISHING.character));
      const want = [
        row(1795100000000, "JK", "a977", "a", "", 6362, 15.5, FISHING.rival),
        row(1795200000000, "JK", "a977", "s", 24322, 6362, 13.3, FISHING.character),
        row(1795200000000, "JR", "a977", 6362, 13.3, 1),
        row(1795200500000, "JH", "Tanaris", "Wavestrider Beach", 600, 4, 30.25, 13.3, 6362, "Raw Rockscale Cod", "", "", 1, 5, 4, "", "", "", "", "", ""),
        row(1795300000000, "JN", "a977", "Wavestrider Beach", 1446, 5000, 2500, 1),
        row(1795300000000, "JE", FISHING.rival, "A"),
      ];
      for (const line of want) {
        if (!lines.includes(line)) throw mismatch("a Fishing Log line", line, lines.slice(1).join(" | "));
      }
      for (const c of CHARACTERS.filter((c) => c.name !== FISHING.character)) {
        if (fs.existsSync(path.join(ctx.site.cli.out, c.name, "fishing"))) throw mismatch(`${c.name}: a Fishing Log payload`, "none", "written");
      }
    },
  },
  {
    lane: "cli",
    name: "a guild bank read before the guild line is filed under the guild",
    run(ctx) {
      for (const c of CHARACTERS) {
        const inventory = JSON.parse(read(path.join(ctx.site.cli.out, c.name, "inventory.json")));
        expectEqual(`${c.name}: guild_name in inventory.json`, inventory.guild_name, GUILD);
        expectEqual(`${c.name}: guild_money_name in inventory.json`, inventory.guild_money_name, GUILD);
      }
    },
  },
  {
    lane: "cli",
    name: "the addon files are written",
    run(ctx) {
      const dir = path.join(ctx.site.cli.wow, "_forever_", "Interface", "AddOns", "ScarletData");
      for (const file of ["Digest.lua", "Items.lua", "Recaps.lua", "Feats.lua", "ScarletData.toc"]) {
        if (!fs.existsSync(path.join(dir, file))) throw mismatch("a file the export writes for the addon", file, "missing");
      }
      const digest = read(path.join(dir, "Digest.lua"));
      for (const c of CHARACTERS) expectMatch(`Digest.lua names ${c.name}`, digest, new RegExp(`\\["${c.name}"\\]`));
      const recaps = read(path.join(dir, "Recaps.lua"));
      expectMatch("Recaps.lua holds the fixture sessions", recaps, /^ScarletRecaps = \{[\s\S]*\["Testa-Fixture"\][\s\S]*Level 5 to 6 in Elwynn Forest/);
      expectMatch("the toc lists Recaps.lua", read(path.join(dir, "ScarletData.toc")), /^Recaps\.lua$/m);
      const feats = read(path.join(dir, "Feats.lua"));
      expectMatch("Feats.lua is counted locally for the fixture character", feats, /^ScarletFeats = \{[\s\S]*source = "local"[\s\S]*\["Testa-Fixture"\] = \{[\s\S]*held = \d+, total = \d+, score = \d+/);
      expectMatch("Feats.lua gives a counter feat the addon's own count as b", feats, /\["slayer"\] = \{ v = \d+, s = \d+, b = \d+ \}/);
      expectNoMatch("Feats.lua gives a feat that is not a counter no b", feats, /\["veteran"\] = \{[^}]*\bb = /);
      expectMatch("the toc lists Feats.lua", read(path.join(dir, "ScarletData.toc")), /^Feats\.lua$/m);
      expectNoMatch("Digest.lua says nothing of the website when the CLI wrote it", digest, /\bweb = /);
    },
  },
  {
    lane: "cli",
    name: "a second export changes nothing",
    run(ctx) {
      const s = ctx.site.cli;
      const before = CHARACTERS.map((c) => hashOf(path.join(s.out, c.name, "payloads")));
      const run = exportSite(ctx, s);
      expectEqual("exit code", run.code, 0);
      expectMatch("the output", run.out, /no new chunks/);
      expectNoMatch("the output", run.out, /from The Fishing Log/);
      expectEqual("payload files after the second export", CHARACTERS.map((c) => hashOf(path.join(s.out, c.name, "payloads"))), before);
    },
  },
  {
    lane: "cli",
    name: "a later session is picked up",
    run(ctx) {
      const s = ctx.site.cli;
      writeWow(s.wow, { later: true });
      const run = exportSite(ctx, s);
      expectEqual("exit code", run.code, 0);
      expectMatch("the output", run.out, new RegExp(`${LATER.character}: 1 chunks, ${LATER.lines.length} events`));
      expectNoMatch("the output", run.out, new RegExp(`${PROBE}:`));
      expectEqual("rows in tables/sessions.csv", csvRows(path.join(s.out, LATER.character, "tables/sessions.csv")), 3);
    },
  },
  {
    lane: "cli",
    name: "a saved store that started over is read again, and nothing exported before is overwritten",
    run(ctx) {
      const s = ctx.site.reset;
      writeReset(s.wow, "before");
      const first = exportSite(ctx, s);
      expectEqual("exit code", first.code, 0);
      expectMatch("the first export", first.out, /Reset-Fixture: 3 chunks, 9 events/);
      expectNoMatch("the first export", first.out, /started over/);
      const old = resetPayloads(s);
      expectEqual("payload files of the first store", Object.keys(old.main), ["1-3.scl"]);
      expectEqual("addon payload files of the first store", Object.keys(old.addons), ["1-1.scl"]);
      expectEqual("level after the first store", resetLevel(s), 4);
      expectEqual("sessions after the first store", resetSessions(s), 3);

      writeReset(s.wow, "after", { unreadable: true });
      const again = exportSite(ctx, s);
      expectEqual("exit code after the store started over", again.code, 0);
      expectMatch("the export of the new store", again.out, /Reset-Fixture: 3 chunks, 9 events/);
      expectMatch("the export of the new addon store", again.out, /Reset-Fixture: 1 addon chunks, 1 addon lines/);
      expectMatch("the line for the store", again.out, RESTARTED);
      expectMatch("the line for the addon store", again.out, ADDONS_RESTARTED);
      expectMatch("the line for the chunk that did not decode", again.err, UNREADABLE);
      const now = resetPayloads(s);
      expectOldFilesKept("main payloads", old.main, now.main);
      expectOldFilesKept("addon payloads", old.addons, now.addons);
      expectEqual("payload files after the restart", Object.keys(now.main).sort(), ["1-3-1795500000.scl", "1-3.scl"]);
      expectEqual("addon payload files after the restart", Object.keys(now.addons).sort(), ["1-1-1795500000.scl", "1-1.scl"]);
      expectEqual("level after the restart", resetLevel(s), 7);
      expectEqual("sessions after the restart", resetSessions(s), 6);
      const marks = readState(s);
      const key = Object.keys(marks).find((k) => k.endsWith("Scarlet.lua") && k.includes("E2ERESET"));
      expectEqual("the marks of the store", [marks[key], marks[`${key}#t`], marks[`${key}#since`], marks[`${key}#bad`]], [3, 1795514400, 3, 1]);
    },
  },
  {
    lane: "cli",
    name: "a second export after the store started over reads nothing new",
    run(ctx) {
      const s = ctx.site.reset;
      const before = resetPayloads(s);
      const run = exportSite(ctx, s);
      expectEqual("exit code", run.code, 0);
      expectMatch("the output", run.out, /no new chunks \(1 SavedVariables file\(s\), 1 chunk\(s\) failed to decode\)/);
      expectNoMatch("the output", run.out, /started over/);
      expectEqual("payload files after the second export", resetPayloads(s), before);
      expectEqual("sessions after the second export", resetSessions(s), 6);
    },
  },
  {
    lane: "cli",
    name: "a state.json from before the store's times were kept is migrated, and still reads a store that started over",
    run(ctx) {
      const s = ctx.site.legacy;
      writeReset(s.wow, "before");
      expectEqual("exit code of the first export", exportSite(ctx, s).code, 0);
      const old = resetPayloads(s);
      dropNewMarks(s);
      for (const key of Object.keys(readState(s))) if (NEW_MARKS.test(key)) throw mismatch("a state.json in the old shape", "no marks with times", key);

      const migrated = exportSite(ctx, s);
      expectEqual("exit code of the export that migrates", migrated.code, 0);
      expectMatch("the export that migrates", migrated.out, /no new chunks/);
      expectNoMatch("the export that migrates", migrated.out, /started over/);
      expectEqual("payload files after the migration", resetPayloads(s), old);
      const key = Object.keys(readState(s)).find((k) => k.endsWith("Scarlet.lua") && k.includes("E2ERESET"));
      expectEqual("the marks of the store after the migration", [readState(s)[key], readState(s)[`${key}#t`], readState(s)[`${key}#addons#t`]], [3, 1795414400, 1795400000]);

      dropNewMarks(s);
      writeReset(s.wow, "after");
      const again = exportSite(ctx, s);
      expectEqual("exit code after the store started over", again.code, 0);
      expectMatch("the export of the new store", again.out, /Reset-Fixture: 3 chunks, 9 events/);
      expectMatch("the line for the store", again.out, RESTARTED);
      expectMatch("the line for the addon store", again.out, ADDONS_RESTARTED);
      const now = resetPayloads(s);
      expectOldFilesKept("main payloads", old.main, now.main);
      expectOldFilesKept("addon payloads", old.addons, now.addons);
      expectEqual("payload files after the restart", Object.keys(now.main).sort(), ["1-3-1795500000.scl", "1-3.scl"]);
      expectEqual("level after the restart", resetLevel(s), 7);
      expectEqual("sessions after the restart", resetSessions(s), 6);
      const last = exportSite(ctx, s);
      expectMatch("the export after that", last.out, /no new chunks/);
      expectEqual("payload files after the export after that", resetPayloads(s), now);
    },
  },
  {
    lane: "cli",
    name: "a bad invocation fails loudly",
    run(ctx) {
      const unknown = scarlet(ctx, "bogus", "--wow", ctx.site.cli.wow, "--out", ctx.site.cli.out);
      if (unknown.code === 0) throw mismatch("exit code of an unknown command", "not 0", "0");
      expectMatch("the message for an unknown command", unknown.err, /unknown command 'bogus' \(use export, watch, logs or upload\)/);
      const bare = scarlet(ctx);
      expectEqual("exit code with no command", bare.code, 2);
      expectMatch("the usage line", bare.err, /usage: scarlet <export\|watch\|logs\|upload>/);
    },
  },
  {
    lane: "app",
    name: "the app starts",
    async run(ctx) {
      await startApp(ctx);
      const page = await ctx.app.evaluate(
        `({ items: document.querySelectorAll("#nav .item").length, title: document.querySelector("#pagetitle").textContent, ver: document.querySelector("#ver").textContent })`,
      );
      expectEqual("items in the nav", page.items, new Set(VIEWS.map((v) => v.key)).size);
      expectNoMatch("the page title", page.title, /^boot:/);
      expectEqual("the version", page.ver, "dev");
    },
  },
  {
    lane: "app",
    name: "the store loaded the recording",
    async run(ctx) {
      const app = running(ctx);
      const store = await appState(app);
      expectEqual("characters in app_state", store.chars, [PROBE, TESTA]);
      expectEqual("sessions in app_state", store.sessions, 3);
      const foot = await app.evaluate(`document.querySelector("#foot").textContent`);
      expectMatch("the foot of the nav", foot, new RegExp(`${CHARACTERS.length} ${CHARACTERS.length === 1 ? "character" : "characters"}`));
      expectMatch("the foot says uploading is off", foot, /Uploading is off/);
    },
  },
  ...VIEWS.map(({ key, hash = key, shows, never }) => ({
    lane: "app",
    name: `${hash} shows the selected character`,
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, key, shows, OTHER_CHARACTER_ONLY.has(key) ? OTHER_CHARACTER : null, hash);
      expectNoMatch(`${hash} shows its empty state`, await app.evaluate(BODY), never);
    },
  })),
  {
    lane: "app",
    name: "old addresses land on the view that replaced them",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      for (const [old, now] of OLD_ADDRESSES) {
        await app.evaluate(`location.hash = ${JSON.stringify(old)}`);
        await app.until(`location.hash === ${JSON.stringify(`#${now}`)}`, `${old} lands on ${now}`);
      }
      await selectCharacter(app, "");
      await app.evaluate(`location.hash = "overview"`);
      await app.until(`location.hash === "#characters"`, "overview with no character picked lands on characters");
      await selectCharacter(app, TESTA);
    },
  },
  {
    lane: "app",
    name: "the component sheet draws every component",
    async run(ctx) {
      const app = running(ctx);
      await app.evaluate(`location.hash = "system"`);
      await app.until(`/Feat crest: the shield is the scope/.test(${BODY}) && /Link entry points/.test(${BODY})`, "the component sheet shows its sections");
      const drawn = await app.evaluate(`({ crests: document.querySelectorAll("#body .sc-crest svg").length, charts: document.querySelectorAll("#body svg.sc-svg").length, cmp: document.querySelectorAll("#body .sc-cmp").length, rings: document.querySelectorAll("#body .sc-ring svg").length, active: document.querySelector("#nav .item.active")?.dataset.key ?? null })`);
      expectEqual("the component sheet has no nav entry", drawn.active, null);
      if (drawn.crests < 19 || drawn.charts < 3 || drawn.cmp < 3 || drawn.rings < 1) throw mismatch("components on the sheet", "19 crests, 3 charts, 3 compare strips, a ring", JSON.stringify(drawn));
    },
  },
  {
    lane: "app",
    name: "the run summary shows the run around a session",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, PROBE);
      const shows = /^(?=.*Level 5 to 6 in Elwynn Forest)(?=.*Testa Fixture)(?=.*1,605 XP in)(?=.*2 kills in)(?=.*Experience every 5 minutes)(?=.*Killed most)(?=.*Levels, deaths and feats)(?!.*Probe Lane)/;
      await app.evaluate(`location.hash = "sessions/1795000000000-1795000130000"`);
      await app.until(`${patternExpression(shows)}.test(${BODY})`, `the run summary shows ${shows}`);
    },
  },
  {
    lane: "app",
    name: "the Character page leads with progress to the level cap and a Records row, and a session page shows its pace",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, "character", /Records/);
      const drawn = await app.evaluate(`({ levelBar: document.querySelectorAll("#body .goals-sheet .sc-goal.lg .sg-t > i").length, figure: document.querySelector("#body .goals-sheet .sc-goal.lg .sg-h")?.textContent.replace(/\\s+/g, " ").trim() ?? "", records: document.querySelectorAll("#body .goals-sheet .sc-record").length, first: document.querySelector("#body .goals-sheet > section")?.querySelector("h2")?.textContent ?? "" })`);
      expectEqual("the level goal figure", drawn.figure, "6 / 60");
      expectEqual("goal bars for the level", drawn.levelBar, 1);
      expectEqual("the first module of the Character page", drawn.first, "Level");
      if (drawn.records < 1) throw mismatch("tiles in the Records row", "at least one", String(drawn.records));
      await app.evaluate(`location.hash = "sessions/1795000000000-1795000130000"`);
      await app.until(`document.querySelectorAll("#body .sc-pace").length > 0`, "the session page shows its pace tiles");
      const pace = await app.evaluate(`[...document.querySelectorAll("#body .sc-pace")].map((p) => p.innerText.replace(/\\s+/g, " ").trim())`);
      if (!pace.some((p) => /Under 15 minutes|of 3 sessions to compare|Middle half/.test(p))) throw mismatch("a pace tile's comparison state", "Under 15 minutes, N of 3 sessions to compare or Middle half", JSON.stringify(pace));
    },
  },
  {
    lane: "app",
    name: "the Characters page draws each character's progress to its caps and a gold goal",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "characters", /Last session/);
      const drawn = await app.evaluate(`({ cards: document.querySelectorAll("#body .ch-card").length, levels: [...document.querySelectorAll("#body .ch-card .ch-caps .sc-goal .sg-n")].map((n) => n.textContent.replace(/\\s+/g, " ").trim()), gold: document.querySelectorAll("#body #r-gold-goals .sc-goal, #body #r-gold-goals .gold-best").length })`);
      expectEqual("character cards", drawn.cards, CHARACTERS.length);
      for (const level of ["6 / 60", "3 / 60"]) if (!drawn.levels.includes(level)) throw mismatch("a card's level goal", level, JSON.stringify(drawn.levels));
      if (drawn.gold < 1) throw mismatch("goals under the gold figure", "at least one", String(drawn.gold));
    },
  },
  {
    lane: "app",
    name: "the Loot page draws a goal for a notable drop not yet received",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "loot", /Drop rates/);
      const drawn = await app.evaluate(`[...document.querySelectorAll("#body .lt-chase .lt-miss")].map((g) => g.innerText.replace(/\\s+/g, " ").trim())`);
      if (!drawn.some((g) => /Wool Cloth/.test(g) && /^Wool Cloth 1 of 1 corpse$/.test(g))) throw mismatch("a row for Wool Cloth dropped and not received", "Wool Cloth 1 of 1 corpse", JSON.stringify(drawn));
    },
  },
  {
    lane: "app",
    name: "the Items page draws the value held against its best and each item's farmed total",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "items", /Value held/);
      const drawn = await app.evaluate(`({ goal: document.querySelector("#body .it .sc-tile")?.innerText.replace(/\\s+/g, " ").trim() ?? "", farmed: [...document.querySelectorAll("#body .it-farm")].map((n) => n.textContent.trim()) })`);
      if (!/^Carried by characters 4s ?50c Best 4s ?50c · .* Priced: 1 of 2 kinds$/.test(drawn.goal)) throw mismatch("the value held", "Carried by characters 4s 50c Best 4s 50c · day Priced: 1 of 2 kinds", drawn.goal);
      expectEqual("a goal bar on the value held", await app.evaluate(`document.querySelectorAll("#body .it .sc-goal").length`), 0);
      if (!drawn.farmed.includes("3 looted")) throw mismatch("the farmed total of Linen Cloth", "3 looted", JSON.stringify(drawn.farmed));
    },
  },
  {
    lane: "app",
    name: "the Legacy page draws the three trees from the snapshot, locked placeholders, the arrows between talents and each talent once as a goal",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, "legacy", /Earned/);
      expectEqual("the points earned", (await text(app, "#body .legacy-sheet .leg-earned"))[0], "Earned 2 / 65 3%");
      expectEqual("the points bar", await app.evaluate(`document.querySelector("#body .legacy-sheet .leg-pips").getAttribute("aria-label")`), "2 of 65 points earned, 0 spent, 2 to spend");
      expectEqual("the trees", await text(app, "#body .tf-tab[data-tree] h3"), ["Professions · 0 points", "Adventure · 0 points", "Progression · 0 points"]);
      expectEqual("slots drawn: 7 talents and 2 placeholders in each of 3 trees", await app.evaluate(`[document.querySelectorAll("#body .tf-slot").length, document.querySelectorAll("#body .tf-slot.locked").length]`), [27, 6]);
      expectEqual("a talent never taken is drawn untaken", await app.evaluate(`document.querySelector('#body .tf-slot[data-node="110316"]').getAttribute("aria-label") + "|" + document.querySelector('#body .tf-slot[data-node="110316"]').dataset.state`), "Permanence, rank 0 of 2|untaken");
      expectEqual("Gourmand holds its rank, shown before its icon arrives", await app.evaluate(`(() => { const s = document.querySelector('#body .tf-slot[data-node="110313"]'); return [s.getAttribute("aria-label"), s.dataset.state, s.querySelector(".tf-rank").textContent, s.querySelector(".tf-init").textContent]; })()`), ["Gourmand, rank 1 of 3", "partial", "1/3", "G"]);
      expectEqual("the arrow from Gourmand to Permanence is lit by Gourmand's rank", await app.evaluate(`document.querySelector('#body .tf-arrow[data-from="110313"][data-to="110316"]')?.classList.contains("lit")`), true);
      expectEqual("an arrow between untaken talents is drawn unlit", await app.evaluate(`document.querySelector('#body .tf-arrow[data-from="110315"][data-to="110309"]')?.classList.contains("lit")`), false);
      expectEqual("the goals outside the points head", await text(app, "#body .sc-goal"), ["Gourmand 1 / 3 33% Next rank costs 1 · Can afford"]);
      for (let waited = 0; waited < 3000 && !ctx.website.icons.length; waited += 100) await sleep(100);
      expectEqual("the icon of Gourmand was asked for by its id alone", ctx.website.icons.includes("GET /spell-icons/134020.png"), true);
    },
  },
  {
    lane: "app",
    name: "the Talents page draws each tab as the in-game frame: untaken talents, ranks, arrows and a timeline by level",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, "character", /Points spent/, null, "character/talents");
      expectEqual("the points goal: 3 spent and the 1 unspent point; the tabs' own available counts (0 and 3, the other tabs' points) are not points to spend", (await text(app, "#body .points-head .sc-goal.lg"))[0], "Points spent 3 / 4 75%");
      expectEqual("the split across the tabs", await text(app, "#body .tab-split .sc-key span"), ["Arms 3", "Fury 0"]);
      expectEqual("the tabs", await text(app, "#body .tf-tab h3"), ["Arms · 3 points", "Fury · 0 points"]);
      const slots = await app.evaluate(`[...document.querySelectorAll("#body .tf-slot")].map((s) => [s.dataset.node, s.dataset.state, s.getAttribute("aria-label"), s.querySelector(".tf-rank")?.textContent ?? "", s.style.left, s.style.top])`);
      expectEqual("every visible node of both tabs, on the grid: the typo'd 33300 reads as row 2", slots, [
        ["9001", "partial", "Improved Heroic Strike, rank 2 of 3", "2/3", "0px", "0px"],
        ["9002", "maxed", "Deflection, rank 2 of 2", "2/2", "0px", "72px"],
        ["9003", "untaken", "Overpower, rank 0 of 1", "0/1", "72px", "72px"],
        ["9004", "untaken", "Slam, rank 0 of 1. Choice of Slam or Whirlwind", "0/1", "216px", "144px"],
        ["9010", "untaken", "Rampage, rank 0 of 1", "0/1", "0px", "0px"],
        ["9011", "untaken", "Bloodthirst, rank 0 of 2", "0/2", "72px", "144px"],
      ]);
      expectEqual("the arrows: lit from a talent that holds a rank, unlit from one that holds none", await app.evaluate(`[...document.querySelectorAll("#body .tf-arrow")].map((a) => [a.dataset.from, a.dataset.to, a.classList.contains("lit")])`), [["9001", "9002", true], ["9010", "9011", false]]);
      const picks = await text(app, "#body .tp-pick");
      if (!picks.some((p) => /^I Improved Heroic Strike Picked · point 4 · Arms · .* 1→2\/3$/.test(p))) throw mismatch("the pick on the timeline", "I Improved Heroic Strike Picked · point 4 · Arms ... 1→2/3", JSON.stringify(picks));
      expectEqual("the timeline groups by level", await text(app, "#body .tp-lv"), ["Level 6"]);
    },
  },
  {
    lane: "app",
    name: "the Stats page draws records, the latest run's pace and the rates run by run, and a record opens its run",
    async run(ctx) {
      const app = await openGrind(ctx, "stats", /Records/);
      const records = await text(app, "#body .stats-goals .sc-record");
      for (const want of [/^Most XP in a session 3,000 /, /^Most kills in a session 10 /, /^Longest session 25m /]) {
        if (!records.some((r) => want.test(r))) throw mismatch("a record tile", String(want), JSON.stringify(records));
      }
      const paces = await text(app, "#body .stats-goals .sc-pace");
      if (!paces.some((p) => /^Experience New best for levels \d+.\d+ 7,200an hour 3,000 XP in 25m/.test(p))) throw mismatch("the experience pace of the latest run", "New best, 7,200an hour, 3,000 XP in 25m", JSON.stringify(paces));
      const sparks = await app.evaluate(`document.querySelectorAll("#body .stats-goals .stat-trend .sc-spark").length`);
      if (sparks < 2) throw mismatch("trend lines under the rates", "at least 2", String(sparks));
      await app.evaluate(`[...document.querySelectorAll("#body .stats-goals .sc-record")].find((r) => /Most XP/.test(r.innerText)).click()`);
      await app.until(`/3,000 XP in 25m/.test(${BODY})`, "the record opens the run with its experience");
    },
  },
  {
    lane: "app",
    name: "the Activity page draws the longest day, week and streak, this week against earlier weeks, and the current streak",
    async run(ctx) {
      const app = await openGrind(ctx, "activity", /Records/);
      const records = await text(app, "#body .goals-sheet .sc-record");
      for (const want of [/^Longest day 25m /, /^Most played week 1h 15m /, new RegExp(`^Longest streak 6 days ${shortDay(GRIND.sessions[0].start * 1000)} to ${shortDay(GRIND.sessions[5].start * 1000)}`)]) {
        if (!records.some((r) => want.test(r))) throw mismatch("a record tile", String(want), JSON.stringify(records));
      }
      const pace = (await text(app, "#body .goals-sheet .sc-pace"))[0] ?? "";
      if (new Date().getDay() === 1) expectEqual("this week's pace tile on a Monday", pace, "");
      else if (!/^Played this week through yesterday .*(Middle half|\d+[smhd]$)/.test(pace)) throw mismatch("this week's tile", "Played this week through yesterday, with Middle half only once 3 weeks exist", pace);
      expectNoMatch("a comparison before 3 weeks exist", await app.evaluate(BODY), /of 3 weeks to compare/);
      const streak = (await text(app, "#body .goals-sheet .sc-goal"))[0] ?? "";
      if (!/^Current streak \d+ days? \/ 6 days?/.test(streak)) throw mismatch("the current streak goal", "Current streak N days / 6 days", streak);
      await app.evaluate(`[...document.querySelectorAll("#body .goals-sheet .sc-record")].find((r) => /Longest day/.test(r.innerText)).click()`);
      await app.until(`/25m/.test(${BODY}) && location.hash.startsWith("#sessions/")`, "the longest day opens its run");
      await open(app, "activity", /Records/);
      const week = (await text(app, "#body .goals-sheet .sc-record")).find((r) => /^Most played week /.test(r));
      const played = week?.match(/^Most played week (\d+h \d+m) /)?.[1];
      if (!played) throw mismatch("the tile of the most played week", "Most played week 1h 15m", String(week));
      await app.evaluate(`[...document.querySelectorAll("#body .goals-sheet .sc-record")].find((r) => /Most played week/.test(r.innerText)).click()`);
      await app.until(`/-days$/.test(location.hash) && document.querySelector("#body .recap")`, "the most played week opens its days");
      const opened = await app.evaluate(BODY);
      expectMatch("the opened week's played time", opened, new RegExp(`${SHOWN.grind} · .* · ${played} played`));
      if (OTHER_GRIND.test(opened)) throw mismatch("the opened week's characters", `only ${SHOWN.grind}`, excerpt(opened));
      expectEqual("the characters of the opened week", await text(app, "#body .recap .sc-seg button"), []);
    },
  },
  {
    lane: "app",
    name: "the Log page heads the list with each run's duration, experience, gold and kills, and a mark where a run set a new best",
    async run(ctx) {
      const app = await openGrind(ctx, "log", /Event types/);
      await app.until(`document.querySelectorAll("#body .lg-cell").length === ${GRIND.sessions.length}`, "a summary for each run");
      const cells = await text(app, "#body .lg-cell");
      if (!/^25m .* XP 3,000 Gold 0 Kills 10 New best experience/.test(cells[0])) throw mismatch("the newest run's summary", "25m ... XP 3,000 ... New best experience", cells[0]);
      if (/New best/.test(cells[cells.length - 1])) throw mismatch("the oldest run's summary", "no New best", cells[cells.length - 1]);
      await app.evaluate(`document.querySelectorAll("#body .lg-cell")[1].click()`);
      await app.until(`document.querySelectorAll("#body .lg-cell.on").length === 1`, "a run is picked");
      await selectCharacter(app, "");
      await app.until(`document.querySelectorAll("#body .lg-cell").length > 0 && [...document.querySelectorAll("#body .lg-cell")].every((c) => c.innerText.includes(${JSON.stringify(SHOWN.alt)}))`, "a summary of both characters");
      expectEqual("summaries of both characters: one for each WoW run, not for each session", (await text(app, "#body .lg-cell")).length, GRIND.sessions.length);
      const account = await text(app, "#body .lg-cell");
      if (!account.every((c) => new RegExp(`^30m .* ${SHOWN.grind}, ${SHOWN.alt} XP [\\d,]+ Gold 0 Kills 20`).test(c)) || !/XP 6,000 /.test(account[0])) throw mismatch("a WoW run of both characters", `30m ... ${SHOWN.grind}, ${SHOWN.alt} XP 6,000 Gold 0 Kills 20 for the newest`, JSON.stringify(account));
      await selectCharacter(app, GRIND.name);
    },
  },
  {
    lane: "app",
    name: "the run summary carries one titled recap for each character",
    async run(ctx) {
      const app = running(ctx);
      const summary = await app.evaluate(`(async () => (await import("/bridge.js")).invoke("run_summary", { from: 1795000000000, to: 1795000130000 }))()`);
      expectEqual("the summary fields", Object.keys(summary).sort(), ["chars", "from", "paces", "played", "recaps", "sessions", "to"]);
      expectEqual("recaps in the run", summary.recaps.map((r) => r.character), ["Testa-Fixture"]);
      const recap = summary.recaps[0];
      expectEqual("the recap title", recap.title, { rule: "level", text: "Level 5 to 6 in Elwynn Forest" });
      expectEqual("the recap levels", recap.level, [5, 6]);
      expectEqual("experience by source", [recap.xp.kills, recap.xp.quests], [205, 1400]);
      expectEqual("the buckets", [recap.buckets.secs, recap.buckets.xp.length, recap.buckets.xp.reduce((a, b) => a + b, 0)], [300, 1, 1605]);
      expectEqual("each session row has a title", summary.sessions.every((s) => typeof s.title === "string" && s.title.length > 0), true);
    },
  },
  {
    lane: "app",
    name: "the sessions list is newest first, steps through sessions, and follows the session mode",
    async run(ctx) {
      const app = running(ctx);
      const file = path.join(ctx.site.app.home, "settings.json");
      const cards = () => app.evaluate(`[...document.querySelectorAll("#body .sc-sessions .sc-session")].map((c) => c.innerText.replace(/\\s+/g, " "))`);
      const cardCount = (n, what) => app.until(`document.querySelectorAll("#body .sc-sessions .sc-session").length === ${n}`, what);
      const press = (text, scope = "#body button") => app.evaluate(`[...document.querySelectorAll(${JSON.stringify(scope)})].find((b) => b.textContent === ${JSON.stringify(text)}).click()`);
      const modeSaved = async (mode) => {
        let saved;
        for (let i = 0; i < 150; i++) {
          saved = JSON.parse(read(file));
          if (saved.session_mode === mode) break;
          await sleep(20);
        }
        expectEqual("session_mode in settings.json", saved.session_mode, mode);
        return saved;
      };
      await selectCharacter(app, "");
      await app.evaluate(`location.hash = "sessions"`);
      await cardCount(3, "the list shows three WoW runs");
      const [newest, middle, oldest] = await cards();
      expectMatch("the newest card", newest, /Probe Lane/);
      expectMatch("the second card", middle, /2m in Westfall/);
      expectMatch("the oldest card", oldest, /Level 5 to 6 in Elwynn Forest/);

      await app.evaluate(`document.querySelector("#body .sc-sessions .sc-session").click()`);
      await app.until(`/^#sessions\\/\\d+-\\d+$/.test(location.hash) && document.querySelector("#body .recap-actions select")`, "a card opens its session page with the session select");
      const bar = await app.evaluate(`({ older: !!document.querySelector('#body [aria-label="Older session"]'), newer: !!document.querySelector('#body [aria-label="Newer session"]'), options: [...document.querySelectorAll("#body .recap-actions select option")].map((o) => o.textContent) })`);
      expectEqual("the newest session has an Older button and no Newer button", [bar.older, bar.newer], [true, false]);
      expectEqual("sessions in the select", bar.options.length, 3);
      expectMatch("the first option of the select", bar.options[0], /Probe Lane/);
      await app.evaluate(`document.querySelector('#body [aria-label="Older session"]').click()`);
      await app.until(`/Level 5 to 6|Westfall/.test(${BODY}) && document.querySelector('#body [aria-label="Newer session"]')`, "Older opens the session before it");

      await app.evaluate(`location.hash = "sessions"`);
      await cardCount(3, "the list is back");
      await press("Days", "#body .sc-seg button");
      await modeSaved("day");
      await cardCount(3, "Days shows one card for each of the three days");
      await app.evaluate(`document.querySelector("#body .sc-sessions .sc-session").click()`);
      await app.until(`/(?:Sun|Mon|Tue|Wed|Thu|Fri|Sat) [A-Z][a-z]{2} \\d{1,2}(?: \\d{4})? .* · 1 session · .* played/.test(${BODY})`, "a day's page reads as a day");
      await app.evaluate(`location.hash = "sessions"`);
      await cardCount(3, "the list is back in Days");
      await press("WoW runs", "#body .sc-seg button");
      await modeSaved("auto");
      await cardCount(3, "WoW runs shows three cards again");

      const saved = JSON.parse(read(file));
      fs.writeFileSync(file, JSON.stringify({ ...saved, session_marks: [{ from: 1795000040000, to: 1795000100000 }] }));
      await press("Manual", "#body .sc-seg button");
      await modeSaved("manual");
      await cardCount(1, "Manual shows the one marked session");
      expectMatch("the marked session's card", (await cards())[0], /1m /);
      await app.evaluate(`document.querySelector("#body .sc-sessions .sc-session").click()`);
      await app.until(`/1m played/.test(${BODY}) && /205 XP in/.test(${BODY}) && !/1,605/.test(${BODY})`, "a marked session's page holds only the minute inside it");
      await app.evaluate(`location.hash = "sessions"`);
      await cardCount(1, "the list is back in Manual");
      await press("Start session");
      await cardCount(2, "Start session adds a card");
      expectMatch("the card of the running session", (await cards())[0], /In progress/);
      await app.until(`[...document.querySelectorAll("#body button")].some((b) => b.textContent === "Stop session")`, "Stop session is offered");
      await press("Stop session");
      await cardCount(1, "Stop session leaves only the marked session");
      await press("WoW runs", "#body .sc-seg button");
      await modeSaved("auto");
      await cardCount(3, "WoW runs shows three cards after Manual");
      fs.writeFileSync(file, JSON.stringify({ ...JSON.parse(read(file)), session_marks: [] }));
    },
  },
  {
    lane: "app",
    name: "the timeline reads each hit of a death recap",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, "log", /kill Stonetusk Boar/);
      const search = async (term, shows) => {
        await app.evaluate(`(() => { const q = document.querySelector('#body input[placeholder^="Search"]'); q.value = ${JSON.stringify(term)}; q.dispatchEvent(new Event("input")); })()`);
        await app.until(`${patternExpression(shows)}.test(${BODY})`, `the timeline searched for ${term} shows ${shows}`);
      };
      await search("avoidable", /^(?=.*Reaper Slam \(5101\) for 412 from Harvest Reaper \(115\), the killing blow \(37 overkill\), avoidable, deadly, the killer was a creature \(level 12, elite\), 7 hits in the recap)(?=.*a fall for 640, the killing blow \(12 overkill\), avoidable, 2 hits in the recap)(?!.*from the environment)/);
      await search("before death", /^(?=.*a melee swing for 88 from Harvest Reaper, 1s before death)(?=.*Reaper Slam \(5101\) for 150 from Harvest Reaper, 55 absorbed, 3s before death)(?=.*a ranged shot for 61 from Defias Thug)(?=.*Rust Bleed \(5102\) for 20 from Harvest Reaper, 7 resisted, 3 blocked, 5s before death)(?=.*a melee swing for 70 from Harvest Reaper, 8s before death)(?!.*spell \?)/);
    },
  },
  {
    lane: "app",
    name: "a death counts once and a fall is named as the killer",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      const spots = (map, ch) => app.evaluate(`(async () => (await import("/bridge.js")).invoke("map_layers", { map: ${map}, ch: ${JSON.stringify(ch)} }))().then((l) => l.deaths.length)`);
      await open(app, "character", /Deaths .* 2 deaths\. Killed most by (Fall|Harvest Reaper), 1 time\./);
      await open(app, "stats", / 2 Deaths /);
      await open(app, "stats", /What killed you (Fall 1 Harvest Reaper 1|Harvest Reaper 1 Fall 1) /);
      expectEqual("death places of Testa in Westfall", await spots(WESTFALL, TESTA), 2);
      const killers = JSON.parse(read(path.join(ctx.site.app.out, TESTA, "overview.json"))).killers;
      expectEqual("killers in Testa-Fixture/overview.json", JSON.stringify(killers.map((k) => `${k.name} ${k.n}`).sort()), JSON.stringify(["Fall 1", "Harvest Reaper 1"]));
      await selectCharacter(app, PROBE);
      await open(app, "character", /Deaths .* 1 death\. Killed most by Kobold Vermin, 1 time\./, /2 deaths/);
      await open(app, "stats", / 1 Deaths /);
      await open(app, "stats", /What killed you Kobold Vermin 1 /, /Kobold Vermin 2/);
      expectEqual("death places of Probe in Elwynn", await spots(ELWYNN, PROBE), 1);
    },
  },
  {
    lane: "app",
    name: "feats are counted locally and send nothing while uploading is off, and lead to the website settings",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, "feats", FEATS_LOCAL);
      await selectCharacter(app, "");
      await open(app, "feats", FEATS_LOCAL_ALL);
      await sleep(300);
      expectEqual("requests the website address received", ctx.website.seen, []);
      await app.evaluate(`[...document.querySelectorAll("#body button")].find((b) => b.textContent === "Website settings").click()`);
      await app.until(`location.hash === "#settings/website"`, "the button opens the website settings");
    },
  },
  {
    lane: "app",
    name: "the feats page search and collection chips narrow the account block and the character rows",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "feats", FEATS_LOCAL_ALL);
      const feats = () => app.evaluate(`document.querySelectorAll("#body .feats-account .sc-wall li").length`);
      const blocks = () => app.evaluate(`[...document.querySelectorAll("#body .feats-block")].filter((b) => !b.hidden).length`);
      const settle = (expression, label) => app.until(expression, label);
      const all = await feats();
      const rows = await blocks();
      if (!(all > 1)) throw new Error(`the account block shows ${all} feats`);
      const chips = await app.evaluate(`[...document.querySelectorAll("#body .feats-view > .feats-controls .sc-chipbtn")].map((b) => b.textContent)`);
      if (!/^All \d+$/.test(chips[0]) || Number(chips[0].slice(4)) < all) throw new Error(`the first chip reads ${chips[0]}, the account block shows ${all}`);
      if (chips.length < 3) throw new Error(`only ${chips.length} chips: ${chips}`);
      await app.evaluate(`(() => {
        const input = document.querySelector("#body .feats-view > .feats-controls input");
        input.value = "zzzz";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      })()`);
      await settle(`document.querySelectorAll("#body .feats-account .sc-wall li").length === 0 && /No feats match\\./.test(${BODY})`, "a search with no match shows No feats match.");
      expectEqual("blocks left with nothing matching", await blocks(), 0);
      const name = await app.evaluate(`(() => {
        const input = document.querySelector("#body .feats-view > .feats-controls input");
        input.value = "";
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return document.querySelector("#body .feats-account .sc-wall li button").title.slice(0, 4).toLowerCase();
      })()`);
      await settle(`document.querySelectorAll("#body .feats-account .sc-wall li").length === ${all}`, "clearing the search restores every feat");
      await app.evaluate(`(() => {
        const input = document.querySelector("#body .feats-view > .feats-controls input");
        input.value = ${JSON.stringify(name)};
        input.dispatchEvent(new Event("input", { bubbles: true }));
      })()`);
      await settle(`(() => { const n = document.querySelectorAll("#body .feats-account .sc-wall li").length; return n > 0 && n < ${all}; })()`, `typing "${name}" narrows the feats`);
      await app.evaluate(`(() => {
        const input = document.querySelector("#body .feats-view > .feats-controls input");
        input.value = "";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      })()`);
      await settle(`document.querySelectorAll("#body .feats-account .sc-wall li").length === ${all}`, "search cleared");
      await app.evaluate(`document.querySelectorAll("#body .feats-view > .feats-controls .sc-chipbtn")[1].click()`);
      await settle(`(() => { const n = document.querySelectorAll("#body .feats-account .sc-wall li").length; return n > 0 && n < ${all}; })()`, "a collection chip narrows the feats");
      await app.evaluate(`document.querySelectorAll("#body .feats-view > .feats-controls .sc-chipbtn")[0].click()`);
      await settle(`document.querySelectorAll("#body .feats-account .sc-wall li").length === ${all}`, "All restores every feat");
      expectEqual("character rows shown again", await blocks(), rows);
    },
  },
  {
    lane: "app",
    name: "with motion on, the entrance ends on the numbers reduced motion shows",
    async run(ctx) {
      prepareSite(ctx, ctx.site.motion);
      const moving = await entrancePages(ctx, false);
      const still = await entrancePages(ctx, true);
      for (const [key] of ENTRANCE_PAGES) {
        expectEqual(`the ${key} entrance ran with motion on`, moving.played[key], true);
        expectEqual(`the ${key} entrance never ran under reduced motion`, still.played[key], false);
        if (moving.pages[key] !== still.pages[key]) throw mismatch(`${key} after its entrance`, excerpt(still.pages[key]), excerpt(moving.pages[key]));
      }
    },
  },
  {
    lane: "app",
    name: "the character filter works",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "characters", new RegExp(`^(?=.*Last session)(?=.*${SHOWN.testa})(?=.*${SHOWN.probe})`));
      await selectCharacter(app, PROBE);
      await open(app, "character", /Probe Lane Level 3 Gnome Mage/, new RegExp(`Totals|Whole life|${SHOWN.testa}`));
    },
  },
  {
    lane: "app",
    name: "the website settings offer Counts from other addons, on",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await app.evaluate(`location.hash = "settings/website"`);
      await app.until(
        `[...document.querySelectorAll("#body .switch")].some((s) => s.textContent.includes("Counts from other addons") && s.classList.contains("on") && s.getAttribute("aria-checked") === "true")`,
        "the website settings show Counts from other addons switched on",
      );
      expectEqual("requests the website address received", ctx.website.seen, []);
    },
  },
  {
    lane: "app",
    name: "a setting survives",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "settings", /Group characters by game account/);
      await app.evaluate(
        `[...document.querySelectorAll("#body .switch")].find((s) => s.textContent.includes("Group characters by game account")).click()`,
      );
      const file = path.join(ctx.site.app.home, "settings.json");
      let saved;
      for (let i = 0; i < 150; i++) {
        saved = JSON.parse(read(file));
        if (saved.group_by_account === true) break;
        await sleep(20);
      }
      expectEqual("group_by_account in settings.json", saved.group_by_account, true);
      await open(app, "characters", new RegExp(`^(?=.*game account E2EONE)(?=.*game account E2ETWO)(?=.*${SHOWN.testa})(?=.*${SHOWN.probe})`, "i"));
    },
  },
  {
    lane: "app",
    name: "characters of unlisted game accounts keep their own warband banks",
    async run(ctx) {
      const app = running(ctx);
      const wow = ctx.site.app.wow;
      await selectCharacter(app, "");
      for (const c of CHARACTERS) fs.rmSync(savedVariablesPath(wow, c));
      try {
        await open(app, "settings", /WoW folders/);
        await open(
          app,
          "items",
          /Linen Cloth 3 looted Testa Fixture 3 Warband bank \(account unknown, Probe Lane\) 5 Warband bank \(account unknown, Testa Fixture\) 20 28 42s 1s ?50c each .* Guild bank 98g ?76s 7 items · 1 kind · Last read .* by Probe Lane/,
          /Warband bank \(E2E|Guild bank \(/,
        );
      } finally {
        writeWow(wow);
      }
    },
  },
  {
    lane: "app",
    name: "open folder is allowed and takes a folder key, not a path",
    async run(ctx) {
      const app = running(ctx);
      const answer = (args) =>
        app.evaluate(
          `window.__TAURI_INTERNALS__.invoke("open_folder", ${JSON.stringify(args)}).then(() => "opened", (e) => String(e))`,
        );
      expectEqual("open_folder with an unknown key", await answer({ which: "elsewhere" }), "Scarlet has no folder called elsewhere.");
      const raw = await answer({ path: ctx.site.app.home });
      if (!/missing required key which/.test(raw)) throw mismatch("open_folder with a raw path", "an answer that names the missing key", raw);
    },
  },
  {
    lane: "app",
    name: "run setup again walks the three steps",
    async run(ctx) {
      const app = running(ctx);
      const clickSetup = async (name) => {
        await app.until(
          `[...document.querySelectorAll("#setup button")].some((b) => b.textContent === ${JSON.stringify(name)})`,
          `the setup shows the ${name} button`,
        );
        await app.evaluate(`[...document.querySelectorAll("#setup button")].find((b) => b.textContent === ${JSON.stringify(name)}).click()`);
      };
      await open(app, "settings", /WoW folders/);
      await app.evaluate(`[...document.querySelectorAll("#body button")].find((b) => b.textContent === "Run setup again").click()`);
      await app.until(
        `document.querySelector("#setup")?.innerText.includes("Find WoW Forever") && document.querySelector("#shell").hidden === true`,
        "the setup shows step 1 and hides the shell",
      );
      await clickSetup("Continue");
      await app.until(`document.querySelector("#setup")?.innerText.includes("Install the Scarlet addon")`, "the setup shows step 2");
      await clickSetup("Skip");
      await app.until(`document.querySelector("#setup")?.innerText.includes("Link this app to scarletweb.io")`, "the setup shows step 3");
      await clickSetup("Not now");
      await app.until(`!document.querySelector("#setup") && document.querySelector("#shell").hidden === false`, "the setup closed and the shell is back");
      const file = path.join(ctx.site.app.home, "settings.json");
      let saved;
      for (let i = 0; i < 150; i++) {
        saved = JSON.parse(read(file));
        if (saved.setup_done === true) break;
        await sleep(20);
      }
      expectEqual("setup_done in settings.json", saved.setup_done, true);
      await sleep(300);
      expectEqual("requests the website address received", ctx.website.seen, []);
    },
  },
  {
    lane: "app",
    name: "an export from inside the app refreshes it",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      const before = await headEvents(app);
      writeWow(ctx.site.app.wow, { later: true });
      await open(app, "data", /Export now/);
      await app.evaluate(`[...document.querySelectorAll("#body button")].find((b) => b.textContent === "Export now").click()`);
      await app.until(`/^Exported in/.test(document.querySelector("#body .status")?.textContent ?? "")`, "the export reports success", 30000);
      await app.until(`(async () => (await import("/bridge.js")).invoke("app_state"))().then((s) => s.sessions === 4)`, "app_state reports 4 sessions after Export now", 5000);
      const events = await app.until(`(() => { const n = Number(/([\\d,]+) events/.exec(document.querySelector("#head-stats").textContent)?.[1].replace(/,/g, "")); return n > ${before} ? n : 0; })()`, `the events in the header rise above ${before}`, 5000);
      expectEqual("events the header gained", events - before, LATER.lines.length);
      const data = path.join(ctx.site.app.wow, "_forever_", "Interface", "AddOns", "ScarletData");
      const host = ctx.website.url.replace(/^https?:\/\//, "").replace(/\./g, "\\.");
      expectMatch("Digest.lua says uploading is off and no character has a page", read(path.join(data, "Digest.lua")), new RegExp(`^  web = \\{ upload = false, linked = false, host = "${host}", sync = 0 \\},$`, "m"));
      expectNoMatch("Digest.lua gives no character a web page while uploading is off", read(path.join(data, "Digest.lua")), /page = /);
    },
  },
  {
    lane: "app",
    name: "the Data page says the game's store started over and how many recordings could not be read",
    async run(ctx) {
      const s = ctx.site.restart;
      fs.mkdirSync(s.home, { recursive: true });
      writeReset(s.wow, "before");
      expectEqual("exporting the first store", exportSite(ctx, s).code, 0);
      writeReset(s.wow, "after", { unreadable: true });
      expectEqual("exporting the store that started over", exportSite(ctx, s).code, 0);
      fs.writeFileSync(path.join(s.home, "settings.json"), JSON.stringify({ wow: s.wow, data: s.out, watch: false, last_view: "data", keep_addon: false, open_with_wow: false, upload: { enabled: false, server: ctx.website.url } }));
      const app = await launch({ exe: ctx.exe, home: s.home, reducedMotion: true });
      try {
        await booted(app);
        await open(app, "data", /Export now/);
        const notes = await app.until(`(() => { const n = [...document.querySelectorAll("#body .store-notes .status")].map((x) => x.textContent); return n.length === 3 ? n : null; })()`, "the Data page lists the store's notes");
        expectMatch("the note for the store", notes[0], new RegExp(`^${RESTARTED.source}$`));
        expectMatch("the note for the addon store", notes[1], new RegExp(`^${ADDONS_RESTARTED.source}$`));
        expectEqual("the note for the chunk that did not decode", notes[2], "1 recording in SavedVariables could not be read.");
        const failed = await app.evaluate(`[...document.querySelectorAll("#body .store-notes .status")].map((x) => x.className)`);
        expectEqual("which notes are errors", failed.map((c) => c.includes("err")), [false, false, true]);
        expectEqual("exceptions, console errors and failed requests", app.problems, []);
      } finally {
        if ((await app.close()) === 3) ctx.breach = true;
      }
    },
  },
  {
    lane: "app",
    name: "nothing was reported",
    run(ctx) {
      expectEqual("exceptions, console errors and failed requests", running(ctx).problems, []);
    },
  },
  {
    lane: "app",
    name: "talents of a character first read with every point spent are drawn from the baseline, with no pick date",
    async run(ctx) {
      const app = await fullApp(ctx);
      const [first, second] = fullBuild(ctx.now).sessions.map((s) => s.start * 1000 + 5000);
      await open(app, "character", /Points spent 9 \/ 9/, /No talent lines recorded|Select a character/, "character/talents");
      const body = await app.evaluate(BODY);
      expectMatch("the points head and the tabs", body, /Points spent 9 \/ 9 .* Unspent 0 .* Arms · 6 points .* Fury · 3 points/);
      expectMatch("the build's read", body, new RegExp(`Talents .* Read ${daysAgo(second)} days ago`));
      expectMatch("the timeline's one row", body, new RegExp(`Order points were taken Level 20 9 points already spent in 5 talents when Scarlet first read group 1 Read ${daysAgo(first)} days ago Every point was already spent when Scarlet first read this character`));
      expectNoMatch("respecs with none recorded", body, /Respecs/);
      expectNoMatch("a pick in the timeline", body, /Picked|Refunded|Switched choice|Changed while not tracked|point \d+ ·/);
      expectEqual("the ranks drawn", (await text(app, "#body .tf-tab .tf-slot .tf-rank")).sort(), ["1/1", "1/1", "2/2", "2/2", "3/3"]);
      expectEqual("every drawn talent is at its maximum", await app.evaluate(`[...document.querySelectorAll("#body .tf-tab .tf-slot")].map((s) => s.dataset.state)`), Array(5).fill("maxed"));
      const held = await app.evaluate(`(() => { const t = document.querySelector("#body .tp-pick.held time"); return { text: t.textContent, title: t.title, ago: Number(t.dataset.ago) }; })()`);
      expectEqual("the held row's read", held, { text: `Read ${daysAgo(first)} days ago`, title: exactStamp(first), ago: first });
      expectEqual("rows that carry a pick", await app.evaluate(`document.querySelectorAll("#body .tp-pick:not(.held)").length`), 0);
    },
  },
  {
    lane: "app",
    name: "Legacy of a character first read with every rank taken draws them from the baseline, with no rank history dated at the read",
    async run(ctx) {
      const app = await fullApp(ctx);
      await open(app, "legacy", /Earned 30 \/ 30/, /No Legacy lines recorded|Select a character/);
      const body = await app.evaluate(BODY);
      expectMatch("the points head", body, /Earned 30 \/ 30 .* Spent 30 To spend 0/);
      expectMatch("the trees' points", body, /Professions · 12 points .* Adventure · 8 points .* Progression · 10 points/);
      expectEqual("talents drawn at their maximum", await app.evaluate(`document.querySelectorAll("#body .tf-slot[data-state=maxed]").length`), 9);
      const lines = await text(app, "#body .leg-hist li");
      expectEqual("history lines", lines.length, 9);
      for (const line of lines) expectMatch("a history line", line, /^Rank \d Already held when Scarlet first read this character$/);
      expectNoMatch("a dated rank change", body, /\d→\d/);
      expectEqual("dates on the rank history", await app.evaluate(`document.querySelectorAll("#body .leg-hist time").length`), 0);
    },
  },
  {
    lane: "app",
    name: "a skill missing from a later baseline whose category was listed shows as unlearned, with its last rank and when it was last held",
    async run(ctx) {
      const app = await tradesApp(ctx);
      const held = trades(ctx.now).sessions[0].start * 1000 + 6000;
      await open(app, "character", /Unlearned/, /Select a character/);
      const groups = await skillGroups(app);
      expectEqual("professions drawn as held", groups.Professions.map((r) => r.split(" ")[0]).sort(), ["Herbalism", "Mining"]);
      expectEqual("weapons drawn as held", groups["Weapons and defense"].map((r) => r.split(" ")[0]), ["Swords"]);
      expectEqual("the unlearned group", groups.Unlearned, [`Alchemy 225 / 225 Last held ${daysAgo(held)} days ago`]);
      const row = await app.evaluate(`(() => { const t = document.querySelector("#body .sc-gone time"); return { title: t.title, ago: Number(t.dataset.ago), bars: document.querySelectorAll("#body .sc-gone .sg-t").length }; })()`);
      expectEqual("the last held moment", row, { title: exactStamp(held), ago: held, bars: 0 });
      const goals = await tradesSkills(app, `skills.map((s) => [s.name, s.unlearned && [s.unlearned.last_held_ms, s.unlearned.rank, s.unlearned.max]]).sort()`);
      expectEqual("the skills of the goals command", goals, [["Alchemy", [held, 225, 225]], ["Herbalism", null], ["Mining", null], ["Swords", null]]);
    },
  },
  {
    lane: "app",
    name: "a skill missing from a baseline that lacks its whole category stays learned, and unlearned professions leave the roster",
    async run(ctx) {
      const app = await tradesApp(ctx);
      const skills = await tradesSkills(app, `skills.filter((s) => !s.unlearned).map((s) => s.name).sort()`);
      expectEqual("skills still learned after a baseline with only Swords", skills, ["Herbalism", "Mining", "Swords"]);
      const roster = JSON.parse(await app.evaluate(`(async () => JSON.stringify((await (await import("/bridge.js")).invoke("roster")).chars.find((c) => c.name === "Trades-Fixture").professions.map((p) => p.name).sort()))()`));
      expectEqual("the roster's professions", roster, ["Herbalism", "Mining"]);
    },
  },
  {
    lane: "app",
    name: "every date reads month first with a 12-hour clock, and a shortened one keeps its exact value on hover",
    async run(ctx) {
      const app = running(ctx);
      await open(app, "characters", /Last session/);
      await selectCharacter(app, TESTA);
      expectMatch("the title bar's range", await app.evaluate(`document.querySelector("#head-stats").textContent`), /^[\d,]+ events · [A-Z][a-z]{2} \d{1,2}(?: \d{4})? to [A-Z][a-z]{2} \d{1,2}(?: \d{4})?$/);
      let seen = 0;
      let axes = 0;
      for (const [key, hash, shows] of [["characters", "characters", /Last session/], ["data", "data", /Addons recorded|Export now/], ["items", "items", /Warband and guild banks/], ["character", "character/talents", /Points spent|Order points were taken/], ["legacy", "legacy", /Earned/], ["sessions", "sessions", /Elwynn Forest/], ["log", "log", /Stonetusk Boar/], ["activity", "activity", /Records/], ["stats", "stats", /Experience over time/]]) {
        await open(app, key, shows, null, hash);
        expectNoMatch(`${hash}: a date in an old shape`, await app.evaluate(BODY), OLD_DATE_SHAPE);
        const stamps = await app.evaluate(`[...document.querySelectorAll("#body time[data-ago]")].map((t) => ({ ago: Number(t.dataset.ago), text: t.textContent, hover: t.title || t.dataset.tip || t.closest("[data-tip]")?.dataset.tip || "" }))`);
        for (const t of stamps) {
          expectMatch(`${hash}: a moment`, t.text, new RegExp(`^(?:[A-Za-z ]+ )?${MOMENT}$`));
          expectMatch(`${hash}: the exact value of ${t.text}`, t.hover, new RegExp(escapeRegExp(exactStamp(t.ago))));
        }
        const axis = await app.evaluate(`[...document.querySelectorAll("#body .chart-x span")].map((s) => s.textContent)`);
        for (const label of axis) expectMatch(`${hash}: an axis day`, label, /^(?:[A-Z][a-z]{2} \d{1,2}(?: \d{4})?|\d{1,2}:\d\d [AP]M)$/);
        axes += axis.length;
        seen += stamps.length;
      }
      if (axes < 2) throw mismatch("axis labels found across the pages", "at least 2", String(axes));
      if (seen < 8) throw mismatch("moments found across the pages", "at least 8", String(seen));
    },
  },
  {
    lane: "app",
    name: "the window closes cleanly",
    async run(ctx) {
      const app = running(ctx);
      const code = await app.close();
      ctx.app = null;
      if (code === 3) ctx.breach = true;
      if (code === null) throw new Error("the app was still running 10 s after the close button");
      expectEqual("exit code of the app", code, 0);
      expectEqual("entries of the app's default settings, profile, Documents and build folders, and the Run key", footprint(), ctx.footprint);
    },
  },
];

async function keepEvidence(ctx, check, number) {
  if (!ctx.app) return;
  const stem = path.join(ctx.dir, `fail-${number}`);
  try {
    await ctx.app.screenshot(`${stem}.png`);
    fs.writeFileSync(`${stem}.txt`, await ctx.app.evaluate(`document.body.innerText`));
    console.log(`  screenshot ${stem}.png\n  page text  ${stem}.txt`);
  } catch (error) {
    console.log(`  no screenshot: ${error.message}`);
  }
  if (ctx.app.exitCode() === 3) ctx.breach = true;
  ctx.keep = true;
}

async function runChecks(checks, ctx) {
  let failed = 0;
  const began = performance.now();
  for (const [index, check] of checks.entries()) {
    const at = performance.now();
    let error = null;
    try {
      await check.run(ctx);
    } catch (caught) {
      error = caught;
    }
    const ms = String(Math.round(performance.now() - at)).padStart(5);
    if (!error) {
      console.log(`ok   ${ms}ms  ${check.lane}  ${check.name}`);
      continue;
    }
    failed++;
    if (error.launcherExit === 3) ctx.breach = true;
    console.log(`FAIL ${ms}ms  ${check.lane}  ${check.name}`);
    console.log(String(error.message ?? error).replace(/^/gm, "  "));
    if (check.lane === "app") await keepEvidence(ctx, check, index);
  }
  return { failed, seconds: (performance.now() - began) / 1000 };
}

function build(crates) {
  const began = performance.now();
  for (const crate of crates) {
    const run = spawnSync("cargo", ["build", "-q"], { cwd: path.join(REPO, crate), encoding: "utf8", timeout: 900_000, windowsHide: true });
    if (run.status !== 0) {
      console.error(`build failed in ${crate}/\n${String(run.stderr).split("\n").slice(-25).join("\n")}`);
      process.exit(1);
    }
  }
  return (performance.now() - began) / 1000;
}

function options() {
  try {
    const { values } = parseArgs({
      args: process.argv.slice(2),
      options: { only: { type: "string" }, "no-build": { type: "boolean" }, exe: { type: "string" }, cli: { type: "string" }, keep: { type: "string" } },
      strict: true,
    });
    if (values.only && !["cli", "app"].includes(values.only)) throw new Error(`--only takes cli or app, not ${values.only}`);
    return values;
  } catch (error) {
    console.error(`${error.message}\n${USAGE}`);
    process.exit(2);
  }
}

function removeFolder(dir) {
  try {
    fs.rmSync(dir, { recursive: true, force: true, maxRetries: 50, retryDelay: 100 });
  } catch (error) {
    console.log(`files kept in ${dir}: ${error.message}`);
  }
}

async function main() {
  const given = options();
  let lanes = given.only ? [given.only] : ["cli", "app"];
  if (lanes.includes("app") && !WINDOWS) {
    console.log("skip app: the app lane needs Windows");
    lanes = lanes.filter((lane) => lane !== "app");
  }
  const checks = CHECKS.filter((check) => lanes.includes(check.lane));
  const needsApp = lanes.includes("app");
  const buildSeconds = given["no-build"] ? 0 : build(needsApp ? ["companion", "app"] : ["companion"]);

  const cli = path.resolve(given.cli ?? path.join(REPO, "companion", "target", "debug", WINDOWS ? "scarlet.exe" : "scarlet"));
  const exe = path.resolve(given.exe ?? path.join(REPO, "app", "target", "debug", "scarlet-app.exe"));
  for (const [what, file] of [["scarlet", cli], ...(needsApp ? [["scarlet-app.exe", exe]] : [])]) {
    if (!fs.existsSync(file)) {
      console.error(`${what} is not at ${file}: build it, or point --${what === "scarlet" ? "cli" : "exe"} at it\n${USAGE}`);
      process.exit(2);
    }
  }

  const dir = given.keep ? path.resolve(given.keep) : fs.mkdtempSync(path.join(os.tmpdir(), "scarlet-lane-"));
  fs.mkdirSync(dir, { recursive: true });
  const website = await listenWebsite();
  const ctx = { dir, cli, exe, website, keep: Boolean(given.keep), app: null, grind: null, full: null, trades: null, now: Date.now(), site: { cli: site(path.join(dir, "cli")), app: site(path.join(dir, "app")), motion: site(path.join(dir, "motion")), grind: site(path.join(dir, "grind")), full: site(path.join(dir, "full")), trades: site(path.join(dir, "trades")), reset: site(path.join(dir, "reset")), legacy: site(path.join(dir, "legacy")), restart: site(path.join(dir, "restart")) } };
  writeWow(ctx.site.cli.wow);

  let result = { failed: 1, seconds: 0 };
  try {
    result = await runChecks(checks, ctx);
  } finally {
    if ((await ctx.app?.close().catch(() => null)) === 3) ctx.breach = true;
    if ((await ctx.grind?.close().catch(() => null)) === 3) ctx.breach = true;
    if ((await ctx.full?.close().catch(() => null)) === 3) ctx.breach = true;
    if ((await ctx.trades?.close().catch(() => null)) === 3) ctx.breach = true;
    website.server.close();
    if (ctx.keep || result.failed) console.log(`files kept in ${dir}`);
    else removeFolder(dir);
  }
  console.log(`e2e: ${checks.length} checks, ${result.failed} failed, checks ${result.seconds.toFixed(1)}s, build ${buildSeconds.toFixed(1)}s`);
  if (ctx.breach) {
    console.log("THE LAUNCHER EXITED 3: a window of the app was found on the desktop in use");
    process.exit(3);
  }
  process.exit(result.failed ? 1 : 0);
}

await main();
