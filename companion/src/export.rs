//! The three commands: export, watch, logs. Plus SavedVariables discovery
//! and state.json bookkeeping.

use crate::addons;
use crate::archive;
use crate::counts;
use crate::dungeonjournal;
use crate::events;
use crate::featscore;
use crate::fishing;
use crate::foreverfishing;
use crate::import;
use crate::importarchive::Archive;
use crate::inventory::{self, Inventory};
use crate::lua::{self, Lua};
use crate::marks::{self, Cursor, Seen};
use crate::mastercraft;
use crate::naowhforever;
use crate::overview::{self, Overview};
use crate::recap;
use crate::rxp;
use crate::tables;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
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
    first_t: i64,
}

impl CharRun {
    fn new(q: u64, t: i64) -> Self {
        Self {
            txt: String::new(),
            scl: String::new(),
            names: HashMap::new(),
            chunks: 0,
            events: 0,
            first_q: q,
            last_q: q,
            first_t: t,
        }
    }
}

/// What the exports so far saw of the game's saved store: when it started over, and the chunks
/// that would not decode.
pub fn store_notes(out: &Path) -> marks::StoreNotes {
    marks::notes(&load_state(&out.join("state.json")).0)
}

fn seq_of(db: &Lua) -> Option<u64> {
    db.get("seq")?.as_int().and_then(|n| u64::try_from(n).ok())
}

fn report_store(sv: &Path, addons: bool, cursor: &Cursor) {
    if let Some((at, since)) = cursor.restarted_at() {
        println!(
            "{}: {}",
            sv.display(),
            marks::restart_line(at, since, addons)
        );
    }
    if cursor.unreadable_count() > 0 {
        eprintln!(
            "scarlet: {}: {}",
            sv.display(),
            marks::unreadable_line(cursor.unreadable_count())
        );
    }
}

fn line_id(ms: i64, code: &str, fields: &[String]) -> u64 {
    let mut h = DefaultHasher::new();
    (ms, code, fields.join("\t")).hash(&mut h);
    h.finish()
}

struct Archived<'a> {
    out: &'a Path,
    main: HashMap<String, HashSet<u64>>,
    addon: HashMap<String, HashSet<u64>>,
}

impl<'a> Archived<'a> {
    fn new(out: &'a Path) -> Self {
        Self {
            out,
            main: HashMap::new(),
            addon: HashMap::new(),
        }
    }

    fn holds_chunk(&mut self, ch: &Chunk) -> bool {
        let Some(text) = decode(&ch.e, &ch.z) else {
            return true;
        };
        let held = self.main.entry(ch.c.clone()).or_insert_with(|| {
            load_events(&self.out.join(&ch.c), &ch.c)
                .unwrap_or_default()
                .iter()
                .map(|e| line_id(e.ms, &e.code, &e.fields))
                .collect()
        });
        events::parse_chunk_text(&text, ch.t)
            .iter()
            .all(|e| held.contains(&line_id(e.cs * 10, &e.code, &e.fields)))
    }

