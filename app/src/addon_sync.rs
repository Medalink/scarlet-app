//! Keeps the Scarlet addon current in every WoW folder (the main one and each of `more_wow`)
//! from the addon zip of the update feed. One download serves them all; each folder is judged
//! on its own by the rules below.
//!
//! The app writes into `<WoW>\_forever_\Interface\AddOns\<folder>` only when the folder is
//! missing (it installs it) or the app installed it itself (it holds `.scarlet-app.json`, which
//! lists the files the app wrote). It leaves alone, and says so in Settings:
//! - a folder without that file (installed by hand),
//! - a TOC with `X-Curse-Project-ID` (CurseForge) or `X-Wago-ID` (Wago): their app owns it,
//! - a folder holding `.git` (a checkout),
//! - a junction or symlink, the folder itself or anything inside it (a dev checkout).
//!
//! Settings > Addon has Install, Update and Reinstall buttons (`addon_install`) for the first
//! two kinds above and for a folder installed by hand, which the player confirms first because
//! the app manages it from then on. They write the same way and ignore the "keep current" switch.
//!
//! An update writes the zip's files and removes only files the previous marker listed and the
//! zip no longer has. `Digest.lua`, which the companion once wrote here, is in neither, so it
//! stays until the companion removes it. The zip is checked against the updater key (minisign)
//! before a byte of it is written.
//!
//! `AddOns\ScarletData` belongs to the companion (`export.rs`): it holds the digest and the item
//! lists the export writes. It is never offered, installed, updated, removed or listed here, and
//! a zip entry for it is dropped.

use crate::commands::state;
use crate::settings;
use crate::updater::{self, AddonOffer};
use base64::Engine;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

pub const MARKER: &str = ".scarlet-app.json";
const MANAGER: &str = "scarlet-app";
/// The folder the zip carries today; more come from the feed's `addon.folders`.
const MAIN: &str = "Scarlet";
const DATA: &str = "ScarletData";
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const MAX_ZIP: u64 = 16 << 20;
const MAX_UNPACKED: u64 = 64 << 20;
const MAX_ENTRIES: usize = 4000;
const MAX_WALK: usize = 20_000;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
struct Marker {
    managed_by: String,
    version: String,
    /// Files written by the app, relative to the folder, with `/` separators.
    files: Vec<String>,
}

/// Who looks after a folder.
#[derive(Debug, PartialEq, Eq)]
enum Class {
    Missing,
    /// Installed by this app.
    Ours(Marker),
    ByHand,
    Curse,
    Wago,
    Git,
    Linked,
}

fn is_reparse(meta: &fs::Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || meta.file_type().is_symlink()
}

/// What a walk of a folder found.
#[derive(Default)]
struct Walk {
    linked: bool,
    git: bool,
}

fn walk(dir: &Path, depth: usize, seen: &mut usize, out: &mut Walk) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        *seen += 1;
        if *seen > MAX_WALK {
            // Too big to be an addon: treat it as not ours to rewrite.
            out.linked = true;
            return;
        }
        let Ok(meta) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        if is_reparse(&meta) {
            out.linked = true;
            continue;
        }
        if e.file_name().eq_ignore_ascii_case(".git") {
            out.git = true;
        }
        if meta.is_dir() && depth < 12 {
            walk(&e.path(), depth + 1, seen, out);
        }
    }
}

/// The value of a `## Key:` line in a TOC, case-insensitively.
fn toc_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("##") else {
            continue;
        };
        let Some((k, v)) = rest.split_once(':') else {
            continue;
        };
        if k.trim().eq_ignore_ascii_case(key) {
            return Some(v.trim().to_string());
        }
    }
    None
}

fn tocs(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("toc"))
        })
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .collect()
}

fn read_marker(dir: &Path) -> Option<Marker> {
    let text = fs::read_to_string(dir.join(MARKER)).ok()?;
    let m: Marker = serde_json::from_str(&text).ok()?;
    (m.managed_by == MANAGER).then_some(m)
}

