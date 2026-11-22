//! "Open when WoW Forever starts": a windowless copy of this exe (`--on-wow`)
//! started at sign-in from the HKCU Run value. It scans the process list every
//! few seconds until a `Wow.exe` in the game folder of one of the WoW folders starts, opens Scarlet,
//! then waits on the process handles of every running client (no scanning) until all of them have
//! exited. Nothing is written to
//! disk; the Run value is a registry entry, not a Startup-folder file.
//!
//! Also the one-window rule: a second Scarlet window process brings the open
//! window to the front and exits.

use crate::settings;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, FILETIME, GetLastError, HANDLE,
    INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RegDeleteKeyValueW, RegSetKeyValueW,
};
use windows_sys::Win32::System::Threading::{
    CREATE_NO_WINDOW, CreateMutexW, DETACHED_PROCESS, GetProcessTimes, OpenMutexW, OpenProcess,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    SYNCHRONIZATION_SYNCHRONIZE, WaitForMultipleObjects,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowThreadProcessId, IsIconic, MB_ICONERROR, MB_OK, MessageBoxW,
    SW_RESTORE, SetForegroundWindow, ShowWindow,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Scarlet";
/// Held by the open Scarlet window, so the watcher never opens a second one.
const APP_MUTEX: &str = r"Local\ScarletApp";
/// Held by the watcher, so the sign-in copy and the one the app starts never both run.
const WATCH_MUTEX: &str = r"Local\ScarletWowWatch";
pub const FLAG: &str = "--on-wow";
/// Passed by a window that restarts itself, so the new one waits for it to close.
pub const RESTARTED: &str = "--restarted";
/// How many times a restarted window tries for the window mutex, 100 ms apart.
pub const RESTART_WAIT_TRIES: u32 = 100;
const POLL: Duration = Duration::from_secs(3);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// A mutex name; a copy run with `SCARLET_HOME` gets its own, so trying one never
/// collides with the Scarlet the user already has open.
fn mutex_name(base: &str) -> Vec<u16> {
    match std::env::var("SCARLET_HOME") {
        Ok(home) if !home.is_empty() => {
            let hash = home
                .to_lowercase()
                .bytes()
                .fold(0xcbf29ce484222325u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
                });
            wide(&format!("{base}-{hash:x}"))
        }
        _ => wide(base),
    }
}

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

