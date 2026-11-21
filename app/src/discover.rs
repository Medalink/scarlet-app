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

/// A running WoW Forever client from a folder Scarlet does not read yet (a second install, or
/// another Battle.net login's) adds that folder to the list, so one Scarlet covers every box on
/// the PC without the player entering each. Not for a `SCARLET_HOME` copy.
pub fn adopt_running(app: &AppHandle) {
    if settings::home_override().is_some() {
        return;
    }
    let known = commands::state(app).settings.lock().unwrap().folders();
    let mut fresh: Vec<PathBuf> = Vec::new();
    for exe in launch::wow_paths() {
        let Some(root) = exe.parent().and_then(|game| game.parent()) else {
            continue;
        };
        let is_new = |q: &PathBuf| settings::folder_key(q) == settings::folder_key(root);
        if scarlet::gamedir::is_wow_root(root)
            && exe.parent().is_some_and(|g| g.ends_with("_forever_"))
            && !known.iter().any(is_new)
            && !fresh.iter().any(is_new)
        {
            fresh.push(root.to_path_buf());
        }
    }
    if fresh.is_empty() {
        return;
    }
    if let Ok(cur) = settings::update(|c| {
        for p in &fresh {
            if c.wow.as_os_str().is_empty() || !c.wow.is_dir() {
                c.wow = p.clone();
            } else if !c
                .folders()
                .iter()
                .any(|q| settings::folder_key(q) == settings::folder_key(p))
            {
                c.more_wow.push(p.clone());
            }
        }
    }) {
        *commands::state(app).settings.lock().unwrap() = cur;
        watch::wake();
    }
}