fn classify(dir: &Path) -> Class {
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return Class::Missing;
    };
    if is_reparse(&meta) {
        return Class::Linked;
    }
    if !meta.is_dir() {
        // A file where the folder belongs: not ours to replace.
        return Class::ByHand;
    }
    // An empty folder holds nothing of anyone's.
    if fs::read_dir(dir).is_ok_and(|mut e| e.next().is_none()) {
        return Class::Missing;
    }
    let mut found = Walk::default();
    walk(dir, 0, &mut 0, &mut found);
    if found.linked {
        return Class::Linked;
    }
    if found.git {
        return Class::Git;
    }
    for toc in tocs(dir) {
        if toc_value(&toc, "X-Curse-Project-ID").is_some_and(|v| !v.is_empty()) {
            return Class::Curse;
        }
        if toc_value(&toc, "X-Wago-ID").is_some_and(|v| !v.is_empty()) {
            return Class::Wago;
        }
    }
    match read_marker(dir) {
        Some(m) => Class::Ours(m),
        None => Class::ByHand,
    }
}

/// The version a folder reports: what the app recorded, else its TOC's `## Version:`.
fn installed_version(dir: &Path, class: &Class) -> Option<String> {
    if let Class::Ours(m) = class
        && m.version != "0.0.0"
    {
        return Some(m.version.clone());
    }
    let name = dir.file_name()?.to_string_lossy().to_string();
    let toc = fs::read_to_string(dir.join(format!("{name}.toc"))).ok()?;
    toc_value(&toc, "Version").filter(|v| !v.is_empty())
}

fn addons_dir(wow: &Path) -> PathBuf {
    scarlet::gamedir::game_dir(wow)
        .join("Interface")
        .join("AddOns")
}

// ---- the zip -----------------------------------------------------------------------------------

struct Packed {
    /// (folder, path inside it with `/`, bytes)
    files: Vec<(String, String, Vec<u8>)>,
}

fn pubkey(app: &AppHandle) -> Result<String, String> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "The app has no update key.".to_string())
}

fn unbase64(text: &str) -> Result<String, String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|e| e.to_string())?;
    String::from_utf8(raw).map_err(|e| e.to_string())
}

/// Checks the zip against the updater key, and that the signature is for this very file name
/// (its trusted comment is signed, so a feed cannot pair an old zip with a new version).
fn verify(app: &AppHandle, zip: &[u8], offer: &AddonOffer) -> Result<(), String> {
    let key = minisign_verify::PublicKey::decode(&unbase64(&pubkey(app)?)?)
        .map_err(|e| format!("The update key is not readable: {e}"))?;
    let sig = minisign_verify::Signature::decode(&unbase64(&offer.signature)?)
        .map_err(|e| format!("The addon signature is not readable: {e}"))?;
    key.verify(zip, &sig, true)
        .map_err(|e| format!("The addon zip does not match its signature: {e}"))?;
    let want = format!("file:Scarlet-{}.zip", offer.version);
    if !sig
        .trusted_comment()
        .split('\t')
        .any(|part| part == want.as_str())
    {
        return Err(format!(
            "The addon signature is not for Scarlet-{}.zip.",
            offer.version
        ));
    }
    Ok(())
}

fn folder_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn unpack(zip: &[u8]) -> Result<Packed, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip))
        .map_err(|e| format!("The addon zip is not readable: {e}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err("The addon zip has too many files.".into());
    }
    let mut files = Vec::new();
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let shown = String::from_utf8_lossy(entry.name_raw()).to_string();
        let Some(path) = entry.enclosed_name() else {
            return Err(format!("The addon zip has an unsafe path: {}.", shown));
        };
        let mut parts: Vec<String> = Vec::new();
        for c in path.components() {
            match c {
                Component::Normal(p) => parts.push(p.to_string_lossy().to_string()),
                _ => {
                    return Err(format!("The addon zip has an unsafe path: {}.", shown));
                }
            }
        }
        if parts.first().is_some_and(|f| f.eq_ignore_ascii_case(DATA)) {
            continue;
        }
        if parts.len() < 2 || !folder_ok(&parts[0]) || parts.iter().any(|p| p.contains(':')) {
            return Err(format!(
                "The addon zip has {} outside an addon folder.",
                shown
            ));
        }
        let rel = parts[1..].join("/");
        // The app's own bookkeeping and the companion's file are never shipped or replaced.
        if rel.eq_ignore_ascii_case(MARKER) || rel.eq_ignore_ascii_case("Digest.lua") {
            continue;
        }
        total += entry.size();
        if total > MAX_UNPACKED {
            return Err("The addon zip is too large.".into());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        files.push((parts[0].clone(), rel, bytes));
    }
    if files.is_empty() {
        return Err("The addon zip has no files.".into());
    }
    Ok(Packed { files })
}

