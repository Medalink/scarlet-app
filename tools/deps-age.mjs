// Fails when a crate locked in app/Cargo.lock or companion/Cargo.lock was published less than
// 7 days ago (the repo's dependency cooldown). Reads crates.io's `created_at` for each version.
//
//   node tools/deps-age.mjs              check both lockfiles; exit 1 when a locked crate is too young
//   node tools/deps-age.mjs --report     also list locked crates that have a newer version on the same
//                                        line (1.x, or 0.x) that is old enough
//   node tools/deps-age.mjs --days 10    another cooldown
//
// Only crates.io crates are checked: path and git dependencies have no publish date here.
// Publish dates never change, so each crate's version list is cached in DEPS_AGE_CACHE (default:
// <temp dir>/deps-age) and fetched again only when the locked version is not in the cache. The
// requests go one per second.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const report = args.includes("--report");
const days = Number(args[args.indexOf("--days") + 1] || 7) || 7;
const cutoff = Date.now() - days * 86400e3;
const cacheDir = process.env.DEPS_AGE_CACHE || path.join(os.tmpdir(), "deps-age");
fs.mkdirSync(cacheDir, { recursive: true });

/** name -> Set of locked versions, from every lockfile's registry packages. */
function locked() {
  const out = new Map();
  for (const dir of ["app", "companion"]) {
    const text = fs.readFileSync(path.join(root, dir, "Cargo.lock"), "utf8");
    for (const block of text.split("[[package]]").slice(1)) {
      const name = /^name = "(.+)"$/m.exec(block)?.[1];
      const version = /^version = "(.+)"$/m.exec(block)?.[1];
      const source = /^source = "(.+)"$/m.exec(block)?.[1];
      if (!name || !version || !source?.startsWith("registry+")) continue;
      if (!out.has(name)) out.set(name, new Map());
      out.get(name).set(version, [...(out.get(name).get(version) || []), dir]);
    }
  }
  return out;
}

let lastRequest = 0;
async function fetchVersions(name) {
  const wait = lastRequest + 1000 - Date.now();
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  lastRequest = Date.now();
  const res = await fetch(`https://crates.io/api/v1/crates/${name}`, {
    headers: { "User-Agent": "deps-age-check" },
  });
  if (!res.ok) throw new Error(`crates.io answered ${res.status} for ${name}`);
  const data = await res.json();
  return data.versions.map((v) => ({ num: v.num, at: v.created_at, yanked: v.yanked }));
}

async function versionsOf(name, need) {
  const file = path.join(cacheDir, `${name}.json`);
  let cached = null;
  try {
    cached = JSON.parse(fs.readFileSync(file, "utf8"));
  } catch {}
  // A report wants the newest versions, so it refreshes a cache older than a day.
  const fresh = cached && Date.now() - fs.statSync(file).mtimeMs < 86400e3;
  const complete = cached && [...need].every((v) => cached.some((c) => c.num === v));
  if (complete && (!report || fresh)) return cached;
  const got = await fetchVersions(name);
  fs.writeFileSync(file, JSON.stringify(got));
  return got;
}

/** Cargo treats 1.2.3 and 1.9.0 (or 0.4.1 and 0.4.9) as the same line; only those can replace each other. */
const line = (v) => {
  const [a, b] = v.split(".");
  return a === "0" ? `0.${b}` : a;
};

const young = [];
const behind = [];
const unknown = [];
for (const [name, byVersion] of locked()) {
  let versions;
  try {
    versions = await versionsOf(name, byVersion.keys());
  } catch (e) {
    unknown.push(`${name}: ${e.message}`);
    continue;
  }
  const eligible = versions
    .filter((v) => !v.yanked && Date.parse(v.at) <= cutoff && !v.num.split("+")[0].includes("-"))
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at));
  for (const [version, dirs] of byVersion) {
    const v = versions.find((x) => x.num === version);
    if (!v) {
      unknown.push(`${name}@${version}: not on crates.io`);
    } else if (Date.parse(v.at) > cutoff) {
      const older = eligible.find((x) => line(x.num) === line(version) && Date.parse(x.at) < Date.parse(v.at));
      young.push(
        `${name}@${version} published ${v.at.slice(0, 10)} (${dirs.join(", ")}); newest old enough: ${older ? older.num : "none on this line"}`,
      );
    }
  }
  if (report) {
    for (const version of byVersion.keys()) {
      const newest = eligible.find((v) => line(v.num) === line(version));
      const have = versions.find((x) => x.num === version);
      if (newest && have && Date.parse(newest.at) > Date.parse(have.at)) {
        behind.push(`${name} ${version} -> ${newest.num} (${newest.at.slice(0, 10)})`);
      }
    }
  }
}

if (report && behind.length) console.log(`Newer versions at least ${days} days old:\n  ${behind.join("\n  ")}`);
if (unknown.length) console.error(`Could not check:\n  ${unknown.join("\n  ")}`);
if (young.length) {
  console.error(`Locked crates published less than ${days} days ago:\n  ${young.join("\n  ")}`);
  console.error("Move each down: cargo update -p <name>@<version> --precise <older version>");
}
if (young.length || unknown.length) process.exit(1);
console.log(`deps-age: every locked crate is at least ${days} days old.`);
