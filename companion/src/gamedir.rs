//! Finding the game folder under the World of Warcraft folder, and the World of
//! Warcraft folder itself when the saved path does not exist on this PC.

use std::fs;
use std::path::{Path, PathBuf};

/// The folder WoW Forever installs into, under the World of Warcraft folder.
const FOREVER: &str = "_forever_";

/// A game folder holds the WTF and Interface folders (and `Wow.exe`).
fn is_game_dir(p: &Path) -> bool {
    p.join("WTF").is_dir() || p.join("Interface").is_dir() || p.join("Wow.exe").is_file()
}

/// The folder that holds `WTF`, `Interface`, `Logs` and `Wow.exe`: the World of Warcraft
/// folder itself when it is the game folder, else `_forever_` under it, else the only other
/// `_name_` folder that is one. When none exists yet it is `_forever_`.
pub fn game_dir(wow: &Path) -> PathBuf {
    let forever = wow.join(FOREVER);
    if forever.is_dir() {
        return forever;
    }
    if is_game_dir(wow) {
        return wow.to_path_buf();
    }
    let mut found: Vec<PathBuf> = fs::read_dir(wow)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            n.len() > 2 && n.starts_with('_') && n.ends_with('_') && is_game_dir(p)
        })
        .collect();
    found.sort();
    found.into_iter().next().unwrap_or(forever)
}

/// True when `p` is a World of Warcraft folder that has a WoW Forever game folder.
pub fn is_wow_root(p: &Path) -> bool {
    p.join(FOREVER).is_dir()
}

/// Looks for the World of Warcraft folder on every drive in the places such folders
/// usually are. Slow on a PC with disconnected network drives, so call it off the UI thread.
pub fn find_wow() -> Option<PathBuf> {
    let roots = ["", "Games", "Program Files (x86)", "Program Files"];
    for letter in b'A'..=b'Z' {
        for root in roots {
            let mut dir = PathBuf::from(format!("{}:\\", letter as char));
            if !root.is_empty() {
                dir.push(root);
            }
            let cand = dir.join("World of Warcraft");
            if is_wow_root(&cand) {
                return Some(cand);
            }
        }
    }
    None
}
