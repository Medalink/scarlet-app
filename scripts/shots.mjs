// A full-page screenshot of each view, from the real exe on a hidden desktop, for comparing with the design's renders.
//   node scripts/shots.mjs --out <dir> [--fixture | --grind | --home <dir>] [--views a,b,c] [--char Name-Surname] [--mode auto|day|manual] [--run <from>-<to>] [--delay <ms>] [--width <css px>] [--upload]
// --click <selector> opens the first element it matches after each view is shot, and shoots the page that opens, printing its address and its first words.
// --clip <selector> [--nth n] cuts each shot to the nth element the selector matches, with 12 px around it.
// --delay takes the run view that many ms after its entrance began, for a frame in the middle of it; every other shot
// is taken with reduced motion, so the numbers and bars are final. --width lays the page out that many CSS px wide. --fit shoots the window as it is, without growing the page to its content.
// --fixture builds the e2e fixture and exports it; --grind does the same for the fixture of one character with six long runs; --home copies a Scarlet home (settings.json and its data folder) to
// a temp folder under E:\Temp first, so the original is never written. Uploading is switched off unless --upload.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { launch } from "../e2e/cdp.mjs";
import { writeGrind, writeWow } from "../e2e/scenario.mjs";

const REPO = path.resolve(import.meta.dirname, "..");
const { values } = parseArgs({
  options: {
    out: { type: "string" },
    fixture: { type: "boolean" },
    grind: { type: "boolean" },
    home: { type: "string" },
    views: { type: "string" },
    char: { type: "string" },
    upload: { type: "boolean" },
    run: { type: "string" },
    exe: { type: "string" },
    mode: { type: "string" },
    delay: { type: "string" },
    width: { type: "string" },
    clip: { type: "string" },
    click: { type: "string" },
    fit: { type: "boolean" },
    nth: { type: "string" },
  },
});
if (!values.out || (!values.fixture && !values.grind && !values.home)) {
  console.error("usage: node scripts/shots.mjs --out <dir> (--fixture | --home <dir>) [--views a,b] [--char Name-Surname] [--upload]");
  process.exit(2);
}

const temp = fs.mkdtempSync(path.join(process.env.SCARLET_SHOTS_TEMP ?? "E:\\Temp", "scarlet-shots-"));
const home = path.join(temp, "home");
fs.mkdirSync(home, { recursive: true });
fs.mkdirSync(values.out, { recursive: true });

let settings;
if (values.fixture || values.grind) {
  const wow = path.join(temp, "wow");
  const out = path.join(temp, "out");
  (values.grind ? writeGrind : writeWow)(wow);
  const cli = path.join(REPO, "companion", "target", "debug", "scarlet.exe");
  const run = spawnSync(cli, ["export", "--wow", wow, "--out", out], { encoding: "utf8", windowsHide: true });
  if (run.status !== 0) throw new Error(`export failed: ${run.stderr}`);
  settings = { wow, data: out, watch: false, last_view: "sessions", keep_addon: false, open_with_wow: false, setup_done: true, upload: { enabled: Boolean(values.upload), server: "http://127.0.0.1:8471" }, session_mode: values.mode ?? "auto", session_marks: values.mode === "manual" ? [{ from: 1795000040000, to: 1795000100000 }] : [] };
} else {
  const source = JSON.parse(fs.readFileSync(path.join(values.home, "settings.json"), "utf8"));
  const copy = path.join(temp, "out");
  fs.cpSync(source.data, copy, { recursive: true });
  settings = { wow: "", more_wow: [], last_view: "sessions", ...source, data: copy, watch: false, keep_addon: false, open_with_wow: false, setup_done: true, session_mode: values.mode ?? source.session_mode, upload: { ...source.upload, enabled: Boolean(values.upload) } };
}
fs.writeFileSync(path.join(home, "settings.json"), JSON.stringify(settings));

