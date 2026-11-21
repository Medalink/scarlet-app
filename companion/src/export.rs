//! The three commands: export, watch, logs. Plus SavedVariables discovery
//! and state.json bookkeeping.

use crate::addons;
use crate::archive;
use crate::events;
use crate::lua::{self, Lua};
use crate::overview::{self, Overview};
use crate::tables;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Every `<game>\WTF\Account\*\SavedVariables\Scarlet.lua` of every WoW folder (one per game
/// account, so a player who dual boxes has two or more), with `<game>` from
/// `gamedir::game_dir(wow)`. A folder listed twice counts once.
pub fn find_sv_all(wows: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    for wow in wows {
        let accounts = crate::gamedir::game_dir(wow).join("WTF").join("Account");
        if let Ok(entries) = fs::read_dir(&accounts) {
            for acc in entries {
                let sv = acc?.path().join("SavedVariables").join("Scarlet.lua");
                if sv.is_file() {
                    found.push(sv);
                }
            }
        }
    }
    found.sort_by_cached_key(|p| state_key(p));
    found.dedup_by_key(|p| state_key(p));
    Ok(found)
}

/// `find_sv_all` for one WoW folder.
pub fn find_sv(wow: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    find_sv_all(&[wow.to_path_buf()])
}

/// `has_new_all` for one WoW folder.
pub fn has_new(wow: &Path, out: &Path) -> bool {
    has_new_all(&[wow.to_path_buf()], out)
}

/// `export_new_all` for one WoW folder.
pub fn export_new(wow: &Path, out: &Path) -> Result<bool, Box<dyn Error>> {
    export_new_all(&[wow.to_path_buf()], out)
}

/// `logs_all` for one WoW folder.
pub fn logs(wow: &Path, out: &Path) -> Result<(), Box<dyn Error>> {
    logs_all(&[wow.to_path_buf()], out)
}

#[derive(Serialize, Deserialize, Default)]
struct State(BTreeMap<String, u64>);

fn load_state(path: &Path) -> State {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(path: &Path, state: &State) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, serde_json::to_string_pretty(state)? + "\n")?;
    Ok(())
}

/// state.json keys are canonical SavedVariables paths.
fn state_key(p: &Path) -> String {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    let s = c.to_string_lossy().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

pub(crate) struct Chunk {
    pub(crate) q: u64,
    pub(crate) c: String,
    pub(crate) t: i64,
    pub(crate) e: String,
    pub(crate) z: String,
}

pub(crate) fn chunk(l: &Lua) -> Option<Chunk> {
    Some(Chunk {
        q: l.get("q")?.as_int()? as u64,
        c: l.get("c")?.as_str()?.to_string(),
        t: l.get("t")?.as_int()?,
        e: l.get("e")?.as_str()?.to_string(),
        z: l.get("z")?.as_str()?.to_string(),
    })
}

fn inflate(raw: &[u8]) -> Option<String> {
    let mut out = Vec::new();
    if DeflateDecoder::new(raw).read_to_end(&mut out).is_ok()
        && let Ok(s) = String::from_utf8(out)
    {
        return Some(s);
    }
    let mut out = Vec::new();
    ZlibDecoder::new(raw).read_to_end(&mut out).ok()?;
    String::from_utf8(out).ok()
}

pub(crate) fn decode(e: &str, z: &str) -> Option<String> {
    use base64::Engine;
    match e {
        "txt" => Some(z.to_string()),
        "d64" => {
            let raw = base64::engine::general_purpose::STANDARD.decode(z).ok()?;
            inflate(&raw)
        }
        _ => None,
    }
}

struct CharRun {
    txt: String,
    scl: String,
    names: HashMap<String, String>,
    chunks: u64,
    events: u64,
    first_q: u64,
    last_q: u64,
}

impl CharRun {
    fn new(q: u64) -> Self {
        Self {
            txt: String::new(),
            scl: String::new(),
            names: HashMap::new(),
            chunks: 0,
            events: 0,
            first_q: q,
            last_q: q,
        }
    }
}

/// Export all new chunks from every account's Scarlet.lua. Writes the
/// readable .txt and one .scl payload per character, then state.json.
pub fn export_all(wows: &[PathBuf], out: &Path) -> Result<(), Box<dyn Error>> {
    export_new_all(wows, out).map(|_| ())
}

/// True when a SavedVariables file was written after the last export finished
/// (`state.json` is rewritten at the end of every export). With no `state.json`
/// yet, any SavedVariables file counts.
pub fn has_new_all(wows: &[PathBuf], out: &Path) -> bool {
    let done = fs::metadata(out.join("state.json"))
        .and_then(|m| m.modified())
        .ok();
    find_sv_all(wows).unwrap_or_default().iter().any(|sv| {
        let t = fs::metadata(sv).and_then(|m| m.modified()).ok();
        match (t, done) {
            (Some(t), Some(done)) => t > done,
            (Some(_), None) => true,
            _ => false,
        }
    })
}

/// Every SavedVariables file with its modification time. Two clients write their own file at
/// their own moments, so a caller takes this before an export and compares it after: a file
/// written during the export shows as different and is exported again.
pub fn sv_stamps(wows: &[PathBuf]) -> Vec<(PathBuf, Option<SystemTime>)> {
    find_sv_all(wows)
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let t = fs::metadata(&p).and_then(|m| m.modified()).ok();
            (p, t)
        })
        .collect()
}

