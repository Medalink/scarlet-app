//! settings.json next to the exe. Nothing is ever written to C:.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Serialises load-modify-save within this process.
static WRITE: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    pub wow: PathBuf,
    pub data: PathBuf,
    pub watch: bool,
    pub last_view: String,
    /// Open Scarlet when WoW Forever starts (the `--on-wow` watcher).
    #[serde(default)]
    pub open_with_wow: bool,
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `E:\wow\Scarlet\out` when it exists, else `<exe dir>\data`.
fn default_data() -> PathBuf {
    if let Ok(v) = std::env::var("SCARLET_OUT")
        && !v.is_empty()
    {
        return PathBuf::from(v);
    }
    let wow_out = PathBuf::from(r"E:\wow\Scarlet\out");
    if wow_out.is_dir() {
        return wow_out;
    }
    exe_dir().join("data")
}

pub fn path() -> PathBuf {
    exe_dir().join("settings.json")
}

pub fn load() -> Settings {
    let fallback = Settings {
        wow: PathBuf::from(scarlet::DEFAULT_WOW),
        data: default_data(),
        watch: false,
        last_view: "overview".into(),
        open_with_wow: false,
    };
    let Ok(text) = fs::read_to_string(path()) else {
        return fallback;
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(mut s) => {
            if !s.data.is_dir() && !s.data.exists() && s.data.as_os_str().is_empty() {
                s.data = fallback.data;
            }
            s
        }
        Err(_) => fallback,
    }
}

/// The file as written, or None when it is missing, unreadable or half-written.
pub fn try_load() -> Option<Settings> {
    serde_json::from_str(&fs::read_to_string(path()).ok()?).ok()
}

/// Written beside the file and renamed over it, so a reader never sees half a file.
pub fn save(s: &Settings) -> Result<(), String> {
    let p = path();
    let tmp = p.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&tmp, json + "\n").map_err(|e| e.to_string())?;
    fs::rename(&tmp, &p).map_err(|e| e.to_string())
}

/// Load, change and save under one lock.
pub fn update(f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
    let _g = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let mut cur = load();
    f(&mut cur);
    save(&cur)?;
    Ok(cur)
}

/// `<exe dir>\webview-data`, so WebView2 never touches %LOCALAPPDATA%.
pub fn webview_data() -> PathBuf {
    exe_dir().join("webview-data")
}

pub fn temp_dir() -> PathBuf {
    PathBuf::from(r"E:\Temp")
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