    fn holds_addon_chunk(&mut self, ch: &Chunk) -> bool {
        let Some(text) = decode(&ch.e, &ch.z) else {
            return true;
        };
        let id = |cs: i64, key: &str, code: &str, fields: &[String]| {
            line_id(cs * 10, &format!("{key}\t{code}"), fields)
        };
        let held = self.addon.entry(ch.c.clone()).or_insert_with(|| {
            addons::read_payloads(&self.out.join(&ch.c))
                .iter()
                .map(|e| id(e.cs, &e.key, &e.code, &e.fields))
                .collect()
        });
        addons::parse_chunk_text(&text, ch.t)
            .iter()
            .all(|e| held.contains(&id(e.cs, &e.key, &e.code, &e.fields)))
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
    wows.iter().any(|w| data_addon_stale(w))
        || tables_rev_differs(out)
        || imports_stale(wows, out)
        || fishing_stale(wows, out)
        || counts_stale(wows, out)
        || find_sv_all(wows).unwrap_or_default().iter().any(|sv| {
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

    let mut archived = Archived::new(out);
    for sv in &files {
        let key = state_key(sv);
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
                    let mut cursor = Cursor::new(&state.0, &akey, seq_of(&adb));
                    addons::collect(
                        &adb,
                        &mut cursor,
                        &mut |ch| archived.holds_addon_chunk(ch),
                        &mut addon_runs,
                        &mut bad,
                    );
                    report_store(sv, true, &cursor);
                    cursor.finish(&mut state.0);
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
        let mut cursor = Cursor::new(&state.0, &key, seq_of(&db));
        for item in chunks {
            let Some(ch) = chunk(&item.1) else { continue };
            let seen = cursor.judge(&ch, || archived.holds_chunk(&ch));
            if seen == Seen::Read {
                cursor.skipped(&ch);
                continue;
            }
            let Some(text) = decode(&ch.e, &ch.z) else {
                bad += 1;
                cursor.unreadable();
                continue;
            };
            let evs = events::parse_chunk_text(&text, ch.t);
            let run = runs
                .entry(ch.c.clone())
                .or_insert_with(|| CharRun::new(ch.q, ch.t));
            run.chunks += 1;
            run.last_q = run.last_q.max(ch.q);
            run.first_t = run.first_t.min(ch.t);
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
            cursor.took(&ch, seen);
        }
        report_store(sv, false, &cursor);
        cursor.finish(&mut state.0);
    }

    let mut own = roster(out);
    for name in runs.keys() {
        if !own.contains(name) {
            own.push(name.clone());
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
        archive::write_payload(
            &dir.join("payloads"),
            run.first_q,
            run.last_q,
            run.first_t,
            &payload,
        )?;

        // Rebuild the relational tables from every payload this character
        // has, in full, on each export.
        let evs = load_events(&dir, name)?;
        if let Err(e) = tables::write_char_tables(&evs, name, &dir.join("tables")) {
            eprintln!("scarlet: {name}: tables: {e}");
        }
        let ov = build_overview(name, &evs, &own);
        if let Err(e) = write_overview(&dir, &ov) {
            eprintln!("scarlet: {name}: overview: {e}");
        }
        write_inventory(&dir, name, &evs);
        write_recaps(&dir, name, &evs);
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

    let mut touched: Vec<String> = runs.keys().cloned().collect();
    let rebuild_tables = tables_rev_differs(out);
    let mut tables_rebuilt = true;
    if rebuild_tables {
        for name in own.iter().filter(|n| !runs.contains_key(*n)) {
            let dir = out.join(name);
            if !dir.join("payloads").is_dir() {
                continue;
            }
            let written = load_events(&dir, name)
                .map_err(|e| e.to_string())
                .and_then(|evs| {
                    tables::write_char_tables(&evs, name, &dir.join("tables"))
                        .map_err(|e| e.to_string())
                });
            if let Err(e) = written {
                eprintln!("scarlet: {name}: tables: {e}");
                tables_rebuilt = false;
            }
            touched.push(name.clone());
        }
    }
    refresh_imports(wows, out, &mut state, &touched, &own);
    if rebuild_tables
        && tables_rebuilt
        && let Err(e) = fs::write(out.join("tables.rev"), TABLES_REV)
    {
        eprintln!("scarlet: tables.rev: {e}");
    }
    let fishing_wrote = refresh_fishing(wows, out, &mut state);
    let counts_wrote = refresh_counts(wows, out, &mut state);
    for name in &own {
        if let Err(e) = counts::rebuild(&out.join(name), name) {
            eprintln!("scarlet: {name}: counts: {e}");
        }
    }
    save_state(&state_path, &state)?;
    let wrote = !runs.is_empty() || !addon_runs.is_empty() || fishing_wrote || counts_wrote;
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
    let accounts = save_accounts(wows, out);
    for wow in wows {
        write_digest(wow, out, &accounts);
    }
    Ok(wrote)
}

/// Sets whether the digest lists characters by game account and writes it again at once, so the
/// game sees the change at its next `/reload` without waiting for a recording.
pub fn set_group_by_account(wows: &[PathBuf], out: &Path, on: bool) {
    let _one = EXPORT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    overview::set_group_by_account(on);
    let accounts = save_accounts(wows, out);
    for wow in wows {
        write_digest(wow, out, &accounts);
    }
}

/// Every payload of one character, in time order.
fn load_events(dir: &Path, name: &str) -> std::io::Result<Vec<archive::Event>> {
    let payloads =
        archive::char_payloads(dir)?
            .into_iter()
            .filter_map(|p| match archive::read_payload(&p) {
                Ok(pl) => Some(pl.events),
                Err(e) => {
                    eprintln!("scarlet: {name}: {}: {e}", p.display());
                    None
                }
            });
    let mut evs = archive::merge(payloads);
    evs.sort_by_key(|e| e.ms);
    Ok(evs)
}

fn build_overview(name: &str, evs: &[archive::Event], own: &[String]) -> Overview {
    overview::build_among(
        name,
        evs.iter()
            .map(|e| (e.ms, e.code.as_str(), e.fields.as_slice())),
        own,
    )
}

fn roster(out: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(out)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("payloads").is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

const OVERVIEW_REV: &str = "10";
const TABLES_REV: &str = "1";

fn overview_rev_differs(out: &Path) -> bool {
    fs::read_to_string(out.join("overview.rev")).map_or(true, |t| t.trim() != OVERVIEW_REV)
}

fn tables_rev_differs(out: &Path) -> bool {
    !roster(out).is_empty()
        && fs::read_to_string(out.join("tables.rev")).map_or(true, |t| t.trim() != TABLES_REV)
}

fn overview_rev_stale(out: &Path) -> bool {
    let file = out.join("overview.rev");
    let stale = overview_rev_differs(out);
    if stale && let Err(e) = fs::write(&file, OVERVIEW_REV) {
        eprintln!("scarlet: {}: {e}", file.display());
    }
    stale
}

fn write_recaps(dir: &Path, name: &str, evs: &[archive::Event]) {
    let log = recap::Log::new(evs.iter().map(|e| recap::Rec {
        ms: e.ms,
        code: e.code.as_str(),
        fields: e.fields.as_slice(),
    }));
    let (recaps, tallies) = recap::for_character(name, &log);
    let saved = featscore::saved(&featscore::score(&tallies));
    for (file, text) in [
        ("recaps.json", serde_json::to_string(&recaps)),
        ("feats.json", serde_json::to_string(&saved)),
    ] {
        let written = text
            .map_err(|e| e.to_string())
            .and_then(|text| write_atomic(&dir.join(file), &text).map_err(|e| e.to_string()));
        if let Err(e) = written {
            eprintln!("scarlet: {name}: {file}: {e}");
        }
    }
}

fn read_feats(dir: &Path) -> Option<featscore::Saved> {
    serde_json::from_str(&fs::read_to_string(dir.join("feats.json")).ok()?).ok()
}

fn read_kept(wow: &Path) -> BTreeMap<String, featscore::Kept> {
    let mut kept = BTreeMap::new();
    for sv in find_sv_all(&[wow.to_path_buf()]).unwrap_or_default() {
        let Ok(text) = fs::read_to_string(&sv) else {
            continue;
        };
        let Ok(db) = lua::parse_main(&text) else {
            continue;
        };
        for (name, ch) in db.get("chars").and_then(Lua::items).unwrap_or_default() {
            let lifetime = ch
                .items()
                .unwrap_or_default()
                .iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_int()?)))
                .collect();
            let seen = ch.get("featSeen").and_then(Lua::items).map(|rows| {
                rows.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_int()?)))
                    .collect()
            });
            kept.insert(name.clone(), featscore::Kept { lifetime, seen });
        }
    }
    kept
}