/// One export at a time: the watch thread and a button press, or two clients logging out
/// together, must not read the same chunks twice and write two payloads for them.
static EXPORT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `export`, reporting whether it wrote any new lines. When it did not, the caller
/// has nothing to reload.
pub fn export_new_all(wows: &[PathBuf], out: &Path) -> Result<bool, Box<dyn Error>> {
    let _one = EXPORT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let files = find_sv_all(wows)?;
    let state_path = out.join("state.json");
    let mut state = load_state(&state_path);
    let mut runs: BTreeMap<String, CharRun> = BTreeMap::new();
    let mut bad: u64 = 0;
    let mut addon: BTreeMap<String, Vec<AddonRow>> = BTreeMap::new();
    let mut addon_runs: BTreeMap<String, addons::AddonRun> = BTreeMap::new();

    for sv in &files {
        let key = state_key(sv);
        let last = state.0.get(&key).copied().unwrap_or(0);
        let text = match fs::read_to_string(sv) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("scarlet: skipping {}: {e}", sv.display());
                continue;
            }
        };
        // The addon stream has its own table, chunk numbers and state key.
        if text.contains("ScarletAddonsDB") {
            match lua::parse_named(&text, "ScarletAddonsDB") {
                Ok(Some(adb)) => {
                    let akey = format!("{key}#addons");
                    let alast = state.0.get(&akey).copied().unwrap_or(0);
                    let newest = addons::collect(&adb, alast, &mut addon_runs, &mut bad);
                    if newest > alast {
                        state.0.insert(akey, newest);
                    }
                }
                Ok(None) => {}
                Err(e) => eprintln!("scarlet: {}: addons: {e}", sv.display()),
            }
        }
        let db = match lua::parse_main(&text) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("scarlet: skipping {}: {e}", sv.display());
                continue;
            }
        };
        addon_rows(&db, &mut addon);
        let Some(chunks) = db.get("chunks").and_then(Lua::items) else {
            continue;
        };
        for item in chunks {
            let Some(ch) = chunk(&item.1) else { continue };
            if ch.q <= last {
                continue;
            }
            let Some(text) = decode(&ch.e, &ch.z) else {
                bad += 1;
                continue;
            };
            let evs = events::parse_chunk_text(&text, ch.t);
            let run = runs
                .entry(ch.c.clone())
                .or_insert_with(|| CharRun::new(ch.q));
            run.chunks += 1;
            run.last_q = run.last_q.max(ch.q);
            for ev in evs {
                if let Some(kind) = events::name_kind(&ev.code)
                    && ev.fields.len() >= 2
                {
                    run.names
                        .insert(events::name_key(kind, &ev.fields[0]), ev.fields[1].clone());
                }
                if let Some(line) = events::txt_line(&ev, &run.names) {
                    run.txt.push_str(&line);
                    run.txt.push('\n');
                }
                run.scl.push_str(&events::scl_line(&ev));
                run.scl.push('\n');
                run.events += 1;
            }
            state.0.insert(key.clone(), ch.q);
        }
    }

    for (name, run) in &runs {
        let dir = out.join(name);
        fs::create_dir_all(dir.join("payloads"))?;
        if !run.txt.is_empty() {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("scarlet.txt"))?;
            f.write_all(run.txt.as_bytes())?;
        }
        let payload =
            zstd::stream::encode_all(format!("scarlet\t1\t{name}\n{}", run.scl).as_bytes(), 19)?;
        // Through a temp file: an upload or a reload reading the folder meanwhile never meets
        // half a payload.
        let dest = dir
            .join("payloads")
            .join(format!("{}-{}.scl", run.first_q, run.last_q));
        let tmp = dest.with_extension("scl.tmp");
        fs::write(&tmp, &payload)?;
        fs::rename(&tmp, &dest)?;

        // Rebuild the relational tables from every payload this character
        // has, in full, on each export.
        let evs = load_events(&dir, name)?;
        if let Err(e) = tables::write_char_tables(&evs, name, &dir.join("tables")) {
            eprintln!("scarlet: {name}: tables: {e}");
        }
        let ov = build_overview(name, &evs);
        if let Err(e) = write_overview(&dir, &ov) {
            eprintln!("scarlet: {name}: overview: {e}");
        }
        if let Some(rows) = addon.get(name) {
            crosscheck(name, &ov, rows);
        }
        println!(
            "{name}: {} chunks, {} events, {} bytes (.scl)",
            run.chunks,
            run.events,
            payload.len()
        );
    }

    for (name, run) in &addon_runs {
        let size = addons::write_run(&out.join(name), name, run)?;
        println!(
            "{name}: {} addon chunks, {} addon lines, {size} bytes (.scl)",
            run.chunks, run.events
        );
    }

    save_state(&state_path, &state)?;
    let wrote = !runs.is_empty() || !addon_runs.is_empty();
    if runs.is_empty() && addon_runs.is_empty() {
        let fail = if bad > 0 {
            format!(", {bad} chunk(s) failed to decode")
        } else {
            String::new()
        };
        println!(
            "no new chunks ({} SavedVariables file(s){fail})",
            files.len()
        );
    }
    for wow in wows {
        write_digest(wow, out);
    }
    Ok(wrote)
}

