//! Self-update from the GitHub releases of Medalink/scarlet-app.
//!
//! Two tracks, both read from a `latest.json` that `scripts/release.ps1` publishes:
//! - Release: the asset of the latest release (`releases/latest/download/latest.json`).
//! - Beta: the asset of the fixed `beta-channel` release, which the script replaces on every
//!   publish with the newer of the newest beta and the latest release.
//!
//! The plugin downloads the installer and checks its minisign signature against the key in
//! `tauri.conf.json`; this module only picks the endpoint at run time, decides whether the
//! feed's version is newer (never a downgrade) and shows the notes. A build whose version is
//! the placeholder `0.0.0-dev` (a plain `cargo build`) reads the feed to show the latest
//! versions, but never offers the installer or syncs the addon, unless `SCARLET_UPDATE_BASE`
//! points it at a test server: `<base>/<release|beta>/latest.json` and `<base>/releases.json`.

use crate::commands::state;
use crate::settings;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

const FEEDS: &str = "https://github.com/Medalink/scarlet-app/releases";
const RELEASES_API: &str = "https://api.github.com/repos/Medalink/scarlet-app/releases?per_page=40";
/// Test servers: where the feeds and the release list come from instead of GitHub.
pub const BASE_ENV: &str = "SCARLET_UPDATE_BASE";
const RECHECK: Duration = Duration::from_secs(6 * 3600);
const NOTES_KEPT: usize = 8;
const NOTES_CACHE: Duration = Duration::from_secs(600);
const REBUILT_POLL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Release,
    Beta,
}

impl Track {
    pub fn key(self) -> &'static str {
        match self {
            Track::Release => "release",
            Track::Beta => "beta",
        }
    }

    fn parse(s: &str) -> Option<Track> {
        match s {
            "release" => Some(Track::Release),
            "beta" => Some(Track::Beta),
            _ => None,
        }
    }
}

pub fn version(app: &AppHandle) -> Version {
    app.package_info().version.clone()
}

/// The placeholder version of a build nobody released.
pub fn is_dev(v: &Version) -> bool {
    v.pre.as_str() == "dev"
}

pub fn override_base() -> Option<String> {
    std::env::var(BASE_ENV)
        .ok()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
}

/// The app offers to install a newer build of itself and keeps the addon current: released
/// builds only. Every build reads the feed.
pub fn enabled(app: &AppHandle) -> bool {
    !is_dev(&version(app)) || override_base().is_some()
}

/// The saved track; unset follows the installed build, so a beta build keeps getting betas.
pub fn track(app: &AppHandle) -> Track {
    let saved = state(app).settings.lock().unwrap().track.clone();
    saved.as_deref().and_then(Track::parse).unwrap_or_else(|| {
        if version(app).pre.as_str().starts_with("beta") {
            Track::Beta
        } else {
            Track::Release
        }
    })
}

fn feed_url(track: Track) -> Result<tauri::Url, String> {
    let text = match override_base() {
        Some(base) => format!("{base}/{}/latest.json", track.key()),
        None => match track {
            Track::Release => format!("{FEEDS}/latest/download/latest.json"),
            Track::Beta => format!("{FEEDS}/download/beta-channel/latest.json"),
        },
    };
    text.parse()
        .map_err(|e| format!("Invalid update address ({text}): {e}"))
}

/// What a feed's `addon` entry says: the zip to keep the WoW addon on. Its signature is the
/// `.sig` of the zip, made with the same key as the installer's.
#[derive(Clone, Deserialize)]
pub struct AddonOffer {
    pub version: String,
    pub url: String,
    pub signature: String,
    /// The top-level folders in the zip, when it carries more than `Scarlet`.
    #[serde(default)]
    pub folders: Vec<String>,
}

#[derive(Clone)]
pub struct FeedInfo {
    pub version: Version,
    pub addon: Option<AddonOffer>,
}

struct Shared {
    update: Option<Update>,
    feed: Option<FeedInfo>,
    notes: String,
    date: Option<String>,
    checked_ms: Option<i64>,
    error: Option<String>,
    checking: bool,
    installing: bool,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared {
    update: None,
    feed: None,
    notes: String::new(),
    date: None,
    checked_ms: None,
    error: None,
    checking: false,
    installing: false,
});

fn shared() -> std::sync::MutexGuard<'static, Shared> {
    SHARED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The feed read by the last successful check (whatever its version).
pub fn current_feed() -> Option<FeedInfo> {
    shared().feed.clone()
}