const exe = values.exe ?? path.join(REPO, "app", "target", "debug", "scarlet-app.exe");
const app = await launch({ exe, home, reducedMotion: values.delay === undefined });
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let code = 0;
try {
  await app.until(`document.querySelectorAll("#nav .item").length > 0 && document.querySelector("#pagetitle").textContent !== ""`, "the page booted", 20000);
  const rows = await app.evaluate(`(async () => (await import("/bridge.js")).invoke("sessions", { ch: null }))()`);
  const last = rows[rows.length - 1];
  const run = values.run ? `sessions/${values.run}` : last ? `sessions/${last.start}-${last.end}` : "sessions";
  const names = (values.views ?? "system,sessions,run,feats,character,character/class,character/talents,stats,map,loot,characters,items,activity,log,data,settings").split(",");
  if (values.char) await app.evaluate(`(() => { const s = document.querySelector("#char"); s.value = ${JSON.stringify(values.char)}; s.dispatchEvent(new Event("change")); })()`);
  if (values.width) await app.viewport(Number(values.width), 900);
  if (!values.fit) await app.evaluate(`(() => { const s = document.createElement("style"); s.textContent = "html,body,#shell,main,#body{height:auto!important;overflow:visible!important}body{display:block!important}#shell{display:flex;min-height:calc(100vh - 54px)}#nav{min-height:calc(100vh - 54px)}"; document.head.append(s); })()`);
  for (const name of names) {
    const hash = name === "run" ? run : name;
    const asked = Date.now();
    await app.evaluate(`location.hash = ${JSON.stringify(hash)}`);
    await app.until(
      `(() => { const b = document.querySelector("#body"); return !!b && b.dataset.ready === "1" && b.dataset.route === location.hash.slice(1); })()`,
      `${name} drew`,
      60000,
    );
    const drawnMs = Date.now() - asked;
    const timed = values.delay !== undefined && name === "run";
    if (timed) {
      await app.until(`!!document.querySelector("#body .recap.play")`, "the page's entrance began", 10000);
      await sleep(Number(values.delay));
    } else {
      await app.until(`!document.querySelector("#body .play")`, `${name} settled`, 6000).catch(() => {});
      await sleep(values.upload ? 5000 : 300);
    }
    const file = path.join(values.out, `${name.replace(/\//g, "-")}${timed ? `-at-${values.delay}ms` : ""}.png`);
    const rect = values.clip ? await app.evaluate(`(() => { const e = [...document.querySelectorAll(${JSON.stringify(values.clip)})].at(${Number(values.nth ?? 0)}); if (!e) return null; const r = e.getBoundingClientRect(); return { x: Math.max(0, r.left + scrollX - 12), y: Math.max(0, r.top + scrollY - 12), width: r.width + 24, height: r.height + 24 }; })()`) : null;
    await app.screenshot(file, rect ? { clip: rect } : values.fit ? {} : { full: true });
    console.log(`${name}: ${file} (drawn in ${drawnMs} ms)`);
    if (values.click) {
      const opened = await app.evaluate(`(() => { const e = document.querySelector(${JSON.stringify(values.click)}); if (!e) return false; e.click(); return true; })()`);
      if (opened) {
        await app.until(`location.hash.slice(1) !== ${JSON.stringify(hash)} && document.querySelector("#body")?.dataset.ready === "1" && document.querySelector("#body").dataset.route === location.hash.slice(1)`, `${name} opened a page`, 60000);
        await app.until(`!document.querySelector("#body .play")`, `${name}'s opened page settled`, 6000).catch(() => {});
        await sleep(300);
        const seen = await app.evaluate(`({ hash: location.hash, text: document.querySelector("#body").innerText.replace(/\\s+/g, " ").slice(0, 240) })`);
        const shot = path.join(values.out, `${name.replace(/\//g, "-")}-opened.png`);
        await app.screenshot(shot, { full: true });
        console.log(`${name} opened ${seen.hash}: ${seen.text}
  ${shot}`);
      } else console.log(`${name}: nothing matches ${values.click}`);
    }
  }
  if (app.problems.length) {
    console.log(`problems: ${app.problems.join(" | ")}`);
    code = 1;
  }
} catch (e) {
  if (app.problems.length) console.log(`problems: ${app.problems.join(" | ")}`);
  throw e;
} finally {
  const exit = await app.close();
  if (exit === 3) code = 3;
  fs.rmSync(temp, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
}
process.exit(code);
