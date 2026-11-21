//! "Open when WoW Forever starts": a windowless copy of this exe (`--on-wow`)
//! started at sign-in from the HKCU Run value. It scans the process list every
//! few seconds until `<wow>\_forever_\Wow.exe` starts, opens Scarlet, then waits
//! on WoW's process handle (no scanning) until it exits. Nothing is written to
//! disk; the Run value is a registry entry, not a Startup-folder file.
//!
//! Also the one-window rule: a second Scarlet window process brings the open
//! window to the front and exits.

use crate::settings;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateMutexW, DETACHED_PROCESS, OpenMutexW, OpenProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW, SYNCHRONIZATION_SYNCHRONIZE,
    WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowThreadProcessId, IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Scarlet";
/// Held by the open Scarlet window, so the watcher never opens a second one.
const APP_MUTEX: &str = r"Local\ScarletApp";
/// Held by the watcher, so the sign-in copy and the one the app starts never both run.
const WATCH_MUTEX: &str = r"Local\ScarletWowWatch";
pub const FLAG: &str = "--on-wow";
const POLL: Duration = Duration::from_secs(3);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

/// Adds or removes the HKCU Run value that starts the watcher at sign-in.
pub fn set_registered(on: bool) -> Result<(), String> {
    let key = wide(RUN_KEY);
    let name = wide(RUN_VALUE);
    let rc = if on {
        let cmd = format!("\"{}\" {FLAG}", exe()?.display());
        let data = wide(&cmd);
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        }
    } else {
        let rc = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if rc == ERROR_FILE_NOT_FOUND { 0 } else { rc }
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("could not change the startup entry (error {rc})"))
    }
}

/// Starts the watcher in the background; a second copy exits on its own.
pub fn spawn_watcher() {
    if let Ok(exe) = exe() {
        let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let _ = std::process::Command::new(&exe)
            .arg(FLAG)
            .current_dir(dir)
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn();
    }
}

/// Called once by the window process. True when this is the only Scarlet window; the
/// mutex stays held until the process exits. False when one is already open.
pub fn claim_app() -> bool {
    let name = wide(APP_MUTEX);
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if !h.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe { CloseHandle(h) };
        return false;
    }
    true
}

/// Brings the open Scarlet window to the front (restoring it when minimized).
pub fn focus_existing() {
    let title = wide("Scarlet");
    let own_exe_suffix = exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| format!("\\{}", n.to_string_lossy().to_lowercase()))
        })
        .unwrap_or_else(|| "\\scarlet-app.exe".into());
    let mut prev = std::ptr::null_mut();
    for _ in 0..50 {
        let w =
            unsafe { FindWindowExW(std::ptr::null_mut(), prev, std::ptr::null(), title.as_ptr()) };
        if w.is_null() {
            return;
        }
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(w, &mut pid) };
        // The installer may name the exe differently from the cargo binary, so match our own name.
        let ours = image_path(pid).is_some_and(|p| p.to_lowercase().ends_with(&own_exe_suffix));
        if ours {
            unsafe {
                if IsIconic(w) != 0 {
                    ShowWindow(w, SW_RESTORE);
                }
                SetForegroundWindow(w);
            }
            return;
        }
        prev = w;
    }
}

fn app_open() -> bool {
    let name = wide(APP_MUTEX);
    let h = unsafe { OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
    if h.is_null() {
        return false;
    }
    unsafe { CloseHandle(h) };
    true
}

fn norm(p: &Path) -> String {
    p.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn handle_path(h: HANDLE) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    let ok =
        unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Full image path of a process, or None when it cannot be opened.
fn image_path(pid: u32) -> Option<String> {
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return None;
    }
    let p = handle_path(h);
    unsafe { CloseHandle(h) };
    p
}

enum Seen {
    /// Running, with a handle that can be waited on (closed by the caller).
    Running(HANDLE),
    Stopped,
    /// No snapshot, or a Wow.exe whose path could not be read.
    Unknown,
}

/// Looks for a process running from exactly `target` (retail and Classic are Wow.exe too).
fn find(target: &str) -> Seen {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Seen::Unknown;
    }
    let mut unknown = false;
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut found = None;
    let mut more = unsafe { Process32FirstW(snap, &mut e) } != 0;
    while more {
        let end = e
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(e.szExeFile.len());
        let name = String::from_utf16_lossy(&e.szExeFile[..end]);
        if name.eq_ignore_ascii_case("wow.exe") {
            let h = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    e.th32ProcessID,
                )
            };
            match (!h.is_null()).then(|| handle_path(h)).flatten() {
                Some(p) if norm(Path::new(&p)) == target => {
                    found = Some(h);
                    break;
                }
                Some(_) => {}
                None => unknown = true,
            }
            if !h.is_null() {
                unsafe { CloseHandle(h) };
            }
        }
        more = unsafe { Process32NextW(snap, &mut e) } != 0;
    }
    unsafe { CloseHandle(snap) };
    match found {
        Some(h) => Seen::Running(h),
        None if unknown => Seen::Unknown,
        None => Seen::Stopped,
    }
}

fn open_app() {
    if let Ok(exe) = exe() {
        let mut cmd = std::process::Command::new(&exe);
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        let _ = cmd.creation_flags(DETACHED_PROCESS).spawn();
    }
}

/// The `--on-wow` loop. Opens Scarlet when WoW Forever goes from not running to
/// running, so closing Scarlet mid-session does not bring it back. Exits when
/// the setting is turned off.
pub fn watch() {
    let name = wide(WATCH_MUTEX);
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if h.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }
    // was: WoW was running at the last scan. A scan clears it only after two misses, so
    // one failed read does not look like a new start; WoW's own exit clears it at once.
    let mut was = false;
    let mut gone = 0;
    let mut target = None;
    let mut wow: Option<HANDLE> = None;
    loop {
        // A missing or half-written file keeps the last good path; only a real "off" exits.
        match settings::try_load() {
            Some(s) if !s.open_with_wow => {
                unsafe {
                    if let Some(w) = wow {
                        CloseHandle(w);
                    }
                    CloseHandle(h);
                }
                return;
            }
            Some(s) => target = Some(norm(&s.wow.join("_forever_").join("Wow.exe"))),
            None => {}
        }
        // While WoW runs, waiting on its handle is the sleep: no scan until it exits.
        if let Some(w) = wow {
            if unsafe { WaitForSingleObject(w, POLL.as_millis() as u32) } == WAIT_OBJECT_0 {
                unsafe { CloseHandle(w) };
                wow = None;
                was = false;
            }
            continue;
        }
        match target.as_deref().map_or(Seen::Unknown, find) {
            Seen::Running(w) => {
                if !was && !app_open() {
                    open_app();
                }
                was = true;
                gone = 0;
                wow = Some(w);
                continue;
            }
            Seen::Stopped => {
                gone += 1;
                if gone >= 2 {
                    was = false;
                }
            }
            Seen::Unknown => {}
        }
        std::thread::sleep(POLL);
    }
}