/// Notes without the lines for whoever cuts the release: install steps and commit ids.
pub fn clean_notes(body: &str) -> String {
    const DROP: [&str; 4] = ["Install:", "App commit:", "Addon commit:", "Beta of "];
    let mut out: Vec<&str> = Vec::new();
    for line in body.lines() {
        let line = line.trim_end();
        if DROP.iter().any(|p| line.starts_with(p))
            || line.starts_with("Built from the same commits")
        {
            continue;
        }
        if line.is_empty() && out.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

struct Found {
    update: Option<Update>,
    feed: Option<FeedInfo>,
    notes: String,
    date: Option<String>,
}

async fn fetch(app: &AppHandle) -> Result<Found, String> {
    let current = version(app);
    let updater = app
        .updater_builder()
        .endpoints(vec![feed_url(track(app))?])
        .map_err(|e| e.to_string())?
        // The comparison is below: it keeps the feed's version and notes when it is not newer.
        .version_comparator(|_, _| true)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let update = match updater.check().await {
        Ok(Some(u)) => u,
        // Nothing is published on this track yet (the feed is a 404 until the first release).
        Ok(None) | Err(tauri_plugin_updater::Error::ReleaseNotFound) => {
            return Ok(Found {
                update: None,
                feed: None,
                notes: String::new(),
                date: None,
            });
        }
        Err(e) => return Err(format!("Could not check for updates: {e}")),
    };
    let remote = Version::parse(&update.version).map_err(|e| {
        format!(
            "The update feed reported an invalid version ({}): {e}",
            update.version
        )
    })?;
    let addon = update
        .raw_json
        .get("addon")
        .and_then(|a| serde_json::from_value::<AddonOffer>(a.clone()).ok());
    let newer = remote > current && enabled(app);
    let notes = clean_notes(update.body.as_deref().unwrap_or(""));
    let date = update.date.map(|d| d.to_string());
    Ok(Found {
        feed: Some(FeedInfo {
            version: remote,
            addon,
        }),
        update: newer.then_some(update),
        notes,
        date,
    })
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Reads the feed of the chosen track, keeps the update it offers, then lets the addon follow it.
pub async fn refresh(app: &AppHandle) {
    {
        let mut s = shared();
        if s.checking || s.installing {
            return;
        }
        s.checking = true;
    }
    let result = fetch(app).await;
    {
        let mut s = shared();
        s.checking = false;
        s.checked_ms = Some(now_ms());
        match result {
            Ok(f) => {
                s.error = None;
                s.update = f.update;
                s.feed = f.feed;
                s.notes = f.notes;
                s.date = f.date;
            }
            Err(e) => s.error = Some(e),
        }
    }
    crate::addon_sync::run(app);
    let _ = app.emit("update-changed", info(app));
}

/// A check in the background, e.g. after the WoW folder changed.
pub fn kick(app: AppHandle) {
    std::thread::spawn(move || tauri::async_runtime::block_on(refresh(&app)));
}

/// Checks a little after start and every few hours while the app is open.
pub fn spawn(app: AppHandle) {
    let watcher = app.clone();
    std::thread::spawn(move || {
        while !rebuilt() {
            std::thread::sleep(REBUILT_POLL);
        }
        let _ = watcher.emit("update-changed", info(&watcher));
    });
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        loop {
            tauri::async_runtime::block_on(refresh(&app));
            std::thread::sleep(RECHECK);
        }
    });
}

static STARTED_AS: OnceLock<Option<(u64, SystemTime)>> = OnceLock::new();

fn exe_stamp() -> Option<(u64, SystemTime)> {
    let meta = std::fs::metadata(std::env::current_exe().ok()?).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// Records the exe this process runs, for [`rebuilt`]. Call before anything can replace it.
pub fn note_start() {
    STARTED_AS.get_or_init(exe_stamp);
}

/// True once a newer build has replaced this exe on disk, as `scripts/dev-update.ps1` does
/// to a dev copy while it runs.
pub fn rebuilt() -> bool {
    let start = STARTED_AS.get_or_init(exe_stamp);
    start.is_some() && exe_stamp().is_some_and(|now| Some(now) != *start)
}

/// Starts the exe on disk and closes this window; the new process waits for this one to exit.
#[tauri::command]
pub fn restart_rebuilt(app: AppHandle) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::DETACHED_PROCESS;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(
        std::env::args_os()
            .skip(1)
            .filter(|a| a != crate::launch::RESTARTED),
    )
    .arg(crate::launch::RESTARTED)
    .creation_flags(DETACHED_PROCESS);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    cmd.spawn()
        .map_err(|e| format!("Scarlet could not restart: {e}"))?;
    app.exit(0);
    Ok(())
}

#[derive(Serialize, Clone)]
pub struct Offered {
    version: String,
    notes: String,
    date: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct Info {
    /// The build's version; "0.0.0-dev" for a build nobody released.
    version: String,
    dev: bool,
    enabled: bool,
    track: &'static str,
    checking: bool,
    installing: bool,
    checked_ms: Option<i64>,
    error: Option<String>,
    /// The newest version on the chosen track, when it has one.
    latest: Option<String>,
    offered: Option<Offered>,
    /// On a beta of a release that is not out yet while on the Release track.
    ahead_of_track: bool,
    /// A newer build replaced this exe on disk; a restart runs it.
    rebuilt: bool,
}

pub fn info(app: &AppHandle) -> Info {
    let v = version(app);
    let track = track(app);
    let s = shared();
    let ahead = s.feed.as_ref().is_some_and(|f| f.version < v) && !is_dev(&v);
    Info {
        version: v.to_string(),
        dev: is_dev(&v),
        enabled: enabled(app),
        track: track.key(),
        checking: s.checking,
        installing: s.installing,
        checked_ms: s.checked_ms,
        error: s.error.clone(),
        latest: s.feed.as_ref().map(|f| f.version.to_string()),
        offered: s.update.as_ref().map(|u| Offered {
            version: u.version.clone(),
            notes: s.notes.clone(),
            date: s.date.clone(),
        }),
        ahead_of_track: ahead,
        rebuilt: rebuilt(),
    }
}

#[tauri::command]
pub fn update_info(app: AppHandle) -> Info {
    info(&app)
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> Info {
    refresh(&app).await;
    info(&app)
}

#[tauri::command]
pub async fn set_track(app: AppHandle, track: String) -> Result<Info, String> {
    let t = Track::parse(&track).ok_or("Update track must be release or beta.")?;
    let cur = settings::update(|c| c.track = Some(t.key().to_string()))?;
    *state(&app).settings.lock().unwrap() = cur;
    {
        // What the old track offered is not an offer on this one.
        let mut s = shared();
        s.update = None;
        s.feed = None;
        s.notes.clear();
        s.error = None;
    }
    NOTES.lock().unwrap().take();
    refresh(&app).await;
    Ok(info(&app))
}

/// Downloads the offered installer, checks its signature and runs it. On Windows the installer
/// takes over: this process exits and the new version starts.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let update = {
        let mut s = shared();
        if s.installing {
            return Err("An update is already installing.".into());
        }
        let u = s.update.clone().ok_or("There is no update to install.")?;
        s.installing = true;
        u
    };
    let _ = app.emit("update-changed", info(&app));
    let emitter = app.clone();
    let mut done = 0u64;
    let result = update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let _ = emitter.emit("update-progress", (done, total));
            },
            || {},
        )
        .await;
    // Past here the install failed: a successful one has already ended the process.
    shared().installing = false;
    let _ = app.emit("update-changed", info(&app));
    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("The update was not installed: {e}")),
    }
}