const SEEN_SENT: &str = "feats-seen-sent.json";

fn load_seen_sent(out: &Path) -> BTreeMap<String, BTreeMap<String, u8>> {
    fs::read_to_string(out.join(SEEN_SENT))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// The feat tiers the player has been shown in game since the last time they were sent to the website, one entry
/// for each character that has any.
pub fn feats_seen_unsent(wows: &[PathBuf], out: &Path) -> Vec<(String, BTreeMap<String, u8>)> {
    let sent = load_seen_sent(out);
    let mut shown: BTreeMap<String, BTreeMap<String, u8>> = BTreeMap::new();
    for wow in wows {
        for (name, kept) in read_kept(wow) {
            let entry = shown.entry(name).or_default();
            for (key, tier) in kept.seen.unwrap_or_default() {
                if !crate::feats_catalog::FEATS.iter().any(|f| f.key == key) {
                    continue;
                }
                let tier = tier.clamp(0, 5) as u8;
                let at = entry.entry(key).or_insert(0);
                *at = (*at).max(tier);
            }
        }
    }
    shown
        .into_iter()
        .filter_map(|(name, tiers)| {
            let done = sent.get(&name);
            let fresh: BTreeMap<String, u8> = tiers
                .into_iter()
                .filter(|(key, tier)| *tier > done.and_then(|d| d.get(key)).copied().unwrap_or(0))
                .collect();
            (!fresh.is_empty()).then_some((name, fresh))
        })
        .collect()
}

/// Records that `shown` reached the website for `character`, so the next export does not send it again.
pub fn feats_seen_sent(out: &Path, character: &str, shown: &BTreeMap<String, u8>) {
    let mut sent = load_seen_sent(out);
    let entry = sent.entry(character.to_string()).or_default();
    for (key, tier) in shown {
        let at = entry.entry(key.clone()).or_insert(0);
        *at = (*at).max(*tier);
    }
    let written = serde_json::to_string(&sent)
        .map_err(|e| e.to_string())
        .and_then(|text| write_atomic(&out.join(SEEN_SENT), &text).map_err(|e| e.to_string()));
    if let Err(e) = written {
        eprintln!("scarlet: {SEEN_SENT}: {e}");
    }
}

fn refresh_web_sync(out: &Path) {
    let Some(mut web) = overview::web() else {
        return;
    };
    let state = crate::upload::state::load(out);
    web.synced = state
        .characters
        .iter()
        .map(|(name, c)| (name.clone(), c.last_sync as i64))
        .collect();
    web.sync = web.synced.values().copied().max().unwrap_or(0);
    overview::set_web(Some(web));
}

/// Sets what the digest says about the website and writes it again at once, as `set_group_by_account` does.
pub fn set_web(wows: &[PathBuf], out: &Path, web: Option<overview::WebDigest>) {
    let _one = EXPORT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    overview::set_web(web);
    let accounts = save_accounts(wows, out);
    for wow in wows {
        write_digest(wow, out, &accounts);
    }
}

fn read_recaps(dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(dir.join("recaps.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_inventory(dir: &Path, name: &str, evs: &[archive::Event]) {
    let inv = inventory::build(
        evs.iter()
            .map(|e| (e.ms, e.code.as_str(), e.fields.as_slice())),
    );
    let tmp = dir.join("inventory.json.tmp");
    let written = serde_json::to_string(&inv)
        .map_err(|e| e.to_string())
        .and_then(|text| fs::write(&tmp, text).map_err(|e| e.to_string()))
        .and_then(|()| fs::rename(&tmp, dir.join("inventory.json")).map_err(|e| e.to_string()));
    if let Err(e) = written {
        eprintln!("scarlet: {name}: inventory: {e}");
    }
}

fn write_overview(dir: &Path, ov: &Overview) -> Result<(), Box<dyn Error>> {
    let tmp = dir.join("overview.json.tmp");
    let mut ov = ov.clone();
    ov.imported = import::read_view(dir);
    fs::write(&tmp, serde_json::to_string(&ov)?)?;
    fs::rename(&tmp, dir.join("overview.json"))?;
    Ok(())
}

fn import_stamp(path: &Path) -> (String, u64) {
    source_stamp(path, "import2")
}

fn source_stamp(path: &Path, tag: &str) -> (String, u64) {
    let stamp = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    (format!("{}#{tag}", state_key(path)), stamp)
}

fn fishing_accounts(wows: &[PathBuf]) -> Vec<fishing::Account> {
    wows.iter()
        .flat_map(|w| fishing::find(&crate::gamedir::game_dir(w)))
        .collect()
}

const FISHING_STAMP: &str = "fishing3";

fn fishing_changed(state: &State, path: &Path) -> bool {
    let (key, stamp) = source_stamp(path, FISHING_STAMP);
    state.0.get(&key) != Some(&stamp)
}

fn recorded_fishing_chars<'a>(acc: &'a fishing::Account, out: &Path) -> Vec<&'a (String, PathBuf)> {
    acc.chars
        .iter()
        .filter(|(name, _)| out.join(name).join("payloads").is_dir())
        .collect()
}

fn fishing_stale(wows: &[PathBuf], out: &Path) -> bool {
    let state = load_state(&out.join("state.json"));
    fishing_accounts(wows).iter().any(|acc| {
        let chars = recorded_fishing_chars(acc, out);
        let account_changed = acc
            .db
            .as_deref()
            .is_some_and(|p| fishing_changed(&state, p));
        !chars.is_empty()
            && (account_changed || chars.iter().any(|(_, file)| fishing_changed(&state, file)))
    })
}

fn refresh_fishing(wows: &[PathBuf], out: &Path, state: &mut State) -> bool {
    let mut wrote = false;
    for acc in fishing_accounts(wows) {
        let chars: Vec<(String, PathBuf)> = recorded_fishing_chars(&acc, out)
            .into_iter()
            .cloned()
            .collect();
        if chars.is_empty() {
            continue;
        }
        let stamps: Vec<(String, u64)> = acc
            .db
            .iter()
            .chain(chars.iter().map(|(_, file)| file))
            .map(|p| source_stamp(p, FISHING_STAMP))
            .collect();
        if stamps
            .iter()
            .all(|(key, stamp)| state.0.get(key) == Some(stamp))
        {
            continue;
        }
        state.0.extend(stamps);
        for (name, read) in fishing::read_counted(acc.db.as_deref(), &chars) {
            match read {
                Ok(read) => {
                    let dir = out.join(&name);
                    match fishing::write(&dir, &name, &read.lines) {
                        Ok(()) => {
                            println!("{name}: {} lines from The Fishing Log", read.lines.len());
                            wrote = true;
                        }
                        Err(e) => eprintln!("scarlet: {name}: The Fishing Log: {e}"),
                    }
                    match counts::feed(&dir, read.counts) {
                        Ok(0) => {}
                        Ok(n) => println!("{name}: {n} fishing counts archived"),
                        Err(e) => eprintln!("scarlet: {name}: counts: {e}"),
                    }
                }
                Err(e) => eprintln!("scarlet: {name}: The Fishing Log: {e}"),
            }
        }
    }
    wrote
}

const COUNTS_STAMP: &str = "counts3";

struct CountSources {
    files: Vec<PathBuf>,
    character_files: Vec<(String, PathBuf)>,
    exported: Vec<String>,
}

fn count_sources(wows: &[PathBuf], out: &Path) -> Vec<CountSources> {
    let mut found = Vec::new();
    for wow in wows {
        for account in crate::gamedir::account_dirs(&crate::gamedir::game_dir(wow)) {
            let files: Vec<PathBuf> = [foreverfishing::FILE, mastercraft::FILE, naowhforever::FILE]
                .iter()
                .map(|f| account.join("SavedVariables").join(f))
                .filter(|p| p.is_file())
                .collect();
            let exported: Vec<String> = crate::gamedir::character_names(&account)
                .into_iter()
                .filter(|name| out.join(name).join("payloads").is_dir())
                .collect();
            let character_files: Vec<(String, PathBuf)> = exported
                .iter()
                .filter_map(|name| {
                    let sv = crate::gamedir::character_saved_variables(&account, name)?;
                    Some([rxp::FILE, dungeonjournal::FILE].map(|f| (name.clone(), sv.join(f))))
                })
                .flatten()
                .filter(|(_, p)| p.is_file())
                .collect();
            if (!files.is_empty() || !character_files.is_empty()) && !exported.is_empty() {
                found.push(CountSources {
                    files,
                    character_files,
                    exported,
                });
            }
        }
    }
    found
}

fn count_stamps(src: &CountSources) -> Vec<(String, u64)> {
    let account_files = src.files.iter().flat_map(|file| {
        src.exported
            .iter()
            .map(move |name| source_stamp(file, &format!("{COUNTS_STAMP}:{name}")))
    });
    let character_files = src
        .character_files
        .iter()
        .map(|(_, file)| source_stamp(file, COUNTS_STAMP));
    account_files.chain(character_files).collect()
}

fn counts_stale(wows: &[PathBuf], out: &Path) -> bool {
    let state = load_state(&out.join("state.json"));
    count_sources(wows, out).iter().any(|src| {
        count_stamps(src)
            .iter()
            .any(|(key, stamp)| state.0.get(key) != Some(stamp))
    })
}

fn read_table(file: &Path, table: &str) -> Option<Lua> {
    let text = fs::read_to_string(file).map_err(|e| e.to_string());
    match text.and_then(|t| lua::parse_named(&t, table)) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("scarlet: {}: {e}", file.file_name()?.to_string_lossy());
            None
        }
    }
}

