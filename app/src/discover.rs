//! Finds the World of Warcraft folder when the one in the settings does not exist on this
//! PC (a fresh install, or a settings file copied from another machine).

use crate::{commands, launch, settings, watch};
use std::path::PathBuf;
use tauri::AppHandle;

/// The World of Warcraft folder of a running game, else the first one found on a drive. None for
/// a `SCARLET_HOME` copy (see `settings::detect_wow`).
fn find() -> Option<PathBuf> {
    if settings::home_override().is_some() {
        return None;
    }
    // Wow.exe sits in the game folder (`_forever_`), one level below the WoW folder.
    for exe in launch::wow_paths() {
        if let Some(root) = exe.parent().and_then(|game| game.parent())
            && scarlet::gamedir::is_wow_root(root)
        {
            return Some(root.to_path_buf());
        }
    }
    settings::detect_wow()
}

/// Looks on a background thread and, when it finds the folder, saves it in the settings.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let Some(found) = find() else { return };
        let saved = settings::update(|c| {
            // A folder the user typed meanwhile wins.
            if !c.wow.is_dir() {
                c.wow = found.clone();
            }
        });
        if let Ok(cur) = saved {
            commands::state(&app).settings.lock().unwrap().wow = cur.wow;
            watch::wake();
        }
    });
}
