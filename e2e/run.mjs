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
import { CHARACTERS, ELWYNN, FISHING, GUILD, LATER, WESTFALL, lineCount, savedVariablesPath, writeWow } from "./scenario.mjs";

const REPO = path.resolve(import.meta.dirname, "..");
const WINDOWS = process.platform === "win32";
const USAGE = "usage: node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]";
const TESTA = CHARACTERS[0].name;
const PROBE = CHARACTERS[1].name;
const SHOWN = { testa: TESTA.replace("-", " "), probe: PROBE.replace("-", " ") };
const OTHER_CHARACTER = new RegExp(`${SHOWN.probe}|Kobold Vermin`);
const OTHER_CHARACTER_ONLY = new Set(["character", "sessions", "log", "map", "loot", "data"]);

const BODY = `document.querySelector("#body").innerText.replace(/\\s+/g, " ")`;

const ACCOUNT_ROW = /(?=.*Account \d+ of \d+)(?!.*Account holds)(?=.*Scarlet.*Legendary.*Epic.*Rare.*Uncommon)/.source;
const FEATS_LOCAL = new RegExp(String.raw`^${ACCOUNT_ROW}(?=.*Testa Fixture holds [1-9]\d* feats?)(?=.*Closest to the next tier)(?=.*Every feat Testa can hold)(?=.*\d+ more feats are counted on the website Website settings)`);
const FEATS_LOCAL_ALL = new RegExp(String.raw`^${ACCOUNT_ROW}(?=.*Probe Lane holds [1-9]\d* feats?)(?=.*Testa Fixture holds [1-9]\d* feats?)(?=.*\d+ more feats are counted on the website)`);

const VIEWS = [
  {
    key: "character",
    shows: /^(?=.*Testa Fixture Level 6 Human Warrior Alliance in Westfall)(?=.*Whole life Quests completed 1,234 .* Titles known 3 .* Zone maps explored 3 .* Instance locks Blackfathom Deeps normal 1\/4 .* Lord Kazzak World boss)(?!.*Gnomeregan)/,
    never: /No characters recorded yet/,
  },
  { key: "characters", shows: /^(?=.*Level target)(?=.*Testa Fixture)(?=.*Probe Lane)/, never: /No characters recorded yet/ },
  { key: "feats", shows: FEATS_LOCAL, never: /No feats counted|Could not|refused|Copper|Silver|Legend\b|Poor/ },
  { key: "stats", shows: /^(?=.*1,605 Experience .* 2 Kills)(?=.*Travel and time)(?=.*Experience over time)(?=.*Top spells cast Heroic Strike 1)(?=.*Moving and standing)(?=.*Zones by time)/, never: /No recorded time yet/ },
  { key: "activity", shows: /Peak: [1-9]\d* events in an hour/, never: /Peak: 0 events|No events yet/ },
  { key: "log", shows: /kill Stonetusk Boar \(113\) by you/, never: /No events|Nothing recorded/ },
  { key: "sessions", shows: /^(?=.*2m in Westfall)(?=.*Level 5 to 6 in Elwynn Forest)(?=.*2m 10s · 2 kills · 1 quest · 1,605 XP)/, never: /No sessions yet\./ },
  { key: "map", shows: /Elwynn Forest 12 points Westfall 10 points/, never: /No path points yet/ },
  { key: "loot", shows: /^(?=.*Drop rates .* Stonetusk Boar 1 corpse Linen Cloth 100% 1\/1 Chunk of Boar Meat 100% 1\/1)(?=.*Items received and rolls Common 3 Uncommon 1 Rare 1 Won 1 Fixture Stranger Solo 2 Testa Fixture Solo 3 1)/, never: /No creature loot with an item list|No loot recorded yet/ },
  { key: "character", hash: "character/class", shows: /Spells cast most Heroic Strike 1/, never: /Select a character|Loading the class section/ },
  { key: "character", hash: "character/talents", shows: /Arms · 3 points, 2 available .* Improved Heroic Strike 2\/3 Deflection 2\/2 .* Picked 1→2\/3/, never: /No talent lines recorded|Select a character/ },
  { key: "legacy", shows: /Progression Gourmand · rank 1 of 3 .* Well Fed duration .* 0 1 15\.0 min — — .* 1 1 20\.0 min \+33% 33% /, never: /No Legacy lines recorded|Select a character/ },
  {
    key: "items",
    shows: /^(?=.*Linen Cloth 2589 Testa Fixture 3 Warband bank \(E2EONE\) 20 Warband bank \(E2ETWO\) 5 28 1s ?50c 42s ?00c Wool Cloth 2592 Guild bank 7 7 – –)(?=.*Recipes 2× Linen Cloth 1× Bolt of Linen Cloth Testa Fixture)(?=.*Warband and guild banks Warband bank \(E2EONE\) 1 ?item Last read \d{4}-\d\d-\d\d \d\d:\d\d by Testa Fixture Balance 12g ?34s Warband bank \(E2ETWO\) 1 ?item Last read .* by Probe Lane Balance 5g ?00s Guild bank 1 ?item Last read .* by Probe Lane Balance 98g ?76s)(?!.*1s ?25c)/,
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
    const server = http.createServer((request, answer) => {
      seen.push(`${request.method} ${request.url}`);
      answer.statusCode = 404;
      answer.end();
    });
    server.listen(0, "127.0.0.1", () => resolve({ server, seen, url: `http://127.0.0.1:${server.address().port}` }));
  });

