//! Compress actions: export, logs, sizes, watch thread, settings I/O.

use crate::commands::{blocking, reload, state};
use crate::session_groups::{Mark, SessionMode};
use crate::settings::{self, Settings};
use crate::watch::{WATCH_ON, set_watch, wake};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::AppHandle;

#[derive(Serialize)]
pub struct Sizes {
    sv_files: u64,
    sv_bytes: u64,
    payload_bytes: u64,
    ratio: f64,
    watch: bool,
}

#[tauri::command]
pub async fn sizes(app: AppHandle) -> Sizes {
    blocking(move || {
        let s = state(&app);
        let (wows, dir) = {
            let st = s.settings.lock().unwrap();
            (st.folders(), st.data.clone())
        };
        let sv = scarlet::export::find_sv_all(&wows).unwrap_or_default();
        let mut sv_bytes = 0u64;
        for p in &sv {
            sv_bytes += std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        }
        let mut payload_bytes = 0u64;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.filter_map(|e| e.ok()) {
                let pl = e.path().join("payloads");
                if let Ok(files) = std::fs::read_dir(&pl) {
                    for f in files.filter_map(|f| f.ok()) {
                        payload_bytes += f.metadata().map(|m| m.len()).unwrap_or(0);
                    }
                }
            }
        }
        let ratio = if payload_bytes > 0 {
            sv_bytes as f64 / payload_bytes as f64
        } else {
            0.0
        };
        Sizes {
            sv_files: sv.len() as u64,
            sv_bytes,
            payload_bytes,
            ratio,
            watch: WATCH_ON.load(Ordering::Relaxed),
        }
    })
    .await
}

fn run_export(app: &AppHandle) -> String {
    let s = state(app);
    let (wows, out) = {
        let st = s.settings.lock().unwrap();
        (st.folders(), st.data.clone())
    };
    if !wows
        .iter()
        .any(|w| scarlet::gamedir::game_dir(w).join("WTF").is_dir())
    {
        return "Export skipped: the WoW folder in Settings has no WoW Forever install.".into();
    }
    let before = std::time::Instant::now();
    match crate::watch::export_and_reload(app, &wows, &out) {
        Ok(_) => format!("Exported in {:.1}s.", before.elapsed().as_secs_f64()),
        Err(e) => format!("Export failed: {e}"),
    }
}

#[tauri::command]
pub async fn compress_export(app: AppHandle) -> String {
    blocking(move || run_export(&app)).await
}

#[tauri::command]
pub async fn compress_logs(app: AppHandle) -> String {
    blocking(move || {
        let s = state(&app);
        let (wows, out) = {
            let st = s.settings.lock().unwrap();
            (st.folders(), st.data.clone())
        };
        match scarlet::export::logs_all(&wows, &out) {
            Ok(()) => "Combat logs compressed.".into(),
            Err(e) => format!("Could not compress combat logs: {e}"),
        }
    })
    .await
}

/// Turn "Watch SavedVariables" on or off: the switch and the `watch` setting, so a player
/// whose folder was not found by itself keeps exporting after they pick one in setup.
#[tauri::command(async)]
pub fn toggle_watch(app: AppHandle, on: bool) -> Result<bool, String> {
    set_watch(&app, on);
    let watch = WATCH_ON.load(Ordering::Relaxed);
    let cur = settings::update(|c| c.watch = watch)?;
    *state(&app).settings.lock().unwrap() = cur;
    Ok(watch)
}

/// Record that the first-run setup has finished (or was declined), so it does not open again.
#[tauri::command(async)]
pub fn set_setup_done(app: AppHandle, done: bool) -> Result<(), String> {
    let cur = settings::update(|c| c.setup_done = done)?;
    *state(&app).settings.lock().unwrap() = cur;
    Ok(())
}