#[derive(Serialize, Clone)]
pub struct Note {
    tag: String,
    version: String,
    prerelease: bool,
    date: String,
    body: String,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

static NOTES: Mutex<Option<(Instant, Track, Vec<Note>)>> = Mutex::new(None);

pub fn http_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(concat!("scarlet-app/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn notes_from(json: &str, track: Track) -> Result<Vec<Note>, String> {
    let releases: Vec<ApiRelease> =
        serde_json::from_str(json).map_err(|e| format!("The release list is not readable: {e}"))?;
    let mut out: Vec<(Version, Note)> = Vec::new();
    for r in releases {
        // Only the releases the script made: vX.Y.Z and vX.Y.Z-beta.N, not the feed's own release.
        let Some(v) = r
            .tag_name
            .strip_prefix('v')
            .and_then(|t| Version::parse(t).ok())
        else {
            continue;
        };
        let plain = v.pre.is_empty();
        let beta = v.pre.as_str().starts_with("beta.");
        if r.draft || !(plain || beta) || (r.prerelease && track == Track::Release) {
            continue;
        }
        out.push((
            v.clone(),
            Note {
                tag: r.tag_name,
                version: v.to_string(),
                prerelease: !plain,
                date: r.published_at.unwrap_or_default(),
                body: clean_notes(r.body.as_deref().unwrap_or("")),
            },
        ));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out.into_iter().take(NOTES_KEPT).map(|(_, n)| n).collect())
}

/// The notes of the newest releases on the chosen track, from their GitHub release bodies.
#[tauri::command(async)]
pub fn release_notes(app: AppHandle) -> Result<Vec<Note>, String> {
    let track = track(&app);
    if let Some((at, t, notes)) = NOTES.lock().unwrap().as_ref()
        && *t == track
        && at.elapsed() < NOTES_CACHE
    {
        return Ok(notes.clone());
    }
    let url = match override_base() {
        Some(base) => format!("{base}/releases.json"),
        None => RELEASES_API.to_string(),
    };
    let mut resp = http_agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Could not read the release notes: {e}"))?;
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("Could not read the release notes: {e}"))?;
    let notes = notes_from(&text, track)?;
    *NOTES.lock().unwrap() = Some((Instant::now(), track, notes.clone()));
    Ok(notes)
}
