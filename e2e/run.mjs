// The checks to run before a ship: the real `scarlet` command and, on Windows, the real app on a desktop of its own.
//   node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]
// Exit 0 when every check passes, 1 when one fails, 2 on a usage error.
import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { parseArgs } from "node:util";
import { launch } from "./cdp.mjs";
import { CHARACTERS, LATER, lineCount, writeWow } from "./scenario.mjs";

const REPO = path.resolve(import.meta.dirname, "..");
const WINDOWS = process.platform === "win32";
const USAGE = "usage: node e2e/run.mjs [--only cli|app] [--no-build] [--exe <scarlet-app.exe>] [--cli <scarlet>] [--keep <dir>]";
const TESTA = CHARACTERS[0].name;
const PROBE = CHARACTERS[1].name;

const BODY = `document.querySelector("#body").innerText.replace(/\\s+/g, " ")`;

const VIEWS = [
  { key: "overview", shows: /Testa-Fixture Level 6 Human Warrior Alliance in Westfall/, never: /No characters recorded yet/ },
  { key: "stats", shows: /1,605 Experience .* 2 Kills/, never: /No recorded time yet/ },
  { key: "heatmap", shows: /Peak: [1-9]\d* events in an hour/, never: /Peak: 0 events|No events yet/ },
  { key: "timeline", shows: /kill Stonetusk Boar \(113\) by you/, never: /No events|Nothing recorded/ },
  { key: "sessions", shows: /Testa-Fixture 2m 5s Westfall 0 0 0c 0 .* Testa-Fixture 2m 10s Elwynn Forest 2 1605 \+3s43c 1/, never: /No sessions yet\./ },
  { key: "map", shows: /Elwynn Forest 16 points Westfall 10 points/, never: /No path points yet/ },
  { key: "charts", shows: /Experience over time 95 1,605 .* Top spells cast Heroic Strike 1 /, never: /Experience over time No data/ },
  { key: "loot", shows: /Drop rates .* Solo Stonetusk Boar .* Chunk of Boar Meat 100% 1\/1 .* Testa-Fixture Solo 3 /, never: /No creature loot with an item list|No loot recorded yet/ },
  { key: "talents", shows: /Arms · 3 points, 2 available .* Improved Heroic Strike 2\/3 Deflection 2\/2 .* Picked 1→2\/3/, never: /No talent lines recorded|Select a character/ },
  { key: "items", shows: /Linen Cloth 2589 3 Testa-Fixture 3 in bags/, never: /No bags or bank recorded yet|No items match/ },
  { key: "addons", shows: /GatherMate2 1 .* GatherMate2 Testa-Fixture \(2447\) map 1429 at 43\.9, 67\.0/, never: /No lines from other addons/ },
  { key: "compress", shows: /Export now .* 2 file\(s\) .* Payloads .* [1-9]\d* events loaded/, never: /Data folder .* · 0 events loaded/ },
  { key: "settings", shows: /WoW folders .* E2EONE .* E2ETWO/, never: /No game accounts yet/ },
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
  const run = spawnSync(ctx.cli, args, { encoding: "utf8", env });
  return { code: run.status, out: run.stdout ?? "", err: run.stderr ?? "" };
};

const exportSite = (ctx, site) => scarlet(ctx, "export", "--wow", site.wow, "--out", site.out);

const site = (root) => ({ wow: path.join(root, "wow"), out: path.join(root, "out"), home: path.join(root, "home") });

const read = (file) => fs.readFileSync(file, "utf8");

const hashOf = (dir) =>
  Object.fromEntries(
    fs
      .readdirSync(dir)
      .sort()
      .map((name) => [name, crypto.createHash("sha256").update(fs.readFileSync(path.join(dir, name))).digest("hex")]),
  );

const csvRows = (file) => read(file).trim().split(/\r?\n/).length - 1;

const gitStatus = () => spawnSync("git", ["status", "--porcelain"], { cwd: REPO, encoding: "utf8" }).stdout ?? "";

const selectCharacter = (app, name) =>
  app.evaluate(`(() => {
    const select = document.querySelector("#char");
    if (select.value === ${JSON.stringify(name)}) return;
    select.value = ${JSON.stringify(name)};
    select.dispatchEvent(new Event("change"));
  })()`);

const patternExpression = (pattern) => `new RegExp(${JSON.stringify(pattern.source)}, ${JSON.stringify(pattern.flags)})`;

async function open(app, key, shows) {
  await app.evaluate(`location.hash = ${JSON.stringify(key)}`);
  await app.until(
    `document.querySelector("#nav .item.active")?.dataset.key === ${JSON.stringify(key)} && ${patternExpression(shows)}.test(${BODY})`,
    `${key} shows ${shows}`,
  );
}

const running = (ctx) => {
  if (!ctx.app) throw new Error("the app is not running: the check that starts it failed");
  return ctx.app;
};

const appState = (app) => app.evaluate(`window.__TAURI__.core.invoke("app_state")`);

const headEvents = (app) => app.evaluate(`Number(/([\\d,]+) events/.exec(document.querySelector("#head-stats").textContent)?.[1].replace(/,/g, "") ?? NaN)`);