fn download(
    offer: &AddonOffer,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<Vec<u8>, String> {
    let failed = |e: &dyn std::fmt::Display| format!("Could not download the addon: {e}");
    let mut resp = updater::http_agent()
        .get(&offer.url)
        .call()
        .map_err(|e| failed(&e))?;
    let total = resp.body().content_length();
    let mut reader = resp.body_mut().with_config().limit(MAX_ZIP).reader();
    let mut out = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| failed(&e))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
        progress(out.len() as u64, total);
    }
    Ok(out)
}

// ---- writing -----------------------------------------------------------------------------------

fn write_marker(dir: &Path, version: &str, files: &[String]) -> Result<(), String> {
    let m = Marker {
        managed_by: MANAGER.into(),
        version: version.into(),
        files: files.to_vec(),
    };
    let text = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())? + "\n";
    fs::write(dir.join(MARKER), text).map_err(|e| e.to_string())
}

/// Writes one file next to its place and renames it over, so a reader never sees half of it.
fn write_file(dir: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    let dest = rel.split('/').fold(dir.to_path_buf(), |p, s| p.join(s));
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Ok(meta) = fs::symlink_metadata(&dest)
        && (is_reparse(&meta) || meta.is_dir())
    {
        return Err(format!("{rel} is a link or a folder; left alone."));
    }
    let tmp = dest.with_extension("scarlet-new");
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Installs or updates one folder from the zip's files. The marker goes first with version
/// 0.0.0, so an interrupted run is still ours and finishes on the next one.
fn apply(
    dir: &Path,
    version: &str,
    files: &[(&str, &[u8])],
    old: Option<&Marker>,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let new_rel: Vec<String> = files.iter().map(|(r, _)| (*r).to_string()).collect();
    let mut listed: BTreeSet<String> = new_rel.iter().cloned().collect();
    if let Some(m) = old {
        listed.extend(m.files.iter().cloned());
    }
    let listed: Vec<String> = listed.into_iter().collect();
    if old.is_none() {
        write_marker(dir, "0.0.0", &listed)?;
    }
    for (rel, bytes) in files {
        write_file(dir, rel, bytes)?;
    }
    if let Some(m) = old {
        for rel in &m.files {
            if new_rel.contains(rel)
                || rel.eq_ignore_ascii_case("Digest.lua")
                || rel
                    .split('/')
                    .any(|p| p == ".." || p.is_empty() || p.contains(':'))
            {
                continue;
            }
            let path = rel.split('/').fold(dir.to_path_buf(), |p, s| p.join(s));
            if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && !is_reparse(&m)) {
                let _ = fs::remove_file(&path);
                // Empty folders the old version had go too, up to the addon folder.
                let mut parent = path.parent().map(Path::to_path_buf);
                while let Some(p) = parent {
                    if p == dir || fs::remove_dir(&p).is_err() {
                        break;
                    }
                    parent = p.parent().map(Path::to_path_buf);
                }
            }
        }
    }
    write_marker(dir, version, &new_rel)
}

// ---- running -----------------------------------------------------------------------------------

static LAST: Mutex<Option<Last>> = Mutex::new(None);
static RUNNING: Mutex<()> = Mutex::new(());