/// Adds or removes the HKCU Run value that starts the watcher at sign-in.
/// A copy run with `SCARLET_HOME` (a test or trial copy) never touches the Run
/// value: it belongs to the installed app, and a copy writing it would point
/// sign-in at its own exe, which may be deleted with its scratch folder.
pub fn set_registered(on: bool) -> Result<(), String> {
    if std::env::var_os("SCARLET_HOME").is_some_and(|v| !v.is_empty()) {
        return Ok(());
    }
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
        Err(format!(
            "Could not update the Windows startup setting (error {rc})."
        ))
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
    let name = mutex_name(APP_MUTEX);
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
    let name = mutex_name(APP_MUTEX);
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

pub(crate) enum Seen {
    /// Running: one handle per client (dual boxing runs several from the same folder), each
    /// able to be waited on and closed by the caller.
    Running(Vec<HANDLE>),
    Stopped,
    /// No snapshot, or a Wow.exe whose path could not be read.
    Unknown,
}

/// The most handles one `WaitForMultipleObjects` takes. More clients than this are not tracked.
const MAX_CLIENTS: usize = 60;

/// Looks for every process running from exactly one of `targets`, the `Wow.exe` of each WoW folder
/// (retail and Classic are Wow.exe too).
pub(crate) fn find(targets: &[String]) -> Seen {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Seen::Unknown;
    }
    let mut unknown = false;
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut found: Vec<HANDLE> = Vec::new();
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
                Some(p) if targets.contains(&norm(Path::new(&p))) && found.len() < MAX_CLIENTS => {
                    found.push(h);
                    more = unsafe { Process32NextW(snap, &mut e) } != 0;
                    continue;
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
    if !found.is_empty() {
        Seen::Running(found)
    } else if unknown {
        Seen::Unknown
    } else {
        Seen::Stopped
    }
}

fn open_app(run: Option<crate::run::Run>) {
    if let Ok(exe) = exe() {
        let mut cmd = std::process::Command::new(&exe);
        if let Some(r) = run {
            cmd.arg(format!("--wow-run={}-{}", r.from, r.to));
        }
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
    let name = mutex_name(WATCH_MUTEX);
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if h.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return;
    }
    // was: WoW was running at the last scan. A scan clears it only after two misses, so
    // one failed read does not look like a new start. With several clients running (dual
    // boxing), one of them closing changes nothing: `was` clears at once only when the last
    // tracked client exits and a fresh scan finds no other one.
    let mut was = false;
    let mut gone = 0;
    let mut targets: Vec<String> = Vec::new();
    let mut clients: Vec<HANDLE> = Vec::new();
    let mut run_from: Option<i64> = None;
    loop {
        // A missing or half-written file keeps the last good path; only a real "off" exits.
        match settings::try_load() {
            Some(s) if !s.open_with_wow => {
                unsafe {
                    for c in clients.drain(..) {
                        CloseHandle(c);
                    }
                    CloseHandle(h);
                }
                return;
            }
            Some(s) => {
                targets = exes(&s.folders());
            }
            None => {}
        }
        // While WoW runs, waiting on every client's handle is the sleep: no scan until one exits.
        if !clients.is_empty() {
            let r = unsafe {
                WaitForMultipleObjects(
                    clients.len() as u32,
                    clients.as_ptr(),
                    0,
                    POLL.as_millis() as u32,
                )
            };
            if r == WAIT_TIMEOUT {
                continue;
            }
            let i = r.wrapping_sub(WAIT_OBJECT_0) as usize;
            if i < clients.len() {
                unsafe { CloseHandle(clients.swap_remove(i)) };
            } else {
                // The wait failed: drop what is held and look again.
                for c in clients.drain(..) {
                    unsafe { CloseHandle(c) };
                }
                std::thread::sleep(POLL);
            }
            if clients.is_empty() {
                // The last tracked client exited. One started meanwhile is WoW still running,
                // not a new start, so Scarlet is not opened for it.
                match if targets.is_empty() {
                    Seen::Unknown
                } else {
                    find(&targets)
                } {
                    Seen::Running(found) => {
                        run_from = earliest_start(&found, run_from);
                        clients = found;
                    }
                    Seen::Stopped => {
                        was = false;
                        run_over(run_from.take());
                    }
                    Seen::Unknown => {}
                }
            }
            continue;
        }
        match if targets.is_empty() {
            Seen::Unknown
        } else {
            find(&targets)
        } {
            Seen::Running(found) => {
                run_from = earliest_start(&found, run_from);
                if !was && !app_open() {
                    open_app(None);
                }
                was = true;
                gone = 0;
                clients = found;
                continue;
            }
            Seen::Stopped => {
                gone += 1;
                if gone >= 2 {
                    was = false;
                    run_over(run_from.take());
                }
            }
            Seen::Unknown => {}
        }
        std::thread::sleep(POLL);
    }
}

fn run_over(from: Option<i64>) {
    if let Some(from) = from
        && !app_open()
    {
        open_app(Some(crate::run::Run {
            from,
            to: crate::run::now_ms(),
        }));
    }
}

/// A message box for a failure when there is no window to show it in.
pub fn message_box(title: &str, text: &str) {
    let (t, x) = (wide(title), wide(text));
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            x.as_ptr(),
            t.as_ptr(),
            MB_OK | MB_ICONERROR,
        )
    };
}

/// Image paths of every running Wow.exe that can be read.
pub fn wow_paths() -> Vec<PathBuf> {
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut e: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut more = unsafe { Process32FirstW(snap, &mut e) } != 0;
    while more {
        let end = e
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(e.szExeFile.len());
        if String::from_utf16_lossy(&e.szExeFile[..end]).eq_ignore_ascii_case("wow.exe")
            && let Some(p) = image_path(e.th32ProcessID)
        {
            out.push(PathBuf::from(p));
        }
        more = unsafe { Process32NextW(snap, &mut e) } != 0;
    }
    unsafe { CloseHandle(snap) };
    out
}

/// The normalised `Wow.exe` path of each WoW folder.
pub(crate) fn exes(folders: &[PathBuf]) -> Vec<String> {
    folders
        .iter()
        .map(|f| norm(&scarlet::gamedir::game_dir(f).join("Wow.exe")))
        .collect()
}

pub(crate) fn earliest_start(clients: &[HANDLE], from: Option<i64>) -> Option<i64> {
    clients.iter().filter_map(|&h| created(h)).chain(from).min()
}

fn created(h: HANDLE) -> Option<i64> {
    let mut birth = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let ok = unsafe { GetProcessTimes(h, &mut birth, &mut exit, &mut kernel, &mut user) };
    (ok != 0).then(|| {
        let since_1601 = ((birth.dwHighDateTime as i64) << 32) | birth.dwLowDateTime as i64;
        (since_1601 - 116_444_736_000_000_000) / 10_000
    })
}

/// For each folder, how many WoW Forever clients run from it: two or more in one folder when
/// the player dual boxes from one install.
pub fn running_clients(folders: &[PathBuf]) -> Vec<usize> {
    let running: Vec<String> = wow_paths().iter().map(|p| norm(p)).collect();
    exes(folders)
        .iter()
        .map(|t| running.iter().filter(|r| *r == t).count())
        .collect()
}