async function startApp(ctx) {
  const s = ctx.site.app;
  fs.mkdirSync(s.home, { recursive: true });
  writeWow(s.wow);
  const exported = exportSite(ctx, s);
  expectEqual("exporting the fixture for the app", exported.code, 0);
  fs.writeFileSync(
    path.join(s.home, "settings.json"),
    JSON.stringify({ wow: s.wow, data: s.out, watch: false, last_view: "overview", keep_addon: false, open_with_wow: false }),
  );
  ctx.repoBefore = gitStatus();
  ctx.app = await launch({ exe: ctx.exe, home: s.home });
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
      expectMatch("a known line in Probe-Lane/scarlet.txt", read(path.join(s.out, PROBE, "scarlet.txt")), /kill\s+Kobold Vermin \(6\) by you/);
    },
  },
  {
    lane: "cli",
    name: "the addon files are written",
    run(ctx) {
      const dir = path.join(ctx.site.cli.wow, "_forever_", "Interface", "AddOns", "ScarletData");
      for (const file of ["Digest.lua", "Items.lua", "ScarletData.toc"]) {
        if (!fs.existsSync(path.join(dir, file))) throw mismatch("a file the export writes for the addon", file, "missing");
      }
      const digest = read(path.join(dir, "Digest.lua"));
      for (const c of CHARACTERS) expectMatch(`Digest.lua names ${c.name}`, digest, new RegExp(`\\["${c.name}"\\]`));
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
      expectEqual("items in the nav", page.items, VIEWS.length);
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
      expectMatch("the foot of the nav", foot, new RegExp(`${CHARACTERS.length} character\\(s\\)`));
    },
  },
  ...VIEWS.map(({ key, shows, never }) => ({
    lane: "app",
    name: `${key} shows the selected character`,
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, TESTA);
      await open(app, key, shows);
      expectNoMatch(`${key} shows its empty state`, await app.evaluate(BODY), never);
    },
  })),
  {
    lane: "app",
    name: "the character filter works",
    async run(ctx) {
      const app = running(ctx);
      await selectCharacter(app, "");
      await open(app, "overview", new RegExp(`^(?=.*Level target)(?=.*${TESTA})(?=.*${PROBE})`));
      await selectCharacter(app, PROBE);
      await open(app, "overview", /Probe-Lane Level 3 Gnome Mage/);
      expectNoMatch("the overview of one character", await app.evaluate(BODY), /Level target/);
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
      await open(app, "overview", new RegExp(`^(?=.*game account E2EONE)(?=.*game account E2ETWO)(?=.*${TESTA})(?=.*${PROBE})`, "i"));
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
      await open(app, "compress", /Export now/);
      await app.evaluate(`[...document.querySelectorAll("#body button")].find((b) => b.textContent === "Export now").click()`);
      await app.until(`window.__TAURI__.core.invoke("app_state").then((s) => s.sessions === 4)`, "app_state reports 4 sessions after Export now", 5000);
      await app.until(`/^export ok/.test(document.querySelector("#body .status")?.textContent ?? "")`, "the export reports success", 5000);
      const events = await app.until(`(() => { const n = Number(/([\\d,]+) events/.exec(document.querySelector("#head-stats").textContent)?.[1].replace(/,/g, "")); return n > ${before} ? n : 0; })()`, `the events in the header rise above ${before}`, 5000);
      expectEqual("events the header gained", events - before, LATER.lines.length);
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
      if (code === null) throw new Error("the app was still running 3 s after the close button");
      expectEqual("exit code of the app", code, 0);
      expectEqual("files changed in the repository", gitStatus(), ctx.repoBefore);
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
  if (ctx.app.exitCode() === 3) console.log("  THE LAUNCHER EXITED 3: a window of the app was found on the desktop in use");
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
    console.log(`FAIL ${ms}ms  ${check.lane}  ${check.name}`);
    console.log(String(error.message ?? error).replace(/^/gm, "  "));
    if (check.lane === "app") await keepEvidence(ctx, check, index);
  }
  return { failed, seconds: (performance.now() - began) / 1000 };
}

function build(crates) {
  const began = performance.now();
  for (const crate of crates) {
    const run = spawnSync("cargo", ["build", "-q"], { cwd: path.join(REPO, crate), encoding: "utf8" });
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

  const dir = given.keep ? path.resolve(given.keep) : fs.mkdtempSync(path.join(os.tmpdir(), "scarlet-e2e-"));
  fs.mkdirSync(dir, { recursive: true });
  const ctx = { dir, cli, exe, keep: Boolean(given.keep), app: null, site: { cli: site(path.join(dir, "cli")), app: site(path.join(dir, "app")) } };
  writeWow(ctx.site.cli.wow);

  let result = { failed: 1, seconds: 0 };
  try {
    result = await runChecks(checks, ctx);
  } finally {
    await ctx.app?.close().catch(() => {});
    if (!ctx.keep && !result.failed) fs.rmSync(dir, { recursive: true, force: true });
    else console.log(`files kept in ${dir}`);
  }
  console.log(`e2e: ${checks.length} checks, ${result.failed} failed, checks ${result.seconds.toFixed(1)}s, build ${buildSeconds.toFixed(1)}s`);
  process.exit(result.failed ? 1 : 0);
}

await main();