#[derive(Clone)]
struct Last {
    ms: i64,
    text: String,
    error: bool,
}

fn say(text: String, error: bool) {
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Last {
        ms: chrono::Utc::now().timestamp_millis(),
        text,
        error,
    });
}

fn offered_folders(offer: &AddonOffer) -> Vec<String> {
    let mut v = vec![MAIN.to_string()];
    for f in &offer.folders {
        if folder_ok(f) && !f.eq_ignore_ascii_case(DATA) && !v.contains(f) {
            v.push(f.clone());
        }
    }
    v
}

/// True when the offer would change this folder.
fn wants(class: &Class, dir: &Path, offer: &Version) -> bool {
    match class {
        Class::Missing => true,
        Class::Ours(m) => {
            let have = installed_version(dir, class).unwrap_or_else(|| m.version.clone());
            Version::parse(&have).map_or(true, |h| *offer > h)
        }
        _ => false,
    }
}

/// Brings the addon in line with the feed the last check read. Never fails the caller: the
/// outcome is what Settings shows.
pub fn run(app: &AppHandle) {
    let _one = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    if !updater::enabled(app) {
        return;
    }
    let (wows, primary, keep) = {
        let s = state(app).settings.lock().unwrap();
        (s.folders(), s.wow.clone(), s.keep_addon)
    };
    if !keep {
        return;
    }
    let Some(offer) = updater::current_feed().and_then(|f| f.addon) else {
        return;
    };
    let wows: Vec<PathBuf> = wows
        .into_iter()
        .filter(|w| settings::folder_problem(w).is_none())
        .collect();
    if wows.is_empty() {
        return;
    }
    let Ok(version) = Version::parse(&offer.version) else {
        say(
            format!(
                "The feed's addon version {:?} is not a version.",
                offer.version
            ),
            true,
        );
        return;
    };
    let names = offered_folders(&offer);
    let behind: Vec<&PathBuf> = wows
        .iter()
        .filter(|w| {
            let addons = addons_dir(w);
            names.iter().any(|n| {
                let dir = addons.join(n);
                wants(&classify(&dir), &dir, &version)
            })
        })
        .collect();
    if behind.is_empty() {
        return;
    }
    let packed = match download(&offer, |_, _| {}).and_then(|zip| {
        verify(app, &zip, &offer)?;
        unpack(&zip)
    }) {
        Ok(p) => p,
        Err(e) => {
            say(e, true);
            return;
        }
    };
    let mut done: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    for wow in behind {
        let whose = if settings::folder_key(wow) == settings::folder_key(&primary) {
            String::new()
        } else {
            format!(" in {}", wow.display())
        };
        match sync_folders(&addons_dir(wow), &offer.version, &version, &packed) {
            Ok(lines) => done.extend(lines.into_iter().map(|l| format!("{l}{whose}"))),
            Err(e) => failed.push(format!("{e}{whose}")),
        }
    }
    if !failed.is_empty() {
        failed.extend(done);
        say(failed.join("; "), true);
    } else if !done.is_empty() {
        say(
            format!(
                "{}. It loads the next time WoW starts or you type /reload.",
                done.join("; ")
            ),
            false,
        );
    }
}

fn sync_folders(
    addons: &Path,
    offer_text: &str,
    offer: &Version,
    packed: &Packed,
) -> Result<Vec<String>, String> {
    let mut folders: Vec<&str> = packed.files.iter().map(|(f, _, _)| f.as_str()).collect();
    folders.dedup();
    let folders: BTreeSet<&str> = folders.into_iter().collect();
    let mut done: Vec<String> = Vec::new();
    for folder in folders {
        let dir = addons.join(folder);
        let class = classify(&dir);
        if !wants(&class, &dir, offer) {
            continue;
        }
        let files: Vec<(&str, &[u8])> = packed
            .files
            .iter()
            .filter(|(f, _, _)| f == folder)
            .map(|(_, r, b)| (r.as_str(), b.as_slice()))
            .collect();
        let old = match &class {
            Class::Ours(m) => Some(m),
            _ => None,
        };
        apply(&dir, offer_text, &files, old)
            .map_err(|e| format!("Could not write {folder}: {e}"))?;
        done.push(match old {
            Some(_) => format!("{folder} updated to {offer_text}"),
            None => format!("{folder} installed at {offer_text}"),
        });
    }
    Ok(done)
}