fn feed_forever_fishing(file: &Path, exported: &[String], out: &Path) -> bool {
    let Some(db) = read_table(file, foreverfishing::TABLE) else {
        return false;
    };
    let (by_name, unmatched) = foreverfishing::assign(foreverfishing::read(&db), exported);
    if unmatched > 0 {
        println!("{unmatched} ForeverFishing sessions matched no exported character");
    }
    let mut wrote = false;
    for (name, records) in by_name {
        match counts::feed(&out.join(&name), records) {
            Ok(0) => {}
            Ok(n) => {
                println!("{name}: {n} ForeverFishing sessions archived");
                wrote = true;
            }
            Err(e) => eprintln!("scarlet: {name}: counts: {e}"),
        }
    }
    wrote
}

fn feed_mastercraft(file: &Path, exported: &[String], out: &Path) -> bool {
    let Some(modified) = fs::metadata(file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
    else {
        return false;
    };
    let Some(db) = read_table(file, mastercraft::TABLE) else {
        return false;
    };
    let mut totals = mastercraft::life(&db, modified)
        .into_iter()
        .collect::<Vec<_>>();
    totals.extend(mastercraft::gathers(&db, modified));
    if totals.is_empty() {
        return false;
    }
    let dirs: Vec<PathBuf> = exported.iter().map(|name| out.join(name)).collect();
    match counts::feed_account(&dirs, totals) {
        Ok(0) => false,
        Ok(n) => {
            println!("{n} Mastercraft account totals archived");
            true
        }
        Err(e) => {
            eprintln!("scarlet: Mastercraft: counts: {e}");
            false
        }
    }
}

fn feed_naowhforever(file: &Path, exported: &[String], out: &Path) -> bool {
    let Some(db) = read_table(file, naowhforever::TABLE) else {
        return false;
    };
    let mut wrote = false;
    for (name, records) in naowhforever::read(&db, exported) {
        match counts::feed(&out.join(&name), records) {
            Ok(0) => {}
            Ok(n) => {
                println!("{name}: {n} NaowhForever boss kills archived");
                wrote = true;
            }
            Err(e) => eprintln!("scarlet: {name}: counts: {e}"),
        }
    }
    wrote
}

fn feed_character(
    file: &Path,
    name: &str,
    out: &Path,
    table: &str,
    label: &str,
    read: impl FnOnce(&Lua) -> Vec<counts::Record>,
) -> bool {
    let Some(db) = read_table(file, table) else {
        return false;
    };
    match counts::feed(&out.join(name), read(&db)) {
        Ok(0) => false,
        Ok(n) => {
            println!("{name}: {n} {label} archived");
            true
        }
        Err(e) => {
            eprintln!("scarlet: {name}: counts: {e}");
            false
        }
    }
}

fn refresh_counts(wows: &[PathBuf], out: &Path, state: &mut State) -> bool {
    let mut wrote = false;
    for src in count_sources(wows, out) {
        let stamps = count_stamps(&src);
        if stamps
            .iter()
            .all(|(key, stamp)| state.0.get(key) == Some(stamp))
        {
            continue;
        }
        state.0.extend(stamps);
        for file in &src.files {
            match file.file_name().and_then(|n| n.to_str()) {
                Some(foreverfishing::FILE) => {
                    wrote |= feed_forever_fishing(file, &src.exported, out);
                }
                Some(mastercraft::FILE) => wrote |= feed_mastercraft(file, &src.exported, out),
                Some(naowhforever::FILE) => wrote |= feed_naowhforever(file, &src.exported, out),
                _ => {}
            }
        }
        for (name, file) in &src.character_files {
            match file.file_name().and_then(|n| n.to_str()) {
                Some(rxp::FILE) => {
                    wrote |= feed_character(
                        file,
                        name,
                        out,
                        rxp::TABLE,
                        "RXPGuides level records",
                        |db| rxp::read(db, name),
                    );
                }
                Some(dungeonjournal::FILE) => {
                    wrote |= feed_character(
                        file,
                        name,
                        out,
                        dungeonjournal::TABLE,
                        "ForeverDungeonJournal runs",
                        dungeonjournal::read,
                    );
                }
                _ => {}
            }
        }
    }
    wrote
}

fn imports_stale(wows: &[PathBuf], out: &Path) -> bool {
    let sources: Vec<PathBuf> = wows
        .iter()
        .flat_map(|w| import::find_files(&crate::gamedir::game_dir(w)))
        .map(|(path, _)| path)
        .collect();
    if sources.is_empty() {
        return false;
    }
    let state = load_state(&out.join("state.json"));
    sources.iter().any(|path| {
        let (key, stamp) = import_stamp(path);
        state.0.get(&key) != Some(&stamp)
    })
}

fn imports_file(dir: &Path, name: &str) -> PathBuf {
    dir.join(crate::importarchive::DIR).join(name)
}

fn refresh_imports(
    wows: &[PathBuf],
    out: &Path,
    state: &mut State,
    touched: &[String],
    own: &[String],
) {
    let games: Vec<PathBuf> = wows.iter().map(|w| crate::gamedir::game_dir(w)).collect();
    let mut sources_changed = false;
    for game in &games {
        for (path, _) in import::find_files(game) {
            let (key, stamp) = import_stamp(&path);
            if state.0.get(&key) != Some(&stamp) {
                sources_changed = true;
                state.0.insert(key, stamp);
            }
        }
    }
    let needs_seed = own.iter().any(|name| {
        let dir = out.join(name);
        imports_file(&dir, "view.json").is_file() && !imports_file(&dir, "archive.tsv").is_file()
    });
    let (mut by, notes) = if sources_changed || needs_seed {
        import::read_all(&games, own)
    } else {
        (BTreeMap::new(), Vec::new())
    };
    for note in notes {
        eprintln!("scarlet: import: {note}");
    }
    let today = chrono::Local::now().date_naive();
    for name in own {
        let dir = out.join(name);
        if !dir.join("payloads").is_dir() {
            continue;
        }
        let mut archive = match Archive::load(&dir) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("scarlet: {name}: import: {e}");
                continue;
            }
        };
        let added = archive.merge(by.remove(name).unwrap_or_default());
        let redo = sources_changed
            || touched.contains(name)
            || added > 0
            || (!archive.is_empty() && !imports_file(&dir, "view.json").is_file());
        if !redo {
            continue;
        }
        if added > 0 {
            if let Err(e) = archive.save(&dir) {
                eprintln!("scarlet: {name}: import: {e}");
                continue;
            }
            println!("{name}: {added} imported records archived");
        }
        let evs = match load_events(&dir, name) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("scarlet: {name}: import: {e}");
                continue;
            }
        };
        let built = import::build(&archive, &evs, today);
        if let Err(e) = import::write(&dir, name, &built) {
            eprintln!("scarlet: {name}: import: {e}");
            continue;
        }
        if !built.kept.is_empty() || built.view.dropped != import::Dropped::default() {
            println!(
                "{name}: imported {} sales, {} purchases, {} gold points ({} already recorded)",
                built.view.sales,
                built.view.buys,
                built.view.gold.len(),
                built.view.dropped.sales
                    + built.view.dropped.buys
                    + built.view.dropped.other
                    + built.view.dropped.gold
            );
        }
        if !evs.is_empty()
            && let Err(e) = write_overview(&dir, &build_overview(name, &evs, own))
        {
            eprintln!("scarlet: {name}: overview: {e}");
        }
    }
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

