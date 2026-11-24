//! "Watch": export again when the game writes SavedVariables. The thread sleeps in
//! `WaitForMultipleObjects` on a Windows change notification for the account folder, so
//! it costs nothing while the game is not writing. Each WoW folder has its own notification. A
//! switch-off or a settings change sets the wake event; a PC without the game folder yet is
//! rechecked every 30 s.

use crate::commands::{reload, state};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{
    FALSE, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE, FindCloseChangeNotification,
    FindFirstChangeNotificationW, FindNextChangeNotification,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, INFINITE, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
};

pub static WATCH_ON: AtomicBool = AtomicBool::new(false);
/// True while the watch thread is alive. Taken under the lock to decide to exit or to spawn,
/// so a quick off-then-on never leaves the watch without a thread.
#[allow(
    clippy::mutex_atomic,
    reason = "the flag is read and set while the lock is held, so the exit-or-spawn decision is one step"
)]
static RUNNING: Mutex<bool> = Mutex::new(false);
/// Auto-reset event the thread also waits on. The handle is kept as a number (it is never closed).
static WAKE: OnceLock<usize> = OnceLock::new();

/// Debounce after a notification: the game writes the file and its backup in a burst.
const SETTLE: Duration = Duration::from_secs(1);
/// How often to look again for the account folder while it does not exist.
const RECHECK_MS: u32 = 30_000;
/// Folders watched at once: a wait takes 64 handles, one of them the wake event.
const MAX_FOLDERS: usize = 60;

fn wake_handle() -> HANDLE {
    *WAKE.get_or_init(|| {
        // SAFETY: null security attributes and a null name are allowed; the new event is never closed.
        unsafe { CreateEventW(std::ptr::null(), FALSE, FALSE, std::ptr::null()) as usize }
    }) as HANDLE
}

/// Makes the watch thread re-read the settings and the folder it watches.
pub fn wake() {
    // SAFETY: the handle is the process-lifetime event from wake_handle.
    unsafe { SetEvent(wake_handle()) };
}

fn wide(p: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    p.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// A change notification on the folder and everything below it, or None when it is missing.
fn watch_dir(dir: &Path) -> Option<HANDLE> {
    let w = wide(dir);
    // SAFETY: `w` is a NUL-terminated wide path that outlives the call; failure returns INVALID_HANDLE_VALUE, checked below.
    let h = unsafe {
        FindFirstChangeNotificationW(
            w.as_ptr(),
            1,
            FILE_NOTIFY_CHANGE_LAST_WRITE | FILE_NOTIFY_CHANGE_FILE_NAME,
        )
    };
    (h != INVALID_HANDLE_VALUE && !h.is_null()).then_some(h)
}

/// Set the watch flag and start the thread if it is not running.
pub fn set_watch(app: &AppHandle, on: bool) {
    WATCH_ON.store(on, Ordering::Relaxed);
    if on {
        let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if !*running {
            *running = true;
            let app = app.clone();
            std::thread::spawn(move || watch_loop(app));
        }
    }
    wake();
    let _ = app.emit("watch-changed", on);
}

/// Exports and reloads the store only when the export wrote new lines.
pub fn export_and_reload(app: &AppHandle, wows: &[PathBuf], out: &Path) -> Result<bool, String> {
    let wrote = scarlet::export::export_new_all(wows, out).map_err(|e| e.to_string())?;
    if wrote {
        reload(app);
        crate::upload::start_if_on(app);
    }
    Ok(wrote)
}

fn watch_loop(app: AppHandle) {
    loop {
        {
            let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
            if !WATCH_ON.load(Ordering::Relaxed) {
                *running = false;
                return;
            }
        }
        let s = state(&app);
        let (wows, out) = {
            let st = s.settings.lock().unwrap_or_else(|e| e.into_inner());
            (st.folders(), st.data.clone())
        };
        // One notification per WoW folder: each game account in each of them writes its own
        // SavedVariables file when its client logs out or reloads, at its own time.
        let mut notes: Vec<HANDLE> = Vec::new();
        let mut missing = wows.is_empty();
        for wow in wows.iter().take(MAX_FOLDERS) {
            let accounts: PathBuf = scarlet::gamedir::game_dir(wow).join("WTF").join("Account");
            match watch_dir(&accounts) {
                Some(h) => notes.push(h),
                None => missing = true,
            }
        }
        // The files as they were before an export read them. With two clients running, one
        // can log out while the other's export is under way: its file differs from this
        // list afterwards, so it is exported again whatever state.json says.
        let mut stamps = scarlet::export::sv_stamps(&wows);
        // Checked after the notifications exist, so a write between the check and the wait is
        // seen. A start with nothing newer than the last export does no work.
        if scarlet::export::has_new_all(&wows, &out)
            && export_and_reload(&app, &wows, &out).is_err()
        {
            // Do not spin on a file that cannot be read yet.
            stamps.clear();
            // SAFETY: the handle is the process-lifetime event from wake_handle.
            unsafe { WaitForSingleObject(wake_handle(), 5_000) };
        }

        if notes.is_empty() {
            // SAFETY: the handle is the process-lifetime event from wake_handle.
            unsafe { WaitForSingleObject(wake_handle(), RECHECK_MS) };
            continue;
        }
        // Wait here until a folder changes, or the settings or the switch do. A folder that
        // is not there yet is looked for again every 30 s.
        loop {
            let mut objs = vec![wake_handle()];
            objs.extend(&notes);
            let timeout = if missing { RECHECK_MS } else { INFINITE };
            // SAFETY: `objs` holds the wake event and at most MAX_FOLDERS open notification handles, and outlives the wait.
            let r =
                unsafe { WaitForMultipleObjects(objs.len() as u32, objs.as_ptr(), FALSE, timeout) };
            // Index 0 is the wake event; anything but a folder's notification re-reads.
            let i = r.wrapping_sub(WAIT_OBJECT_0) as usize;
            if r == WAIT_TIMEOUT || i == 0 || i >= objs.len() {
                break;
            }
            let note = objs[i];
            // SAFETY: `note` is an open change notification from `notes`.
            unsafe { FindNextChangeNotification(note) };
            std::thread::sleep(SETTLE);
            // SAFETY: `note` is still an open change notification from `notes`.
            unsafe { FindNextChangeNotification(note) };
            if !WATCH_ON.load(Ordering::Relaxed) {
                break;
            }
            let now = scarlet::export::sv_stamps(&wows);
            if now != stamps || scarlet::export::has_new_all(&wows, &out) {
                // Kept only when the export worked, so the next change tries again.
                stamps = if export_and_reload(&app, &wows, &out).is_ok() {
                    now
                } else {
                    Vec::new()
                };
            }
        }
        for note in notes {
            // SAFETY: each notification was opened by watch_dir and is closed once, here.
            unsafe { FindCloseChangeNotification(note) };
        }
    }
}
