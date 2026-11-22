//! The Overview's lifetime figures for one character: quests, titles and zone maps from the
//! addon's `QB`, `KT` and `EB` lines, and the instance locks that have not reset.

use crate::commands::with_store;
use scarlet::lifetime::{Counted, Lock};
use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
pub struct LifetimeView {
    quests: Option<Counted>,
    titles: Option<Counted>,
    explored: Option<Counted>,
    locks: Vec<Lock>,
    now: i64,
}

#[tauri::command(async)]
pub fn lifetime(app: AppHandle, ch: Option<String>) -> Option<LifetimeView> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    with_store(&app, |st| {
        let index = st.char_index(&ch?)? as usize;
        let mut life = st.lifetimes.get(index)?.clone();
        life.keep_current_locks(now);
        Some(LifetimeView {
            quests: life.quests,
            titles: life.titles,
            explored: life.explored,
            locks: life.locks,
            now,
        })
    })
}