fn save_accounts(wows: &[PathBuf], out: &Path) -> BTreeMap<String, Vec<String>> {
    let path = out.join("accounts.json");
    let mut known: BTreeMap<String, Vec<String>> = fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let before = known.clone();
    let mut fresh: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for account in wows.iter().flat_map(|w| accounts(w)) {
        let names = fresh.entry(account.name).or_default();
        for c in account.characters {
            if !names.contains(&c) {
                names.push(c);
            }
        }
    }
    known.extend(fresh);
    for names in known.values_mut() {
        names.sort();
    }
    if known != before
        && let Ok(text) = serde_json::to_string_pretty(&known)
        && let Err(e) = fs::write(
            &path,
            text + "
",
        )
    {
        eprintln!("scarlet: {}: {e}", path.display());
    }
    known
}

const DATA_ADDON: &str = "ScarletData";
const DATA_FILES: [&str; 4] = ["Digest.lua", "Items.lua", "Recaps.lua", "Feats.lua"];
const FALLBACK_INTERFACE: &str = "16001";

fn write_digest(wow: &Path, out: &Path, accounts: &BTreeMap<String, Vec<String>>) {
    let addons = crate::gamedir::game_dir(wow)
        .join("Interface")
        .join("AddOns");
    if !addons.is_dir() {
        println!("digest: no AddOns folder at {}", addons.display());
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
    let own = roster(out);
    let rebuild = overview_rev_stale(out);
    let mut chars: Vec<(String, Overview)> = Vec::new();
    let mut recaps: Vec<(String, Vec<serde_json::Value>)> = Vec::new();
    let mut saved: BTreeMap<String, featscore::Saved> = BTreeMap::new();
    for d in dirs {
        let Some(name) = d.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        let file = d.join("overview.json");
        if rebuild
            || !file.is_file()
            || !d.join("inventory.json").is_file()
            || !d.join("recaps.json").is_file()
            || !d.join("feats.json").is_file()
            || overview_is_stale(&d, &file)
        {
            match load_events(&d, &name) {
                Ok(evs) if !evs.is_empty() => {
                    if let Err(e) = write_overview(&d, &build_overview(&name, &evs, &own)) {
                        eprintln!("scarlet: {name}: overview: {e}");
                    }
                    write_inventory(&d, &name, &evs);
                    write_recaps(&d, &name, &evs);
                }
                _ => continue,
            }
        }
        let ov = fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str::<Overview>(&t).ok());
        match ov {
            Some(ov) if !ov.sessions.is_empty() => {
                recaps.push((name.clone(), read_recaps(&d)));
                if let Some(s) = read_feats(&d) {
                    saved.insert(name.clone(), s);
                }
                chars.push((name, ov));
            }
            Some(_) => {}
            None => eprintln!("scarlet: {name}: overview.json unreadable"),
        }
    }
    if chars.is_empty() {
        println!("digest: no characters with sessions; Digest.lua not written");
        return;
    }
    let accounts: BTreeMap<String, Vec<String>> = accounts
        .iter()
        .filter_map(|(folder, names)| {
            let kept: Vec<String> = names
                .iter()
                .filter(|n| chars.iter().any(|(c, _)| c == *n))
                .cloned()
                .collect();
            (!kept.is_empty()).then(|| (folder.clone(), kept))
        })
        .collect();
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
    let data = addons.join(DATA_ADDON);
    if !prepare_data_dir(&data) {
        return;
    }
    write_items(&data, &chars, out, made);
    write_recap_file(&data, &recaps, made);
    write_feats_file(&data, wow, &chars, saved, made);
    refresh_web_sync(out);
    let text = overview::digest_lua_with(&chars, made, &accounts);
    let dest = data.join("Digest.lua");
    match write_atomic(&dest, &text) {
        Ok(()) => println!(
            "digest: {} character(s), {} bytes -> {}",
            chars.len(),
            text.len(),
            dest.display()
        ),
        Err(e) => println!(
            "digest: could not write {}: {e}; the next export tries again",
            dest.display()
        ),
    }
    if DATA_FILES.iter().all(|f| data.join(f).is_file()) {
        let toc = data.join(format!("{DATA_ADDON}.toc"));
        let text = data_toc(&addons);
        if fs::read_to_string(&toc).ok().as_deref() != Some(text.as_str())
            && let Err(e) = write_atomic(&toc, &text)
        {
            println!("digest: could not write {}: {e}", toc.display());
            return;
        }
        remove_old_data(&addons.join("Scarlet"));
    }
}

