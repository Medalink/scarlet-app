//! Uploading to the Scarlet website, from Settings. The work is `scarlet::upload`; this file
//! holds the app's switches, runs an upload in the background after each export, and answers the
//! Settings view. With uploading off nothing here makes a network request.

use crate::commands::{blocking, state};
use crate::settings::{self, Settings};
use scarlet::upload::api::jitter;
use scarlet::upload::config::{self, UploadConfig};
use scarlet::upload::keys;
use scarlet::upload::session::{self, Session};
use scarlet::upload::state as ustate;
use serde::Serialize;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tauri::AppHandle;

/// True while an upload runs. A second request while one runs sets `PENDING`, and the running
/// thread goes round once more, so uploads never overlap and none is lost.
static BUSY: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
const RETRY_SECS: [u64; 4] = [30, 120, 600, 1800];
const LAUNCH_SPREAD_MS: u64 = 60_000;
static RETRY_STEP: AtomicUsize = AtomicUsize::new(0);
static SYNCS_ASKED: AtomicU64 = AtomicU64::new(0);
/// The one quiet line the last upload left, if it failed.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);
/// What the website removed behind the player's back, said once.
static LAST_NOTE: Mutex<Option<String>> = Mutex::new(None);

#[derive(Serialize)]
pub struct GroupRow {
    key: String,
    label: String,
    switchable: bool,
    on: bool,
}

#[derive(Serialize)]
pub struct CharRow {
    name: String,
    on: bool,
    /// Epoch seconds of the last finished sync, 0 for never.
    last_sync: u64,
    sessions: u64,
    parts: usize,
    last_sent: u64,
    total_sent: u64,
}

#[derive(Serialize)]
pub struct UploadView {
    enabled: bool,
    server: String,
    /// Why uploading cannot be turned on; None when the address is usable.
    problem: Option<String>,
    groups: Vec<GroupRow>,
    characters: Vec<CharRow>,
    /// True once anything was uploaded from this PC, so a switched-off group has something to keep
    /// or remove.
    uploaded: bool,
    busy: bool,
    error: Option<String>,
    note: Option<String>,
    /// The last uploads, newest first.
    log: Vec<ustate::LogEntry>,
}

fn view() -> UploadView {
    let s = settings::load();
    let cfg = s.upload.clone();
    let st = ustate::load(&s.data);
    let groups = config::all_groups()
        .into_iter()
        .filter(|g| g.key != "names")
        .map(|g| GroupRow {
            on: cfg.group_on(&g.key),
            key: g.key,
            label: g.label,
            switchable: g.switchable,
        })
        .collect();
    let characters = session::list_characters(&s.data)
        .into_iter()
        .map(|name| {
            let c = st.characters.get(&name).cloned().unwrap_or_default();
            CharRow {
                on: cfg.character_on(&name),
                last_sync: c.last_sync,
                sessions: c.sessions,
                parts: c.acked.len(),
                last_sent: c.last_sent,
                total_sent: c.total_sent,
                name,
            }
        })
        .collect();
    UploadView {
        enabled: cfg.enabled,
        problem: cfg.server_problem(),
        server: cfg.server,
        groups,
        characters,
        uploaded: st.characters.values().any(|c| c.last_sync > 0),
        busy: BUSY.load(Ordering::Relaxed),
        error: LAST_ERROR.lock().ok().and_then(|e| e.clone()),
        note: LAST_NOTE.lock().ok().and_then(|e| e.clone()),
        log: st.log.iter().rev().take(20).cloned().collect(),
    }
}

/// Change the upload settings on disk and in the running app.
fn change(app: &AppHandle, f: impl FnOnce(&mut UploadConfig)) -> Result<Settings, String> {
    let cur = settings::update(|s| f(&mut s.upload))?;
    state(app).settings.lock().unwrap().upload = cur.upload.clone();
    Ok(cur)
}

fn set_error(e: Option<String>) {
    if let Ok(mut l) = LAST_ERROR.lock() {
        *l = e;
    }
}

fn set_note(n: Option<String>) {
    if let Ok(mut l) = LAST_NOTE.lock() {
        *l = n;
    }
}

/// Uploads in the background when uploading is on: after an export wrote new lines, when the app
/// opens, and when uploading is switched on.
pub fn start_if_on(app: &AppHandle) {
    let cfg = settings::load().upload;
    if cfg.enabled && cfg.server_problem().is_none() {
        start(app);
    }
}

/// `start_if_on` after a random wait of up to a minute, so apps a release restarts together do not sync together.
pub fn start_if_on_after_launch(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(
            jitter(LAUNCH_SPREAD_MS).saturating_sub(Duration::from_millis(LAUNCH_SPREAD_MS / 2)),
        );
        start_if_on(&app);
    });
}