/// Every payload of one character, in time order.
fn load_events(dir: &Path, name: &str) -> std::io::Result<Vec<archive::Event>> {
    let mut evs: Vec<archive::Event> = Vec::new();
    for p in archive::char_payloads(dir)? {
        match archive::read_payload(&p) {
            Ok(pl) => evs.extend(pl.events),
            Err(e) => eprintln!("scarlet: {name}: {}: {e}", p.display()),
        }
    }
    evs.sort_by_key(|e| e.ms);
    Ok(evs)
}

fn build_overview(name: &str, evs: &[archive::Event]) -> Overview {
    overview::build(
        name,
        evs.iter()
            .map(|e| (e.ms, e.code.as_str(), e.fields.as_slice())),
    )
}

fn write_overview(dir: &Path, ov: &Overview) -> Result<(), Box<dyn Error>> {
    let tmp = dir.join("overview.json.tmp");
    fs::write(&tmp, serde_json::to_string(ov)?)?;
    fs::rename(&tmp, dir.join("overview.json"))?;
    Ok(())
}

/// The numbers of one session row the addon keeps in SavedVariables
/// (`ScarletDB.chars[<name>].ov.sess`).
struct AddonRow {
    t: i64,
    p: i64,
    k: i64,
    dd: i64,
    q: i64,
    xp: i64,
    g: i64,
    s: i64,
    r: i64,
}

fn addon_rows(db: &Lua, into: &mut BTreeMap<String, Vec<AddonRow>>) {
    let Some(chars) = db.get("chars").and_then(Lua::items) else {
        return;
    };
    for (name, c) in chars {
        let Some(sess) = c.get("ov").and_then(|o| o.get("sess")).and_then(Lua::items) else {
            continue;
        };
        for (_, row) in sess {
            if row.get("t").and_then(Lua::as_num).is_none() {
                continue;
            }
            let n = |k: &str| {
                row.get(k)
                    .and_then(Lua::as_num)
                    .map_or(0, |v| v.round() as i64)
            };
            into.entry(name.clone()).or_default().push(AddonRow {
                t: n("t"),
                p: n("p"),
                k: n("k"),
                dd: n("dd"),
                q: n("q"),
                xp: n("xk") + n("xd") + n("xq") + n("xe") + n("xo"),
                g: n("g"),
                s: n("s"),
                r: n("r"),
            });
        }
    }
}