// ---- Settings ----------------------------------------------------------------------------------

#[derive(Serialize)]
pub struct FolderStatus {
    pub name: String,
    /// missing, ours, by-hand, curseforge, wago, git, linked
    manager: &'static str,
    /// Who keeps the folder current; none when it is not installed.
    kept_by: Option<&'static str>,
    installed: Option<String>,
    /// The version on the player's track.
    latest: Option<String>,
    can_install: bool,
    can_update: bool,
    can_reinstall: bool,
    /// Installing over this folder makes Scarlet its manager, which the player confirms first.
    takeover: bool,
}

#[derive(Serialize)]
pub struct Status {
    keep: bool,
    /// Why nothing is shown: no WoW folder, or no addon folder to look in.
    problem: Option<String>,
    folders: Vec<FolderStatus>,
    /// release or beta
    track: &'static str,
    last: Option<String>,
    last_ms: Option<i64>,
    last_error: bool,
}

fn kept_by(class: &Class) -> Option<&'static str> {
    match class {
        Class::Missing => None,
        Class::Ours(_) => Some("Scarlet"),
        Class::ByHand => Some("Nobody, installed by hand"),
        Class::Curse => Some("CurseForge"),
        Class::Wago => Some("Wago"),
        Class::Git => Some("A git checkout"),
        Class::Linked => Some("A linked folder"),
    }
}

fn manager_key(class: &Class) -> &'static str {
    match class {
        Class::Missing => "missing",
        Class::Ours(_) => "ours",
        Class::ByHand => "by-hand",
        Class::Curse => "curseforge",
        Class::Wago => "wago",
        Class::Git => "git",
        Class::Linked => "linked",
    }
}

/// True when the buttons may write here: a missing folder, one Scarlet installed, or a plain
/// folder of an addon put there by hand.
fn writable(class: &Class, dir: &Path) -> bool {
    match class {
        Class::Missing | Class::Ours(_) => true,
        Class::ByHand => fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir() && !is_reparse(&m)),
        _ => false,
    }
}

fn folder_status(name: String, dir: &Path, latest: Option<&Version>) -> FolderStatus {
    let class = classify(dir);
    let installed = installed_version(dir, &class);
    let writable = latest.is_some() && writable(&class, dir);
    let missing = class == Class::Missing;
    let newer = latest.is_some_and(|l| {
        installed
            .as_deref()
            .and_then(|v| Version::parse(v).ok())
            .is_none_or(|h| *l > h)
    });
    FolderStatus {
        name,
        manager: manager_key(&class),
        kept_by: kept_by(&class),
        installed,
        latest: latest.map(Version::to_string),
        can_install: writable && missing,
        can_update: writable && !missing && newer,
        can_reinstall: writable && !missing,
        takeover: class == Class::ByHand,
    }
}

/// The addon folders of one WoW folder as the Settings folder list shows them. Empty when the
/// folder holds no game to look in.
pub fn folder_addons(wow: &Path) -> Vec<FolderStatus> {
    if settings::folder_problem(wow).is_some() {
        return Vec::new();
    }
    let offer = updater::current_feed().and_then(|f| f.addon);
    let latest = offer.as_ref().and_then(|o| Version::parse(&o.version).ok());
    let addons = addons_dir(wow);
    let names = offer
        .as_ref()
        .map_or_else(|| vec![MAIN.to_string()], offered_folders);
    names
        .into_iter()
        .map(|name| {
            let dir = addons.join(&name);
            folder_status(name, &dir, latest.as_ref())
        })
        .collect()
}