fn data_addon_current(addons: &Path, data: &Path) -> bool {
    DATA_FILES.iter().all(|f| data.join(f).is_file())
        && fs::read_to_string(data.join(format!("{DATA_ADDON}.toc"))).ok() == Some(data_toc(addons))
}

fn data_addon_stale(wow: &Path) -> bool {
    let addons = crate::gamedir::game_dir(wow)
        .join("Interface")
        .join("AddOns");
    let data = addons.join(DATA_ADDON);
    addons.join("Scarlet").is_dir()
        && !data_addon_current(&addons, &data)
        && fs::symlink_metadata(&data).map_or(true, |m| m.is_dir() && !is_link(&m))
        && !data.join(".git").exists()
}

fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0 || meta.file_type().is_symlink()
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

fn prepare_data_dir(data: &Path) -> bool {
    match fs::symlink_metadata(data) {
        Ok(meta) if is_link(&meta) || !meta.is_dir() || data.join(".git").exists() => {
            println!(
                "digest: {} is a link, a file or a checkout; left alone",
                data.display()
            );
            false
        }
        Ok(_) => true,
        Err(_) => match fs::create_dir_all(data) {
            Ok(()) => true,
            Err(e) => {
                println!("digest: could not create {}: {e}", data.display());
                false
            }
        },
    }
}