fn start(app: &AppHandle) {
    RETRY_STEP.store(0, Ordering::Release);
    SYNCS_ASKED.fetch_add(1, Ordering::AcqRel);
    spawn_sync(app);
}

fn retry_later(app: &AppHandle) {
    let step = RETRY_STEP.fetch_add(1, Ordering::AcqRel);
    let Some(secs) = RETRY_SECS.get(step) else {
        return;
    };
    let asked = SYNCS_ASKED.load(Ordering::Acquire);
    let app = app.clone();
    let wait = jitter(secs * 1000);
    std::thread::spawn(move || {
        std::thread::sleep(wait);
        if SYNCS_ASKED.load(Ordering::Acquire) == asked {
            spawn_sync(&app);
        }
    });
}

fn spawn_sync(app: &AppHandle) {
    if BUSY.swap(true, Ordering::AcqRel) {
        PENDING.store(true, Ordering::Release);
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            PENDING.store(false, Ordering::Release);
            if run_once(&app) && !PENDING.load(Ordering::Acquire) {
                retry_later(&app);
            }
            if !PENDING.load(Ordering::Acquire) {
                break;
            }
        }
        BUSY.store(false, Ordering::Release);
    });
}

fn run_once(app: &AppHandle) -> bool {
    let s = settings::load();
    let cfg = s.upload.clone();
    if !cfg.enabled {
        return false;
    }
    let mut sess = match Session::open(&cfg, &s.data, keys::system_store().as_ref(), true) {
        Ok(Some(sess)) => sess,
        Ok(None) => return false,
        Err(e) => {
            let _ = ustate::record(
                &s.data,
                ustate::LogEntry {
                    at: ustate::now(),
                    error: Some(e.to_string()),
                    ..Default::default()
                },
            );
            set_error(Some(e.to_string()));
            return e.transient();
        }
    };
    let reports = sess.sync_all(None);
    let mut refused: Vec<String> = Vec::new();
    for r in &reports {
        refused.extend(r.refused.iter().cloned());
    }
    refused.sort();
    refused.dedup();
    if !refused.is_empty() {
        let _ = change(app, |c| {
            for g in &refused {
                c.groups.insert(g.clone(), false);
            }
        });
        let all = config::all_groups();
        let names: Vec<String> = refused
            .iter()
            .map(|k| {
                all.iter()
                    .find(|g| g.key == *k)
                    .map(|g| g.label.clone())
                    .unwrap_or_else(|| k.clone())
            })
            .collect();
        let pronoun = if names.len() == 1 {
            "It is"
        } else {
            "They are"
        };
        set_note(Some(format!(
            "The website removed {}. {pronoun} switched off here.",
            names.join(", ")
        )));
    }
    let failed = reports.iter().find(|r| r.failed);
    set_error(failed.and_then(|r| r.note.clone()));
    reports.iter().any(|r| r.retry)
}

#[tauri::command]
pub async fn upload_get() -> UploadView {
    blocking(view).await
}

#[tauri::command]
pub async fn upload_set_enabled(app: AppHandle, on: bool) -> Result<UploadView, String> {
    blocking(move || {
        if on && let Some(p) = settings::load().upload.server_problem() {
            return Err(format!("{p} Uploading cannot be turned on."));
        }
        change(&app, |c| c.enabled = on)?;
        set_error(None);
        set_note(None);
        if on {
            start_if_on(&app);
        }
        Ok(view())
    })
    .await
}

#[tauri::command]
pub async fn upload_set_server(app: AppHandle, server: String) -> Result<UploadView, String> {
    blocking(move || {
        let server = match server.trim() {
            "" => config::DEFAULT_SERVER.to_string(),
            s => s.to_string(),
        };
        if let Some(p) = config::server_problem(&server) {
            return Err(p);
        }
        change(&app, |c| c.server = server)?;
        set_error(None);
        Ok(view())
    })
    .await
}

/// Switch a group. Turning one on tells the website to take it again when this PC has uploaded
/// before; the next upload then sends only that group's parts.
#[tauri::command]
pub async fn upload_set_group(app: AppHandle, key: String, on: bool) -> Result<UploadView, String> {
    blocking(move || {
        if !config::is_switchable(&key) {
            return Err(format!("{key} cannot be switched."));
        }
        let cur = change(&app, |c| {
            c.groups.insert(key.clone(), on);
        })?;
        if on && cur.upload.enabled && cur.upload.server_problem().is_none() {
            let told = Session::open(&cur.upload, &cur.data, keys::system_store().as_ref(), false)
                .and_then(|s| s.map_or(Ok(()), |mut s| s.restore_group(&key)));
            if let Err(e) = told {
                set_error(Some(e.to_string()));
            }
        }
        Ok(view())
    })
    .await
}

