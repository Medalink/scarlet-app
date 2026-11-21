//! Where Scarlet keeps its settings, and what it assumes about a PC it has never seen.
//!
//! Three places, in this order:
//! - `SCARLET_HOME` set: everything (settings, WebView2 profile, temp, default data
//!   folder) lives under that folder. For trying the app without touching the profile.
//! - a `settings.json` next to the exe: the portable copy. Settings, the WebView2
//!   profile and the log stay beside the exe, temp goes to `E:\Temp`, and the data
//!   folder may not be on C:. This is how the owner's dev copy runs.
//! - otherwise the installed app: `%APPDATA%\app.scarlet.desktop\settings.json`
//!   (the folder Tauri's `app_config_dir` returns), WebView2 in its default
//!   `%LOCALAPPDATA%` profile, the data folder under Documents.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
use windows_sys::Win32::System::Com::CoTaskMemFree;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW,
};
use windows_sys::Win32::UI::Shell::{
    FOLDERID_Documents, FOLDERID_RoamingAppData, SHGetKnownFolderPath,
};
use windows_sys::core::{GUID, PWSTR};

/// Serialises load-modify-save within this process.
static WRITE: Mutex<()> = Mutex::new(());

/// The bundle identifier in tauri.conf.json; Tauri's config folder is named after it.
const IDENTIFIER: &str = "app.scarlet.desktop";
const DRIVE_FIXED: u32 = 3;

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub wow: PathBuf,
    pub data: PathBuf,
    pub watch: bool,
    pub last_view: String,
    /// Open Scarlet when WoW Forever starts (the `--on-wow` watcher).
    #[serde(default)]
    pub open_with_wow: bool,
    /// The Map view's art: "dark" (Scarlet's own sheets, the default) or "openmap" (read from the
    /// player's OpenMap addon).
    #[serde(default = "default_map_art")]
    pub map_art: String,
    /// Uploading to the Scarlet website: off until the player turns it on.
    #[serde(default)]
    pub upload: scarlet::upload::config::UploadConfig,
}

fn default_map_art() -> String {
    "dark".into()
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// `SCARLET_HOME`, when set and not empty.
fn home_override() -> Option<PathBuf> {
    std::env::var_os("SCARLET_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// True for a copy that keeps `settings.json` beside the exe (and no `SCARLET_HOME`).
pub fn portable() -> bool {
    static CELL: OnceLock<bool> = OnceLock::new();
    *CELL.get_or_init(|| home_override().is_none() && exe_dir().join("settings.json").is_file())
}

fn known_folder(id: &GUID) -> Option<PathBuf> {
    let mut p: PWSTR = std::ptr::null_mut();
    // SAFETY: SHGetKnownFolderPath fills `p` with a CoTaskMem string that we free below.
    let hr = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p) };
    if p.is_null() {
        return None;
    }
    let text = if hr >= 0 {
        // SAFETY: on success `p` is a NUL-terminated UTF-16 string.
        let units = unsafe {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            std::slice::from_raw_parts(p, n)
        };
        Some(String::from_utf16_lossy(units))
    } else {
        None
    };
    unsafe { CoTaskMemFree(p.cast()) };
    text.map(PathBuf::from)
}

/// The user's Documents folder, wherever Windows (or OneDrive) put it.
fn documents() -> PathBuf {
    known_folder(&FOLDERID_Documents).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("Documents")
    })
}

/// The folder holding settings.json.
pub fn config_dir() -> PathBuf {
    if let Some(h) = home_override() {
        return h;
    }
    if portable() {
        return exe_dir();
    }
    known_folder(&FOLDERID_RoamingAppData)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()))
        .join(IDENTIFIER)
}

pub fn path() -> PathBuf {
    config_dir().join("settings.json")
}

/// Where the window process writes a startup failure (no console in release builds).
pub fn log_path() -> PathBuf {
    config_dir().join("scarlet.log")
}

/// A WebView2 profile folder to force, or None to leave WebView2 its default.
pub fn webview_data() -> Option<PathBuf> {
    if let Some(h) = home_override() {
        return Some(h.join("webview-data"));
    }
    portable().then(|| exe_dir().join("webview-data"))
}

/// A temp folder to force, or None to leave the user's own.
pub fn temp_dir() -> Option<PathBuf> {
    if let Some(h) = home_override() {
        return Some(h.join("temp"));
    }
    portable().then(|| PathBuf::from(r"E:\Temp"))
}

/// `SCARLET_OUT`, else (portable) `E:\wow\Scarlet\out` when it exists or `<exe dir>\data`,
/// else `Scarlet\out` under Documents (under `SCARLET_HOME` when that is set).
fn default_data() -> PathBuf {
    if let Ok(v) = std::env::var("SCARLET_OUT")
        && !v.is_empty()
    {
        return PathBuf::from(v);
    }
    if let Some(h) = home_override() {
        return h.join("Scarlet").join("out");
    }
    if portable() {
        let wow_out = PathBuf::from(r"E:\wow\Scarlet\out");
        if wow_out.is_dir() {
            return wow_out;
        }
        return exe_dir().join("data");
    }
    documents().join("Scarlet").join("out")
}

fn reg_string(root: HKEY, sub: &str, name: &str) -> Option<String> {
    let sub = wide(sub);
    let name = wide(name);
    let mut bytes = 0u32;
    // SAFETY: a size query, then a read into a buffer of the size it reported.
    let rc = unsafe {
        RegGetValueW(
            root,
            sub.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut bytes,
        )
    };
    if rc != 0 || bytes == 0 {
        return None;
    }
    let mut buf = vec![0u16; bytes as usize / 2 + 2];
    let mut got = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            root,
            sub.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buf.as_mut_ptr().cast(),
            &mut got,
        )
    };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let text = String::from_utf16_lossy(&buf[..end]);
    (!text.trim().is_empty()).then_some(text)
}

