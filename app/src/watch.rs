//! "Watch": export again when the game writes SavedVariables. The thread sleeps in
//! `WaitForMultipleObjects` on a Windows change notification for the account folder, so
//! it costs nothing while the game is not writing. A switch-off or a settings change sets
//! the wake event; a PC without the game folder yet is rechecked every 30 s.

use crate::commands::{reload, state};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::{FALSE, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
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
static RUNNING: Mutex<bool> = Mutex::new(false);
/// Auto-reset event the thread also waits on. The handle is kept as a number (it is never closed).
static WAKE: OnceLock<usize> = OnceLock::new();

/// Debounce after a notification: the game writes the file and its backup in a burst.
const SETTLE: Duration = Duration::from_secs(1);
/// How often to look again for the account folder while it does not exist.
const RECHECK_MS: u32 = 30_000;

fn wake_handle() -> HANDLE {
    *WAKE.get_or_init(|| unsafe {
        CreateEventW(std::ptr::null(), FALSE, FALSE, std::ptr::null()) as usize
    }) as HANDLE
}

/// Makes the watch thread re-read the settings and the folder it watches.
pub fn wake() {
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
pub fn export_and_reload(app: &AppHandle, wow: &Path, out: &Path) -> Result<bool, String> {
    let wrote = scarlet::export::export_new(wow, out).map_err(|e| e.to_string())?;
    if wrote {
        reload(app);
        crate::upload::after_export(app);
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
        let (wow, out) = {
            let st = s.settings.lock().unwrap();
            (st.wow.clone(), st.data.clone())
        };
        let accounts: PathBuf = scarlet::gamedir::game_dir(&wow).join("WTF").join("Account");
        let note = watch_dir(&accounts);
        // Checked after the notification exists, so a write between the check and the wait is
        // seen. A start with nothing newer than the last export does no work.
        if scarlet::export::has_new(&wow, &out) && export_and_reload(&app, &wow, &out).is_err() {
            // Do not spin on a file that cannot be read yet.
            unsafe { WaitForSingleObject(wake_handle(), 5_000) };
        }

        let Some(note) = note else {
            unsafe { WaitForSingleObject(wake_handle(), RECHECK_MS) };
            continue;
        };
        // Wait here until the folder changes, or the settings or the switch do.
        loop {
            let objs = [wake_handle(), note];
            let r = unsafe { WaitForMultipleObjects(2, objs.as_ptr(), FALSE, INFINITE) };
            // Index 0 is the wake event; anything but the folder's notification re-reads.
            if r != WAIT_OBJECT_0 + 1 {
                break;
            }
            unsafe { FindNextChangeNotification(note) };
            std::thread::sleep(SETTLE);
            unsafe { FindNextChangeNotification(note) };
            if !WATCH_ON.load(Ordering::Relaxed) {
                break;
            }
            if scarlet::export::has_new(&wow, &out) {
                let _ = export_and_reload(&app, &wow, &out);
            }
        }
        unsafe { FindCloseChangeNotification(note) };
    }
}