/// Remove what the website holds for a group that was switched off.
#[tauri::command]
pub async fn upload_remove_group(app: AppHandle, key: String) -> Result<String, String> {
    blocking(move || {
        if !config::is_switchable(&key) {
            return Err(format!("{key} cannot be removed."));
        }
        let cur = change(&app, |c| {
            c.groups.insert(key.clone(), false);
        })?;
        let Some(mut s) =
            Session::open(&cur.upload, &cur.data, keys::system_store().as_ref(), false)
                .map_err(|e| e.to_string())?
        else {
            return Ok("The website holds nothing from this PC.".into());
        };
        let (parts, rows) = s.remove_group(&key).map_err(|e| e.to_string())?;
        Ok(format!(
            "Removed {parts} {} and {rows} {} from the website.",
            if parts == 1 { "part" } else { "parts" },
            if rows == 1 { "row" } else { "rows" },
        ))
    })
    .await
}

#[tauri::command]
pub async fn upload_set_character(
    app: AppHandle,
    name: String,
    on: bool,
) -> Result<UploadView, String> {
    blocking(move || {
        change(&app, |c| {
            c.characters.insert(name, on);
        })?;
        Ok(view())
    })
    .await
}

#[tauri::command]
pub async fn upload_delete_all() -> Result<String, String> {
    blocking(|| {
        let s = settings::load();
        let Some(mut sess) =
            Session::open(&s.upload, &s.data, keys::system_store().as_ref(), false)
                .map_err(|e| e.to_string())?
        else {
            return Ok("The website holds nothing from this PC.".into());
        };
        let (chars, parts, rows) = sess.delete_all().map_err(|e| e.to_string())?;
        Ok(format!(
            "Removed {chars} {}, {parts} {} and {rows} {} from the website.",
            if chars == 1 {
                "character"
            } else {
                "characters"
            },
            if parts == 1 { "part" } else { "parts" },
            if rows == 1 { "row" } else { "rows" },
        ))
    })
    .await
}

#[tauri::command]
pub async fn upload_now(app: AppHandle) -> Result<UploadView, String> {
    blocking(move || {
        let cfg = settings::load().upload;
        if !cfg.enabled {
            return Err("Uploading is off.".into());
        }
        if let Some(p) = cfg.server_problem() {
            return Err(p);
        }
        set_error(None);
        start(&app);
        Ok(view())
    })
    .await
}

#[derive(Serialize)]
pub struct LinkView {
    code: String,
    linked: bool,
}

/// Whether this install belongs to an account. Asks the website only when uploading is on and this
/// PC has an install key.
#[tauri::command]
pub async fn upload_linked() -> Result<bool, String> {
    blocking(|| {
        let s = settings::load();
        if !s.upload.enabled || s.upload.server_problem().is_some() {
            return Ok(false);
        }
        match Session::open(&s.upload, &s.data, keys::system_store().as_ref(), false)
            .map_err(|e| e.to_string())?
        {
            Some(mut sess) => sess.me().map(|m| m.linked).map_err(|e| e.to_string()),
            None => Ok(false),
        }
    })
    .await
}

/// A link code, and the page to confirm it on, opened in the browser.
#[tauri::command]
pub async fn upload_link() -> Result<LinkView, String> {
    blocking(|| {
        let s = settings::load();
        if !s.upload.enabled {
            return Err("Uploading is off.".into());
        }
        let mut sess = Session::open(&s.upload, &s.data, keys::system_store().as_ref(), true)
            .map_err(|e| e.to_string())?
            .ok_or("This install is not set up for uploading yet.")?;
        if sess.me().map_err(|e| e.to_string())?.linked {
            return Ok(LinkView {
                code: String::new(),
                linked: true,
            });
        }
        let link = sess.link().map_err(|e| e.to_string())?;
        open_url(&link.url)?;
        Ok(LinkView {
            code: link.code,
            linked: false,
        })
    })
    .await
}

/// Opens an address in the default browser. Only called with the link address the website just
/// returned for this install.
fn open_url(url: &str) -> Result<(), String> {
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1")) {
        return Err("The link address is not safe to open.".into());
    }
    let wide = |s: &str| {
        s.encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    // SAFETY: ShellExecute wants COM on the calling thread; a success code is paired with an
    // uninitialize below, on the same thread.
    let com = unsafe {
        CoInitializeEx(
            std::ptr::null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        )
    } >= 0;
    // SAFETY: both strings are NUL-terminated and outlive the call; the other arguments are null.
    let rc = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            wide("open").as_ptr(),
            wide(url).as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as usize;
    if com {
        // SAFETY: pairs the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    if rc > 32 {
        Ok(())
    } else {
        Err(format!("Windows could not open the page (code {rc})."))
    }
}