/// A folder is the WoW Forever install when it holds the `_forever_` client.
fn is_wow_forever(p: &Path) -> bool {
    let f = p.join("_forever_");
    f.join("Wow.exe").is_file() || f.join("WTF").is_dir()
}

/// Finds the folder that holds `_forever_`: the install location Battle.net registered
/// for World of Warcraft, then the usual folders on every fixed drive.
pub fn detect_wow() -> Option<PathBuf> {
    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        for base in [UNINSTALL, UNINSTALL_32] {
            for product in ["World of Warcraft", "World of Warcraft Public Test"] {
                if let Some(v) = reg_string(root, &format!(r"{base}\{product}"), "InstallLocation")
                {
                    candidates.push(PathBuf::from(v.trim_end_matches('\\')));
                }
            }
        }
    }
    // SAFETY: plain Win32 queries.
    let mask = unsafe { GetLogicalDrives() };
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let root = format!("{}:\\", (b'A' + i as u8) as char);
        if unsafe { GetDriveTypeW(wide(&root).as_ptr()) } != DRIVE_FIXED {
            continue;
        }
        for rel in [
            r"Program Files (x86)\World of Warcraft",
            r"Program Files\World of Warcraft",
            r"Games\World of Warcraft",
            r"World of Warcraft",
            r"Blizzard\World of Warcraft",
        ] {
            candidates.push(Path::new(&root).join(rel));
        }
    }
    candidates.into_iter().find(|p| is_wow_forever(p))
}

fn fallback() -> Settings {
    if portable() {
        return Settings {
            wow: PathBuf::from(scarlet::DEFAULT_WOW),
            data: default_data(),
            watch: false,
            last_view: "overview".into(),
            open_with_wow: false,
            map_art: default_map_art(),
            upload: Default::default(),
        };
    }
    let wow = detect_wow();
    Settings {
        // Exporting is what fills the app; start it only when there is a folder to read.
        watch: wow.is_some(),
        wow: wow.unwrap_or_default(),
        data: default_data(),
        last_view: "overview".into(),
        open_with_wow: false,
        map_art: default_map_art(),
        upload: Default::default(),
    }
}

/// The saved settings. The WoW install is searched for only when there is no usable file; a
/// folder the file names stays as written (`discover` replaces one that is gone at startup).
pub fn load() -> Settings {
    let saved = fs::read_to_string(path())
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok());
    match saved {
        Some(mut s) => {
            if s.data.as_os_str().is_empty() {
                s.data = default_data();
            }
            s
        }
        None => fallback(),
    }
}

/// The file as written, or None when it is missing, unreadable or half-written.
pub fn try_load() -> Option<Settings> {
    serde_json::from_str(&fs::read_to_string(path()).ok()?).ok()
}

/// Written beside the file and renamed over it, so a reader never sees half a file.
pub fn save(s: &Settings) -> Result<(), String> {
    let p = path();
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = p.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&tmp, json + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// First run on this PC: write what was found, so the sign-in watcher can read it too.
pub fn ensure_saved() {
    if !path().exists() {
        let _ = save(&load());
    }
}

/// Load, change and save under one lock.
pub fn update(f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let _g = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let mut cur = load();
    f(&mut cur);
    save(&cur)?;
    Ok(cur)
}

/// Why a non-empty `wow` cannot be read, in plain words; None when it holds a game folder
/// (found the way the rest of the app finds it, `gamedir::game_dir`).
pub fn folder_problem(wow: &Path) -> Option<String> {
    if !wow.is_dir() {
        return Some(format!("The folder {} does not exist.", wow.display()));
    }
    if !scarlet::gamedir::game_dir(wow).is_dir() {
        return Some(format!(
            "There is no _forever_ folder inside {}. WoW Forever keeps its client there; \
             enter the folder that contains it.",
            wow.display()
        ));
    }
    None
}

/// What is missing, in plain words. Empty when Scarlet can read a recording.
pub fn problems(s: &Settings) -> Vec<String> {
    let mut out = Vec::new();
    if s.wow.as_os_str().is_empty() {
        out.push(
            "Scarlet could not find World of Warcraft on this PC. Enter the folder that \
             contains _forever_."
                .into(),
        );
        return out;
    }
    if let Some(p) = folder_problem(&s.wow) {
        out.push(p);
        return out;
    }
    let addon = scarlet::gamedir::game_dir(&s.wow)
        .join("Interface")
        .join("AddOns")
        .join("Scarlet");
    if !addon.is_dir() {
        out.push(format!(
            "The Scarlet addon is not installed. Copy its Scarlet folder to {}.",
            addon.parent().unwrap_or(&addon).display()
        ));
    } else if scarlet::export::find_sv(&s.wow).is_ok_and(|v| v.is_empty()) {
        out.push(
            "Scarlet has not saved a recording yet. Play with the addon on, then log out or \
             type /reload so WoW writes it."
                .into(),
        );
    }
    out
}

/// True when the path is absolute and on a drive other than C:. Verbatim
/// (`\\?\C:\`) paths count as C: too.
pub fn not_on_c(p: &Path) -> bool {
    use std::path::{Component, Prefix};
    if !p.is_absolute() {
        return false;
    }
    match p.components().next() {
        Some(Component::Prefix(pre)) => match pre.kind() {
            Prefix::Disk(d) | Prefix::VerbatimDisk(d) => !d.eq_ignore_ascii_case(&b'C'),
            _ => true,
        },
        _ => true,
    }
}
