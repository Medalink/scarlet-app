//! Finding the game folder under the World of Warcraft folder.

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

pub(crate) fn sorted_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The game account folders of a game folder (`WTF\Account\<account>`).
pub fn account_dirs(game: &Path) -> Vec<PathBuf> {
    sorted_dirs(&game.join("WTF").join("Account"))
}

/// The `Name-Surname` of every character folder of a game account folder
/// (`<account>\<realm>\<Name-Surname>`), in path order.
pub fn character_names(account: &Path) -> Vec<String> {
    sorted_dirs(account)
        .into_iter()
        .filter(|realm| realm.file_name().is_some_and(|n| n != "SavedVariables"))
        .flat_map(|realm| sorted_dirs(&realm))
        .filter_map(|ch| Some(ch.file_name()?.to_string_lossy().into_owned()))
        .collect()
}

/// True when `p` is a World of Warcraft folder that has a WoW Forever game folder.
pub fn is_wow_root(p: &Path) -> bool {
    p.join(FOREVER).is_dir()
}