/// Compare the addon's session rows with the ones built from the recorded
/// lines; one `check:` line per number that differs.
fn crosscheck(name: &str, ov: &Overview, rows: &[AddonRow]) {
    let newest = ov.sessions.first().map_or(0, |s| s.t);
    for a in rows {
        let Some(c) = ov
            .sessions
            .iter()
            .filter(|s| (s.t - a.t).abs() <= 3)
            .min_by_key(|s| (s.t - a.t).abs())
        else {
            // A session whose lines are still in the addon's buffer has not
            // been exported yet.
            if a.t < newest - 3 {
                println!("check: {name} t={} no companion session", a.t);
            }
            continue;
        };
        let diff = |field: &str, addon: i64, companion: i64, tol: i64| {
            if (addon - companion).abs() > tol {
                println!(
                    "check: {name} t={} {field} addon={addon} companion={companion}",
                    a.t
                );
            }
        };
        // The companion adds the reload before the next session (docs/OVERVIEW.md, Played); rows the
        // addon wrote before it did the same lack it, up to two minutes.
        if !(0..=120).contains(&(c.p - a.p)) {
            diff("p", a.p, c.p, 5);
        }
        diff("k", a.k, c.k, 0);
        diff("dd", a.dd, c.dd, 0);
        diff("q", a.q, c.q, 0);
        diff("xp", a.xp, c.xk + c.xd + c.xq + c.xe + c.xo, 0);
        diff("g", a.g, c.g, 0);
        diff("s", a.s, c.s, 0);
        diff("r", a.r, c.r, 0);
    }
}

/// A payload newer than overview.json (history imported or copied in after
/// the last export of this character) means the file misses sessions.
fn overview_is_stale(dir: &Path, overview: &Path) -> bool {
    let modified = |p: &Path| fs::metadata(p).and_then(|m| m.modified()).ok();
    let Some(built) = modified(overview) else {
        return true;
    };
    archive::char_payloads(dir)
        .into_iter()
        .flatten()
        .filter_map(|p| modified(&p))
        .any(|t| t > built)
}

/// Write Digest.lua into the addon folder from every character's
/// overview.json (built here for a character that has none yet, or whose
/// payloads are newer than it). A failure is a note, not an error: the next
/// export tries again.
fn write_digest(wow: &Path, out: &Path) {
    let addon = crate::gamedir::game_dir(wow)
        .join("Interface")
        .join("AddOns")
        .join("Scarlet");
    if !addon.is_dir() {
        println!("digest: no addon folder at {}", addon.display());
        return;
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(out)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.join("payloads").is_dir())
        .collect();
    dirs.sort();
    let mut chars: Vec<(String, Overview)> = Vec::new();
    for d in dirs {
        let Some(name) = d.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let file = d.join("overview.json");
        if !file.is_file() || overview_is_stale(&d, &file) {
            match load_events(&d, &name) {
                Ok(evs) if !evs.is_empty() => {
                    if let Err(e) = write_overview(&d, &build_overview(&name, &evs)) {
                        eprintln!("scarlet: {name}: overview: {e}");
                    }
                }
                _ => continue,
            }
        }
        let ov = fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<Overview>(&t).ok());
        match ov {
            Some(ov) if !ov.sessions.is_empty() => chars.push((name, ov)),
            Some(_) => {}
            None => eprintln!("scarlet: {name}: overview.json unreadable"),
        }
    }
    if chars.is_empty() {
        println!("digest: no characters with sessions; Digest.lua not written");
        return;
    }
    // Another character of this player (the second client of a dual box) is not someone they
    // played with, so the list of groupmates leaves it out.
    let own: Vec<String> = chars.iter().map(|(n, _)| n.clone()).collect();
    for (_, ov) in &mut chars {
        ov.mates.retain(|m| !own.contains(&m.name));
    }
    let made = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    for (name, ov) in &chars {
        let eta = ov.eta(made);
        let targets: Vec<String> = eta
            .targets
            .iter()
            .map(|t| format!("L{} {} ({}m) {}", t.level, t.time, t.minutes, t.date))
            .collect();
        println!("eta: {name} {} | {}", targets.join(" | "), eta.basis);
    }
    let text = overview::digest_lua(&chars, made);
    let tmp = addon.join("Digest.lua.tmp");
    let dest = addon.join("Digest.lua");
    if let Err(e) = fs::write(&tmp, &text) {
        println!(
            "digest: could not write {}: {e}; the next export tries again",
            tmp.display()
        );
        return;
    }
    match fs::rename(&tmp, &dest) {
        Ok(()) => println!(
            "digest: {} character(s), {} bytes -> {}",
            chars.len(),
            text.len(),
            dest.display()
        ),
        Err(e) => println!(
            "digest: could not replace {} ({e}); Digest.lua.tmp stays for the next export",
            dest.display()
        ),
    }
}

