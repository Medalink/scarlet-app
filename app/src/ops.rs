//! Compress actions: export, logs, sizes, watch thread, settings I/O.

use crate::commands::{reload, state};
use crate::settings::{self, Settings};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Emitter};

pub static WATCH_ON: AtomicBool = AtomicBool::new(false);
/// Bumped on every switch-on; a loop from an older generation exits.
static WATCH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Serialize)]
pub struct Sizes {
    sv_files: u64,
    sv_bytes: u64,
    payload_bytes: u64,
    ratio: f64,
    watch: bool,
}

#[tauri::command]
pub fn sizes(app: AppHandle) -> Sizes {
    let s = state(&app);
    let (wow, dir) = {
        let st = s.settings.lock().unwrap();
        (st.wow.clone(), st.data.clone())
    };
    let sv = scarlet::export::find_sv(&wow).unwrap_or_default();
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
}

fn run_export(app: &AppHandle) -> String {
    let s = state(app);
    let (wow, out) = {
        let st = s.settings.lock().unwrap();
        (st.wow.clone(), st.data.clone())
    };
    let before = std::time::Instant::now();
    match scarlet::export::export(&wow, &out) {
        Ok(()) => {
            reload(app);
            format!("export ok in {:.1}s", before.elapsed().as_secs_f64())
        }
        Err(e) => format!("export failed: {e}"),
    }
}

#[tauri::command]
pub fn compress_export(app: AppHandle) -> String {
    run_export(&app)
}

#[tauri::command]
pub fn compress_logs(app: AppHandle) -> String {
    let s = state(&app);
    let (wow, out) = {
        let st = s.settings.lock().unwrap();
        (st.wow.clone(), st.data.clone())
    };
    match scarlet::export::logs(&wow, &out) {
        Ok(()) => "combat logs done".into(),
        Err(e) => format!("logs failed: {e}"),
    }
}

#[tauri::command]
pub fn toggle_watch(app: AppHandle, on: bool) -> bool {
    set_watch(&app, on);
    WATCH_ON.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn get_settings() -> Settings {
    settings::load()
}

#[tauri::command]
pub fn set_settings(app: AppHandle, wow: String, data: String) -> Result<Settings, String> {
    let wow_p = PathBuf::from(&wow);
    let data_p = PathBuf::from(&data);
    if !crate::settings::not_on_c(&data_p) {
        return Err("the data folder may not be on C:".into());
    }
    if !crate::settings::not_on_c(&settings::webview_data()) {
        return Err("webview data may not be on C:".into());
    }
    let cur = settings::update(|c| {
        c.wow = wow_p;
        c.data = data_p;
    })?;
    *state(&app).settings.lock().unwrap() = cur.clone();
    reload(&app);
    Ok(cur)
}

/// Turn "Open when WoW Forever starts" on or off: the setting, the HKCU Run value and
/// the watcher. A running watcher sees the setting off and exits within a few seconds.
#[tauri::command]
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

/// Persist the last view so the app reopens on it.
#[tauri::command]
pub fn save_view(view: String) {
    let _ = settings::update(|c| c.last_view = view);
}

/// Set the watch flag and (re)spawn the thread if needed.
pub fn set_watch(app: &AppHandle, on: bool) {
    WATCH_ON.store(on, Ordering::Relaxed);
    if on {
        let generation = WATCH_GEN.fetch_add(1, Ordering::Relaxed) + 1;
        std::thread::spawn(watch_loop(app.clone(), generation));
    }
    let _ = app.emit("watch-changed", on);
}

fn watch_loop(app: AppHandle, generation: u64) -> impl FnOnce() {
    move || {
        let mut seen: HashMap<PathBuf, SystemTime> = HashMap::new();
        let mut first = true;
        loop {
            if !WATCH_ON.load(Ordering::Relaxed) || WATCH_GEN.load(Ordering::Relaxed) != generation
            {
                return;
            }
            let s = state(&app);
            let (wow, out) = {
                let st = s.settings.lock().unwrap();
                (st.wow.clone(), st.data.clone())
            };
            let mut changed = first;
            if let Ok(files) = scarlet::export::find_sv(&wow) {
                for sv in files {
                    if let Ok(m) = std::fs::metadata(&sv)
                        && let Ok(t) = m.modified()
                        && seen.insert(sv.clone(), t) != Some(t)
                    {
                        changed = true;
                    }
                }
            }
            if changed {
                let _ = scarlet::export::export(&wow, &out);
                reload(&app);
                first = false;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    }
}