const read = (file) => fs.readFileSync(file, "utf8");

const hashOf = (dir) =>
  Object.fromEntries(
    fs
      .readdirSync(dir)
      .sort()
      .map((name) => [name, crypto.createHash("sha256").update(fs.readFileSync(path.join(dir, name))).digest("hex")]),
  );

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
    `document.querySelector("#nav .item.active")?.dataset.key === ${JSON.stringify(key)} && ${patternExpression(shows)}.test(${BODY})${without}`,
    `${hash} shows ${shows}${gone ? ` and no longer ${gone}` : ""}`,
  );
}

const running = (ctx) => {
  if (!ctx.app) throw new Error("the app is not running: the check that starts it failed");
  return ctx.app;
};

const appState = (app) => app.evaluate(`(async () => (await import("/bridge.js")).invoke("app_state"))()`);

const headEvents = (app) => app.evaluate(`Number(/([\\d,]+) events/.exec(document.querySelector("#head-stats").textContent)?.[1].replace(/,/g, "") ?? NaN)`);

async function startApp(ctx) {
  const s = ctx.site.app;
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
  ctx.footprint = footprint();
  ctx.app = await launch({ exe: ctx.exe, home: s.home, reducedMotion: true });
  await ctx.app.until(`document.querySelectorAll("#nav .item").length > 0 && document.querySelector("#pagetitle").textContent !== ""`, "the page booted", 10000);
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
      const shows = /^(?=.*Level 5 to 6 in Elwynn Forest)(?=.*Testa Fixture)(?=.*Experience 1,605)(?=.*Kills 2)(?=.*Experience every 5 minutes)(?=.*Killed most)(?=.*Levels, deaths and feats)(?!.*Probe Lane)/;
      await app.evaluate(`location.hash = "sessions/1795000000000-1795000130000"`);
      await app.until(`${patternExpression(shows)}.test(${BODY})`, `the run summary shows ${shows}`);
    },
  },
  {
    lane: "app",
    name: "the run summary carries one titled recap for each character",
    async run(ctx) {
      const app = running(ctx);
      const summary = await app.evaluate(`(async () => (await import("/bridge.js")).invoke("run_summary", { from: 1795000000000, to: 1795000130000 }))()`);
      expectEqual("the summary fields", Object.keys(summary).sort(), ["chars", "from", "played", "recaps", "sessions", "to"]);
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
      await app.until(`/day, .* · 1 session · .* played/.test(${BODY})`, "a day's page reads as a day");
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
      await app.until(`/1m 0s played/.test(${BODY}) && /Experience 205 /.test(${BODY}) && !/1,605/.test(${BODY})`, "a marked session's page holds only the minute inside it");
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
    name: "the character filter works",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "characters", new RegExp(`^(?=.*Level target)(?=.*${SHOWN.testa})(?=.*${SHOWN.probe})`));
      await selectCharacter(app, PROBE);
      await open(app, "character", /Probe Lane Level 3 Gnome Mage/, new RegExp(`Level target|Whole life|${SHOWN.testa}`));
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
          /Linen Cloth 2589 Testa Fixture 3 Warband bank \(account unknown, Probe Lane\) 5 Warband bank \(account unknown, Testa Fixture\) 20 28 1s ?50c .* Guild bank 1 ?item Last read .* by Probe Lane/,
          /Warband bank \(E2E|Guild bank \(/,
        );
      } finally {
        writeWow(wow);
      }
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
    name: "nothing was reported",
    run(ctx) {
      expectEqual("exceptions, console errors and failed requests", running(ctx).problems, []);
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
  const ctx = { dir, cli, exe, website, keep: Boolean(given.keep), app: null, site: { cli: site(path.join(dir, "cli")), app: site(path.join(dir, "app")) } };
  writeWow(ctx.site.cli.wow);

  let result = { failed: 1, seconds: 0 };
  try {
    result = await runChecks(checks, ctx);
  } finally {
    if ((await ctx.app?.close().catch(() => null)) === 3) ctx.breach = true;
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
