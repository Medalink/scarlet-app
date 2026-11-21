//! The Addons view: lines Scarlet recorded from other addons. They are read
//! from `<data>/<Name-Surname>/addons/payloads` when the view opens and never
//! enter the main store, so the other views and their totals do not change.

use crate::commands::state;
use scarlet::addons;
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::AppHandle;

/// The newest lines sent to the view.
const LINE_LIMIT: usize = 500;

#[derive(Serialize)]
pub struct KeyOut {
    key: String,
    label: String,
    lines: u64,
    first_ms: i64,
    last_ms: i64,
}

#[derive(Serialize)]
pub struct LineOut {
    ms: i64,
    character: String,
    key: String,
    label: String,
    text: String,
}

#[derive(Serialize)]
pub struct AddonsView {
    total: u64,
    keys: Vec<KeyOut>,
    /// Newest first.
    lines: Vec<LineOut>,
}

#[tauri::command]
pub fn addons(app: AppHandle, ch: Option<String>) -> AddonsView {
    let dir = state(&app).settings.lock().unwrap().data.clone();
    let mut keys: BTreeMap<String, KeyOut> = BTreeMap::new();
    let mut lines: Vec<LineOut> = Vec::new();
    let mut total = 0u64;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("addons").join("payloads").is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| ch.as_ref().is_none_or(|c| c == n))
        .collect();
    names.sort();
    for name in names {
        for ev in addons::read_payloads(&dir.join(&name)) {
            let ms = ev.cs * 10;
            total += 1;
            let k = keys.entry(ev.key.clone()).or_insert_with(|| KeyOut {
                key: ev.key.clone(),
                label: addons::label(&ev.key).to_string(),
                lines: 0,
                first_ms: ms,
                last_ms: ms,
            });
            k.lines += 1;
            k.first_ms = k.first_ms.min(ms);
            k.last_ms = k.last_ms.max(ms);
            lines.push(LineOut {
                ms,
                character: name.clone(),
                label: addons::label(&ev.key).to_string(),
                text: addons::details(&ev.key, &ev.code, &ev.fields),
                key: ev.key,
            });
        }
    }
    lines.sort_by_key(|l| std::cmp::Reverse(l.ms));
    lines.truncate(LINE_LIMIT);
    AddonsView {
        total,
        keys: keys.into_values().collect(),
        lines,
    }
}