fn data_toc(addons: &Path) -> String {
    let interface = fs::read_to_string(addons.join("Scarlet").join("Scarlet.toc"))
        .ok()
        .and_then(|t| {
            t.lines().find_map(|l| {
                let v = l.trim().strip_prefix("## Interface:")?.trim();
                (!v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == ','))
                    .then(|| v.to_string())
            })
        })
        .unwrap_or_else(|| FALLBACK_INTERFACE.to_string());
    format!(
        "## Interface: {interface}\n## Title: Scarlet Data\n## Notes: History and item lists the \
         Scarlet desktop app writes for the Scarlet addon.\n## Author: Medalink\n\n{}\n",
        DATA_FILES.join("\n")
    )
}

fn remove_old_data(scarlet: &Path) {
    let toc = fs::read_to_string(scarlet.join("Scarlet.toc")).ok();
    for file in DATA_FILES {
        let listed = toc
            .as_deref()
            .is_none_or(|t| t.lines().any(|l| l.trim().eq_ignore_ascii_case(file)));
        for (name, keep) in [(file.to_string(), listed), (format!("{file}.tmp"), false)] {
            let path = scarlet.join(&name);
            if !keep
                && fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && !is_link(&m))
                && fs::remove_file(&path).is_ok()
            {
                println!("digest: removed old {}", path.display());
            }
        }
    }
}