/// The settings, where they are stored and what is missing on this PC.
#[derive(Serialize)]
pub struct SettingsView {
    #[serde(flatten)]
    settings: Settings,
    file: String,
    /// True for a copy that must keep its data off C: (the owner's dev copy).
    keep_off_c: bool,
    problems: Vec<String>,
}

#[tauri::command(async)]
pub fn get_settings() -> SettingsView {
    let settings = settings::load();
    SettingsView {
        problems: settings::problems(&settings),
        file: settings::path().display().to_string(),
        keep_off_c: settings::portable(),
        settings,
    }
}

#[tauri::command(async)]
pub fn set_settings(
    app: AppHandle,
    wow: String,
    data: String,
    more_wow: Option<Vec<String>>,
) -> Result<Settings, String> {
    let wow_p = PathBuf::from(wow.trim());
    let data_p = PathBuf::from(data.trim());
    if data_p.as_os_str().is_empty() {
        return Err("Enter a data folder.".into());
    }
    if settings::portable() && !settings::not_on_c(&data_p) {
        return Err("The data folder must not be on C:.".into());
    }
    let cur = settings::update(|c| {
        c.wow = wow_p;
        c.data = data_p;
        // Left as it is when the caller sends none (the Browse button changes only `wow`).
        if let Some(more) = more_wow {
            c.more_wow = more
                .iter()
                .map(|p| PathBuf::from(p.trim()))
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
        }
    })?;
    *state(&app).settings.lock().unwrap() = cur.clone();
    reload(&app);
    // The watch thread reads the new folders.
    wake();
    // The addon may now be missing from the new folder.
    crate::updater::kick(app.clone());
    Ok(cur)
}

/// Turn "Open when WoW Forever starts" on or off: the setting, the HKCU Run value and
/// the watcher. A running watcher sees the setting off and exits within a few seconds.
#[tauri::command(async)]
pub fn set_open_with_wow(app: AppHandle, on: bool) -> Result<bool, String> {
    // The setting first: a window starting with it on rewrites the Run value, so a
    // registry change the setting does not match would not stick.
    settings::update(|c| c.open_with_wow = on)?;
    if let Err(e) = crate::launch::set_registered(on) {
        let _ = settings::update(|c| c.open_with_wow = !on);
        return Err(e);
    }
    state(&app).settings.lock().unwrap().open_with_wow = on;
    if on {
        crate::launch::spawn_watcher();
        // An off-then-on click can meet the old watcher still exiting; a second copy
        // a moment later covers it (extra copies exit at once).
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(2));
            crate::launch::spawn_watcher();
        });
    }
    Ok(on)
}

/// Turn "Group characters by game account" on or off: the setting, the Characters page and the
/// digest the addon reads.
#[tauri::command(async)]
pub fn set_group_by_account(app: AppHandle, on: bool) -> Result<bool, String> {
    let cur = settings::update(|c| c.group_by_account = on)?;
    *state(&app).settings.lock().unwrap() = cur.clone();
    scarlet::export::set_group_by_account(&cur.folders(), &cur.data, on);
    Ok(on)
}

#[derive(Serialize)]
pub struct SessionPrefs {
    mode: SessionMode,
    marks: Vec<Mark>,
}

fn store_prefs(app: &AppHandle, cur: Settings) -> SessionPrefs {
    let prefs = SessionPrefs {
        mode: cur.session_mode,
        marks: cur.session_marks.clone(),
    };
    *state(app).settings.lock().unwrap() = cur;
    prefs
}

/// What the sessions list calls a session: `auto` (a WoW run), `day` or `manual`.
#[tauri::command(async)]
pub fn set_session_mode(app: AppHandle, mode: SessionMode) -> Result<SessionPrefs, String> {
    let cur = settings::update(|c| c.session_mode = mode)?;
    Ok(store_prefs(&app, cur))
}