pub fn status(app: &AppHandle) -> Status {
    let (wow, keep) = {
        let s = state(app).settings.lock().unwrap();
        (s.wow.clone(), s.keep_addon)
    };
    let offer = updater::current_feed().and_then(|f| f.addon);
    let latest = offer.as_ref().and_then(|o| Version::parse(&o.version).ok());
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut out = Status {
        keep,
        problem: if wow.as_os_str().is_empty() {
            Some("Set the WoW folder first.".into())
        } else {
            settings::folder_problem(&wow)
        },
        folders: Vec::new(),
        track: updater::track(app).key(),
        last: last.as_ref().map(|l| l.text.clone()),
        last_ms: last.as_ref().map(|l| l.ms),
        last_error: last.is_some_and(|l| l.error),
    };
    if out.problem.is_some() {
        return out;
    }
    let addons = addons_dir(&wow);
    let names = offer
        .as_ref()
        .map_or_else(|| vec![MAIN.to_string()], offered_folders);
    for name in names {
        let dir = addons.join(&name);
        out.folders.push(folder_status(name, &dir, latest.as_ref()));
    }
    out
}

#[derive(Serialize, Clone)]
struct Progress<'a> {
    folder: &'a str,
    /// download or write
    stage: &'static str,
    done: u64,
    total: Option<u64>,
}

/// The Install, Update and Reinstall buttons: puts the track's zip in one folder. A folder that
/// is installed by hand needs `takeover`, which the player confirmed; after it Scarlet manages
/// the folder. Folders of CurseForge, Wago, a git checkout or a link are never written.
#[tauri::command(async)]
pub fn addon_install(app: AppHandle, folder: String, takeover: bool) -> Result<Status, String> {
    let _one = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let wow = state(&app).settings.lock().unwrap().wow.clone();
    if let Some(p) = settings::folder_problem(&wow) {
        return Err(p);
    }
    let offer = updater::current_feed()
        .and_then(|f| f.addon)
        .ok_or("There is no addon to install. Check for updates first.")?;
    Version::parse(&offer.version).map_err(|_| {
        format!(
            "The feed's addon version {:?} is not a version.",
            offer.version
        )
    })?;
    if !offered_folders(&offer).contains(&folder) {
        return Err(format!("{folder} is not part of the addon."));
    }
    let dir = addons_dir(&wow).join(&folder);
    let class = classify(&dir);
    if !writable(&class, &dir) {
        return Err(format!(
            "Scarlet leaves the {folder} folder alone: {}.",
            kept_by(&class).unwrap_or("not ours")
        ));
    }
    if class == Class::ByHand && !takeover {
        return Err(format!("{folder} was installed by hand; confirm first."));
    }
    let emit = |stage, done, total| {
        let _ = app.emit(
            "addon-progress",
            Progress {
                folder: &folder,
                stage,
                done,
                total,
            },
        );
    };
    emit("download", 0, None);
    let zip = download(&offer, |done, total| emit("download", done, total))?;
    emit("write", 0, None);
    verify(&app, &zip, &offer)?;
    let packed = unpack(&zip)?;
    let files: Vec<(&str, &[u8])> = packed
        .files
        .iter()
        .filter(|(f, _, _)| *f == folder)
        .map(|(_, r, b)| (r.as_str(), b.as_slice()))
        .collect();
    if files.is_empty() {
        return Err(format!("The addon zip has no {folder} folder."));
    }
    let old = match &class {
        Class::Ours(m) => Some(m),
        _ => None,
    };
    apply(&dir, &offer.version, &files, old)
        .map_err(|e| format!("Could not write {folder}: {e}"))?;
    Ok(status(&app))
}

#[tauri::command(async)]
pub fn addon_status(app: AppHandle) -> Status {
    status(&app)
}

/// The "Keep the addon current" switch.
#[tauri::command(async)]
pub fn set_keep_addon(app: AppHandle, on: bool) -> Result<Status, String> {
    let cur = settings::update(|c| c.keep_addon = on)?;
    *state(&app).settings.lock().unwrap() = cur;
    if on {
        run(&app);
    }
    Ok(status(&app))
}