/// Export once, then again whenever any Scarlet.lua mtime changes.
/// Ctrl+C exits.
pub fn watch_all(wows: &[PathBuf], out: &Path) -> Result<(), Box<dyn Error>> {
    let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
    let mut first = true;
    println!("watching {} folder(s) (Ctrl+C to exit)", wows.len());
    loop {
        let mut changed = false;
        for sv in find_sv_all(wows)? {
            let m = fs::metadata(&sv)?.modified()?;
            if seen.get(&sv) != Some(&m) {
                changed = true;
            }
            seen.insert(sv, m);
        }
        if changed || first {
            first = false;
            export_all(wows, out)?;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

/// zstd-compress finished WoWCombatLog txt files; originals untouched.
///
/// Each running client writes its own `WoWCombatLog-<date>_<time>.txt` in `Logs`, so two
/// clients never share a file. A log does not say which character wrote it, so the files are
/// kept whole and not split by character. A file a client
/// still has open is under ten minutes old and waits; one that cannot be read is skipped and
/// the others go on.
pub fn logs_all(wows: &[PathBuf], out: &Path) -> Result<(), Box<dyn Error>> {
    let dest = out.join("combatlogs");
    fs::create_dir_all(&dest)?;
    let mut done = 0;
    for wow in wows {
        let src_dir = crate::gamedir::game_dir(wow).join("Logs");
        let Ok(entries) = fs::read_dir(&src_dir) else {
            println!("no Logs folder at {}", src_dir.display());
            continue;
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("WoWCombatLog") || !name.ends_with(".txt") {
                continue;
            }
            let md = entry.metadata()?;
            if md.len() == 0 {
                continue;
            }
            let age = SystemTime::now().duration_since(md.modified()?)?;
            if age < Duration::from_secs(600) {
                continue;
            }
            let target = dest.join(format!("{name}.zst"));
            if target.exists() {
                continue;
            }
            let src = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    println!("{name}: skipped, {e}");
                    continue;
                }
            };
            // Through a temp name, so a stop halfway never leaves a short .zst that counts as done.
            let tmp = dest.join(format!("{name}.zst.tmp"));
            let mut dst = File::create(&tmp)?;
            zstd::stream::copy_encode(&mut BufReader::new(src), &mut dst, 3)?;
            drop(dst);
            fs::rename(&tmp, &target)?;
            done += 1;
            println!("{name} -> {}", target.display());
        }
    }
    if done == 0 {
        println!("no new combat logs");
    }
    Ok(())
}

/// One game account folder (`WTF\Account\<name>`, such as `1570195#1`): when it last saved
/// Scarlet's file and the characters in it.
pub struct Account {
    pub name: String,
    /// Epoch seconds the account's Scarlet.lua was last written; 0 when it has none yet.
    pub saved: i64,
    pub characters: Vec<String>,
}

/// Characters per SavedVariables file, kept until the file changes: the folder list asks again
/// each time it is opened and the files can be megabytes.
static CHARS_SEEN: std::sync::Mutex<Vec<(PathBuf, SystemTime, Vec<String>)>> =
    std::sync::Mutex::new(Vec::new());

fn characters_in(sv: &Path, modified: SystemTime) -> Vec<String> {
    let mut seen = CHARS_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, c)) = seen.iter().find(|(p, t, _)| p == sv && *t == modified) {
        return c.clone();
    }
    let mut names: Vec<String> = fs::read_to_string(sv)
        .ok()
        .and_then(|text| lua::parse_main(&text).ok())
        .and_then(|db| {
            db.get("chars")
                .and_then(Lua::items)
                .map(|c| c.iter().map(|(n, _)| n.clone()).collect())
        })
        .unwrap_or_default();
    names.sort();
    seen.retain(|(p, _, _)| p != sv);
    seen.push((sv.to_path_buf(), modified, names.clone()));
    names
}

/// The game accounts of one WoW folder, each with the characters its Scarlet file holds. An
/// account folder without a `SavedVariables` folder is not an account that has played.
pub fn accounts(wow: &Path) -> Vec<Account> {
    let dir = crate::gamedir::game_dir(wow).join("WTF").join("Account");
    let mut out: Vec<Account> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("SavedVariables").is_dir())
        .filter(|e| e.file_name() != "SavedVariables")
        .map(|e| {
            let sv = e.path().join("SavedVariables").join("Scarlet.lua");
            let modified = fs::metadata(&sv).and_then(|m| m.modified()).ok();
            Account {
                name: e.file_name().to_string_lossy().into_owned(),
                saved: modified
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs() as i64),
                characters: modified.map_or_else(Vec::new, |t| characters_in(&sv, t)),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