/// Starts a manual session now; does nothing while one is open.
#[tauri::command(async)]
pub fn session_start(app: AppHandle) -> Result<SessionPrefs, String> {
    let now = crate::run::now_ms();
    let cur = settings::update(|c| {
        if c.session_marks.iter().all(|m| m.to.is_some()) {
            c.session_marks.push(Mark {
                from: now,
                to: None,
            });
        }
    })?;
    Ok(store_prefs(&app, cur))
}

/// Ends the open manual session; does nothing when none is open.
#[tauri::command(async)]
pub fn session_stop(app: AppHandle) -> Result<SessionPrefs, String> {
    let now = crate::run::now_ms();
    let cur = settings::update(|c| {
        for m in c.session_marks.iter_mut().filter(|m| m.to.is_none()) {
            m.to = Some(now.max(m.from));
        }
    })?;
    Ok(store_prefs(&app, cur))
}

fn view_to_store(view: &str) -> String {
    let (key, sub) = match view.split_once('/') {
        Some((key, sub)) => (key, sub),
        None => return view.to_string(),
    };
    let digits = |v: &str| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit());
    match sub.split_once('-') {
        Some((from, to)) if key == "sessions" && digits(from) && digits(to) => key.to_string(),
        _ => view.to_string(),
    }
}

/// Persist the last view so the app reopens on it.
#[tauri::command(async)]
pub fn save_view(app: AppHandle, view: String) {
    let view = view_to_store(&view);
    {
        let mut st = state(&app).settings.lock().unwrap();
        // Re-rendering the same view writes nothing.
        if st.last_view == view {
            return;
        }
        st.last_view = view.clone();
    }
    // The file as written, not `update`: a missing or half-written file stays as it is
    // instead of being replaced by defaults.
    if let Some(mut s) = settings::try_load() {
        s.last_view = view;
        let _ = settings::save(&s);
    }
}

/// The number of WoW Forever clients running now, in every folder, for the sidebar.
#[tauri::command(async)]
pub fn wow_clients(app: AppHandle) -> usize {
    let folders = state(&app).settings.lock().unwrap().folders();
    crate::launch::running_clients(&folders).iter().sum()
}

#[derive(Serialize)]
pub struct AccountView {
    name: String,
    /// Epoch seconds the account's Scarlet file was last written, 0 for never.
    saved: i64,
    characters: Vec<String>,
}

#[derive(Serialize)]
pub struct FolderView {
    path: String,
    primary: bool,
    /// Clients running from this folder.
    running: usize,
    problem: Option<String>,
    /// The Scarlet addon folders in this WoW folder: version, or not installed.
    addon: Vec<crate::addon_sync::FolderStatus>,
    accounts: Vec<AccountView>,
}

/// Every WoW folder with its game accounts and their characters, and the clients running from
/// it: what Settings shows so a player with several boxes sees them all in one app.
#[tauri::command(async)]
pub fn wow_folders(app: AppHandle) -> Vec<FolderView> {
    let (primary, more) = {
        let st = state(&app).settings.lock().unwrap();
        (st.wow.clone(), st.more_wow.clone())
    };
    let mut all: Vec<PathBuf> = Vec::new();
    for p in std::iter::once(primary.clone()).chain(more) {
        if !p.as_os_str().is_empty()
            && !all
                .iter()
                .any(|q| settings::folder_key(q) == settings::folder_key(&p))
        {
            all.push(p);
        }
    }
    let running = crate::launch::running_clients(&all);
    all.iter()
        .zip(running)
        .map(|(wow, running)| FolderView {
            path: wow.display().to_string(),
            primary: settings::folder_key(wow) == settings::folder_key(&primary),
            running,
            problem: settings::folder_problem(wow),
            addon: crate::addon_sync::folder_addons(wow),
            accounts: scarlet::export::accounts(wow)
                .into_iter()
                .map(|a| AccountView {
                    name: a.name,
                    saved: a.saved,
                    characters: a.characters,
                })
                .collect(),
        })
        .collect()
}