fn write_atomic(dest: &Path, text: &str) -> std::io::Result<()> {
    let mut tmp = dest.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text)?;
    fs::rename(&tmp, dest).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn write_recap_file(data: &Path, recaps: &[(String, Vec<serde_json::Value>)], made: i64) {
    let text = recap::recaps_lua(recaps, made);
    let dest = data.join("Recaps.lua");
    match write_atomic(&dest, &text) {
        Ok(()) => println!("recaps: {} bytes -> {}", text.len(), dest.display()),
        Err(e) => println!("recaps: could not write {}: {e}", dest.display()),
    }
}

fn write_feats_file(
    data: &Path,
    wow: &Path,
    chars: &[(String, Overview)],
    mut saved: BTreeMap<String, featscore::Saved>,
    made: i64,
) {
    let mut kept = read_kept(wow);
    let rows: Vec<(String, featscore::Saved, featscore::Kept)> = chars
        .iter()
        .filter_map(|(name, _)| {
            Some((
                name.clone(),
                saved.remove(name)?,
                kept.remove(name).unwrap_or_default(),
            ))
        })
        .collect();
    let text = featscore::feats_lua(&rows, made);
    let dest = data.join("Feats.lua");
    match write_atomic(&dest, &text) {
        Ok(()) => println!("feats: {} bytes -> {}", text.len(), dest.display()),
        Err(e) => println!("feats: could not write {}: {e}", dest.display()),
    }
}

fn write_items(data: &Path, chars: &[(String, Overview)], out: &Path, made: i64) {
    let list: Vec<(String, Inventory)> = chars
        .iter()
        .filter_map(|(name, _)| {
            let text = fs::read_to_string(out.join(name).join("inventory.json")).ok()?;
            Some((name.clone(), serde_json::from_str::<Inventory>(&text).ok()?))
        })
        .collect();
    let text = inventory::items_lua(&list, made);
    let dest = data.join("Items.lua");
    if let Err(e) = write_atomic(&dest, &text) {
        println!("items: could not write {}: {e}", dest.display());
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
